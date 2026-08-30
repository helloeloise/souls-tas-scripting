use std::collections::{HashMap, HashSet};

use crate::actions;
use crate::ast::*;

const MAX_CALL_DEPTH: usize = 128;
const MAX_STEPS: u64 = 50_000_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
}

impl Value {
    fn as_f64(&self, line: usize) -> Result<f64, String> {
        match self {
            Value::Int(v) => Ok(*v as f64),
            Value::Float(v) => Ok(*v),
            Value::Str(_) => Err(format!("line {line}: expected a number, got a string")),
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            Value::Str(s) => !s.is_empty(),
        }
    }

    /// Textual form used both for `print` and for action arguments.
    pub fn render(&self) -> String {
        match self {
            Value::Int(v) => v.to_string(),
            Value::Float(v) => fmt_f64(*v),
            Value::Str(s) => s.clone(),
        }
    }
}

fn fmt_f64(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

enum Flow {
    Normal,
    Return(Value),
}

pub struct Compiler<'a> {
    funcs: HashMap<String, &'a Func>,
    scopes: Vec<HashMap<String, Value>>,
    consts: HashSet<String>,
    frame_base: Vec<usize>,
    frame: i64,
    out: Vec<String>,
    last_emitted: i64,
    output_frame: Option<i64>,
    warnings: Vec<String>,
    depth: usize,
    steps: u64,
}

impl<'a> Compiler<'a> {
    pub fn new(program: &'a Program) -> Result<Self, String> {
        let mut funcs = HashMap::new();
        for f in &program.funcs {
            if funcs.insert(f.name.clone(), f).is_some() {
                return Err(format!("line {}: duplicate function '{}'", f.line, f.name));
            }
        }
        Ok(Self {
            funcs,
            scopes: vec![HashMap::new()],
            consts: HashSet::new(),
            frame_base: vec![0],
            frame: 0,
            out: Vec::new(),
            last_emitted: 0,
            output_frame: None,
            warnings: Vec::new(),
            depth: 0,
            steps: 0,
        })
    }

    pub fn compile(mut self, program: &'a Program) -> Result<(Vec<String>, Vec<String>), String> {
        // Top-level statements run directly in the global scope so functions can read them.
        match self.exec_stmts(&program.main)? {
            Flow::Return(_) => {
                return Err("'return' is only allowed inside a function".into());
            }
            Flow::Normal => {}
        }
        Ok((self.out, self.warnings))
    }

    // --- environment -------------------------------------------------------

    fn base(&self) -> usize {
        *self.frame_base.last().unwrap()
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        let base = self.base();
        for scope in self.scopes[base..].iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        self.scopes[0].get(name)
    }

    fn declare(&mut self, name: &str, value: Value, constant: bool, line: usize) -> Result<(), String> {
        let scope = self.scopes.last_mut().unwrap();
        if scope.contains_key(name) {
            return Err(format!(
                "line {line}: '{name}' is already declared in this scope"
            ));
        }
        scope.insert(name.to_string(), value);
        if constant {
            self.consts.insert(name.to_string());
        } else {
            self.consts.remove(name);
        }
        Ok(())
    }

    fn assign(&mut self, name: &str, value: Value, line: usize) -> Result<(), String> {
        if self.consts.contains(name) {
            return Err(format!("line {line}: cannot assign to constant '{name}'"));
        }
        let base = self.base();
        for scope in self.scopes[base..].iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        if self.scopes[0].contains_key(name) {
            self.scopes[0].insert(name.to_string(), value);
            return Ok(());
        }
        Err(format!(
            "line {line}: undefined variable '{name}' (use 'let' to declare it)"
        ))
    }

    // --- execution ---------------------------------------------------------

    fn exec_block(&mut self, body: &'a [Stmt]) -> Result<Flow, String> {
        self.scopes.push(HashMap::new());
        let result = self.exec_stmts(body);
        self.scopes.pop();
        result
    }

    fn exec_stmts(&mut self, body: &'a [Stmt]) -> Result<Flow, String> {
        for stmt in body {
            self.steps += 1;
            if self.steps > MAX_STEPS {
                return Err("script exceeded the maximum number of steps (infinite loop?)".into());
            }
            match self.exec_stmt(stmt)? {
                Flow::Return(v) => return Ok(Flow::Return(v)),
                Flow::Normal => {}
            }
        }
        Ok(Flow::Normal)
    }

    fn exec_stmt(&mut self, stmt: &'a Stmt) -> Result<Flow, String> {
        match stmt {
            Stmt::Import { line, .. } => Err(format!(
                "line {line}: imports must be resolved from a file, not compiled from a source string"
            )),
            Stmt::Let {
                name,
                expr,
                constant,
                line,
            } => {
                let v = self.eval(expr)?;
                self.declare(name, v, *constant, *line)?;
                Ok(Flow::Normal)
            }

            Stmt::Assign { name, expr, line } => {
                let v = self.eval(expr)?;
                self.assign(name, v, *line)?;
                Ok(Flow::Normal)
            }

            Stmt::Wait(expr, line) => {
                let v = self.eval(expr)?.as_f64(*line)?;
                let frames = v.round() as i64;
                if frames < 0 {
                    return Err(format!("line {line}: 'wait' needs a non-negative amount of frames, got {frames}"));
                }
                self.frame += frames;
                Ok(Flow::Normal)
            }

            Stmt::At(expr, line) => {
                let v = self.eval(expr)?.as_f64(*line)?;
                let frame = v.round() as i64;
                if frame < 0 {
                    return Err(format!("line {line}: 'at' needs a non-negative frame, got {frame}"));
                }
                self.frame = frame;
                Ok(Flow::Normal)
            }

            Stmt::Action { name, args, line } => {
                let mut rendered = Vec::with_capacity(args.len());
                for arg in args {
                    rendered.push(match arg {
                        Arg::Word(w) => w.clone(),
                        Arg::Expr(e) => self.eval(e)?.render(),
                    });
                }
                actions::validate(name, &rendered).map_err(|e| format!("line {line}: {e}"))?;
                self.emit(name, &rendered, *line);
                Ok(Flow::Normal)
            }

            Stmt::Call { name, args, line } => {
                self.call(name, args, *line)?;
                Ok(Flow::Normal)
            }

            Stmt::Repeat { count, body, line } => {
                let n = self.eval(count)?.as_f64(*line)?.round() as i64;
                for _ in 0..n.max(0) {
                    self.steps += 1;
                    if self.steps > MAX_STEPS {
                        return Err("script exceeded the maximum number of steps (infinite loop?)".into());
                    }
                    match self.exec_block(body)? {
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Normal => {}
                    }
                }
                Ok(Flow::Normal)
            }

            Stmt::While { cond, body } => {
                loop {
                    self.steps += 1;
                    if self.steps > MAX_STEPS {
                        return Err("script exceeded the maximum number of steps (infinite loop?)".into());
                    }
                    if !self.eval(cond)?.truthy() {
                        break;
                    }
                    match self.exec_block(body)? {
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        Flow::Normal => {}
                    }
                }
                Ok(Flow::Normal)
            }

            Stmt::If {
                cond,
                then_body,
                else_body,
            } => {
                if self.eval(cond)?.truthy() {
                    self.exec_block(then_body)
                } else {
                    self.exec_block(else_body)
                }
            }

            Stmt::Return(expr) => {
                let v = match expr {
                    Some(e) => self.eval(e)?,
                    None => Value::Int(0),
                };
                Ok(Flow::Return(v))
            }

            Stmt::Print(expr) => {
                let v = self.eval(expr)?;
                eprintln!("[print] {}", v.render());
                Ok(Flow::Normal)
            }
        }
    }

    fn emit(&mut self, name: &str, args: &[String], line: usize) {
        let is_await = name == "await";
        if !is_await && self.frame < self.last_emitted {
            self.warnings.push(format!(
                "line {line}: action is emitted at frame {} which is before the previous frame {}",
                self.frame, self.last_emitted
            ));
        }
        self.last_emitted = self.last_emitted.max(self.frame);
        let timing = match self.output_frame {
            None => {
                self.output_frame = Some(self.frame);
                "0".to_string()
            }
            Some(previous) => {
                let delta = self.frame - previous;
                self.output_frame = Some(self.frame);
                if delta >= 0 {
                    format!("+{delta}")
                } else {
                    self.warnings.push(format!(
                        "line {line}: relative output cannot represent a backwards move from frame {previous} to {}",
                        self.frame
                    ));
                    self.frame.to_string()
                }
            }
        };
        let mut out = format!("{timing} {name}");
        for a in args {
            out.push(' ');
            out.push_str(a);
        }
        self.out.push(out);
    }

    fn call(&mut self, name: &str, args: &'a [Expr], line: usize) -> Result<Value, String> {
        let func = *self
            .funcs
            .get(name)
            .ok_or_else(|| format!("line {line}: undefined function '{name}'"))?;

        if args.len() != func.params.len() {
            return Err(format!(
                "line {line}: function '{name}' expects {} argument(s), got {}",
                func.params.len(),
                args.len()
            ));
        }

        let mut values = Vec::with_capacity(args.len());
        for a in args {
            values.push(self.eval(a)?);
        }

        if self.depth + 1 > MAX_CALL_DEPTH {
            return Err(format!(
                "line {line}: maximum call depth ({MAX_CALL_DEPTH}) exceeded in '{name}'"
            ));
        }

        let mut scope = HashMap::new();
        for (p, v) in func.params.iter().zip(values) {
            scope.insert(p.clone(), v);
        }

        self.depth += 1;
        self.frame_base.push(self.scopes.len());
        self.scopes.push(scope);

        let result = self.exec_stmts(&func.body);

        self.scopes.pop();
        self.frame_base.pop();
        self.depth -= 1;

        match result? {
            Flow::Return(v) => Ok(v),
            Flow::Normal => Ok(Value::Int(0)),
        }
    }

    fn eval(&mut self, expr: &'a Expr) -> Result<Value, String> {
        match expr {
            Expr::Int(v) => Ok(Value::Int(*v)),
            Expr::Float(v) => Ok(Value::Float(*v)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),

            Expr::Var(name, line) => self
                .lookup(name)
                .cloned()
                .ok_or_else(|| format!("line {line}: undefined variable '{name}'")),

            Expr::Unary(op, inner) => {
                let v = self.eval(inner)?;
                match op {
                    UnOp::Neg => match v {
                        Value::Int(i) => Ok(Value::Int(-i)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        Value::Str(_) => Err("cannot negate a string".into()),
                    },
                    UnOp::Not => Ok(Value::Int(if v.truthy() { 0 } else { 1 })),
                }
            }

            Expr::Bin(op, lhs, rhs, line) => {
                if *op == BinOp::And {
                    let l = self.eval(lhs)?;
                    if !l.truthy() {
                        return Ok(Value::Int(0));
                    }
                    return Ok(Value::Int(if self.eval(rhs)?.truthy() { 1 } else { 0 }));
                }
                if *op == BinOp::Or {
                    let l = self.eval(lhs)?;
                    if l.truthy() {
                        return Ok(Value::Int(1));
                    }
                    return Ok(Value::Int(if self.eval(rhs)?.truthy() { 1 } else { 0 }));
                }
                let l = self.eval(lhs)?;
                let r = self.eval(rhs)?;
                binop(*op, l, r, *line)
            }

            Expr::Call(name, args, line) => {
                if let Some(v) = self.builtin(name, args, *line)? {
                    return Ok(v);
                }
                self.call(name, args, *line)
            }
        }
    }

    fn builtin(&mut self, name: &str, args: &'a [Expr], line: usize) -> Result<Option<Value>, String> {
        let arity = |n: usize| -> Result<(), String> {
            if args.len() == n {
                Ok(())
            } else {
                Err(format!(
                    "line {line}: '{name}' expects {n} argument(s), got {}",
                    args.len()
                ))
            }
        };

        let v = match name {
            "frame" => {
                arity(0)?;
                Value::Int(self.frame)
            }
            "abs" => {
                arity(1)?;
                match self.eval(&args[0])? {
                    Value::Int(i) => Value::Int(i.abs()),
                    Value::Float(f) => Value::Float(f.abs()),
                    Value::Str(_) => return Err(format!("line {line}: 'abs' expects a number")),
                }
            }
            "int" => {
                arity(1)?;
                Value::Int(self.eval(&args[0])?.as_f64(line)?.trunc() as i64)
            }
            "float" => {
                arity(1)?;
                Value::Float(self.eval(&args[0])?.as_f64(line)?)
            }
            "floor" | "ceil" | "round" | "sqrt" => {
                arity(1)?;
                let x = self.eval(&args[0])?.as_f64(line)?;
                let r = match name {
                    "floor" => x.floor(),
                    "ceil" => x.ceil(),
                    "round" => x.round(),
                    _ => {
                        if x < 0.0 {
                            return Err(format!("line {line}: 'sqrt' of a negative number"));
                        }
                        x.sqrt()
                    }
                };
                if name == "sqrt" {
                    Value::Float(r)
                } else {
                    Value::Int(r as i64)
                }
            }
            "min" | "max" => {
                arity(2)?;
                let a = self.eval(&args[0])?;
                let b = self.eval(&args[1])?;
                let (af, bf) = (a.as_f64(line)?, b.as_f64(line)?);
                let take_first = if name == "min" { af <= bf } else { af >= bf };
                if take_first {
                    a
                } else {
                    b
                }
            }
            "str" => {
                arity(1)?;
                Value::Str(self.eval(&args[0])?.render())
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }
}

fn binop(op: BinOp, l: Value, r: Value, line: usize) -> Result<Value, String> {
    if op == BinOp::Add {
        if let (Value::Str(a), b) = (&l, &r) {
            return Ok(Value::Str(format!("{a}{}", b.render())));
        }
        if let (a, Value::Str(b)) = (&l, &r) {
            return Ok(Value::Str(format!("{}{b}", a.render())));
        }
    }

    if matches!(op, BinOp::Eq | BinOp::Ne) {
        if let (Value::Str(a), Value::Str(b)) = (&l, &r) {
            let eq = a == b;
            return Ok(Value::Int(if (op == BinOp::Eq) == eq { 1 } else { 0 }));
        }
    }

    let (a, b) = (l.as_f64(line)?, r.as_f64(line)?);
    let both_int = matches!(l, Value::Int(_)) && matches!(r, Value::Int(_));

    let num = |v: f64| {
        if both_int && v.fract() == 0.0 {
            Value::Int(v as i64)
        } else {
            Value::Float(v)
        }
    };
    let boolean = |v: bool| Value::Int(if v { 1 } else { 0 });

    Ok(match op {
        BinOp::Add => num(a + b),
        BinOp::Sub => num(a - b),
        BinOp::Mul => num(a * b),
        BinOp::Div => {
            if b == 0.0 {
                return Err(format!("line {line}: division by zero"));
            }
            num(a / b)
        }
        BinOp::Rem => {
            if b == 0.0 {
                return Err(format!("line {line}: modulo by zero"));
            }
            num(a % b)
        }
        BinOp::Eq => boolean(a == b),
        BinOp::Ne => boolean(a != b),
        BinOp::Lt => boolean(a < b),
        BinOp::Le => boolean(a <= b),
        BinOp::Gt => boolean(a > b),
        BinOp::Ge => boolean(a >= b),
        BinOp::And | BinOp::Or => unreachable!(),
    })
}
