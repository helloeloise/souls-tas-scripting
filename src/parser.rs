use crate::actions;
use crate::ast::*;
use crate::lexer::{Tok, Token};

const KEYWORDS: &[&str] = &[
    "import", "let", "const", "fn", "wait", "at", "repeat", "while", "if", "else", "return", "print", "true",
    "false",
];

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        let i = (self.pos + n).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn line(&self) -> usize {
        self.toks[self.pos].line
    }

    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.next();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: Tok, what: &str) -> Result<(), String> {
        if self.eat(&t) {
            Ok(())
        } else {
            Err(format!(
                "line {}: expected {what}, found {:?}",
                self.line(),
                self.peek()
            ))
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<String, String> {
        match self.peek().clone() {
            Tok::Ident(name) => {
                self.next();
                Ok(name)
            }
            other => Err(format!(
                "line {}: expected {what}, found {other:?}",
                self.line()
            )),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.next();
        }
    }

    fn end_of_stmt(&mut self) -> Result<(), String> {
        match self.peek() {
            Tok::Newline => {
                self.next();
                Ok(())
            }
            Tok::Eof | Tok::RBrace => Ok(()),
            other => Err(format!(
                "line {}: unexpected {other:?} at end of statement",
                self.line()
            )),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut prog = Program {
            funcs: Vec::new(),
            main: Vec::new(),
        };
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::Eof) {
                break;
            }
            if matches!(self.peek(), Tok::Ident(k) if k == "fn") {
                prog.funcs.push(self.parse_func()?);
            } else {
                prog.main.push(self.parse_stmt()?);
            }
        }
        Ok(prog)
    }

    fn parse_func(&mut self) -> Result<Func, String> {
        let line = self.line();
        self.next(); // fn
        let name = self.expect_ident("function name")?;
        if KEYWORDS.contains(&name.as_str()) || actions::is_action(&name) {
            return Err(format!("line {line}: '{name}' is reserved and cannot be a function name"));
        }
        self.expect(Tok::LParen, "'(' after function name")?;
        let mut params = Vec::new();
        if !self.eat(&Tok::RParen) {
            loop {
                self.skip_newlines();
                params.push(self.expect_ident("parameter name")?);
                self.skip_newlines();
                if self.eat(&Tok::Comma) {
                    continue;
                }
                self.expect(Tok::RParen, "',' or ')' in parameter list")?;
                break;
            }
        }
        let body = self.parse_block()?;
        Ok(Func {
            name,
            params,
            body,
            line,
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.skip_newlines();
        self.expect(Tok::LBrace, "'{'")?;
        let mut body = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                Tok::RBrace => {
                    self.next();
                    break;
                }
                Tok::Eof => return Err(format!("line {}: unclosed block, expected '}}'", self.line())),
                _ => body.push(self.parse_stmt()?),
            }
        }
        Ok(body)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        let line = self.line();
        let name = match self.peek().clone() {
            Tok::Ident(name) => name,
            other => {
                return Err(format!(
                    "line {line}: expected a statement, found {other:?}"
                ))
            }
        };

        match name.as_str() {
            "import" => {
                self.next();
                let path = match self.next() {
                    Tok::Str(path) => path,
                    other => {
                        return Err(format!(
                            "line {line}: expected a quoted import path, found {other:?}"
                        ))
                    }
                };
                self.end_of_stmt()?;
                Ok(Stmt::Import { path, line })
            }
            "let" | "const" => {
                self.next();
                let var = self.expect_ident("variable name")?;
                self.expect(Tok::Assign, "'=' in declaration")?;
                let expr = self.parse_expr()?;
                self.end_of_stmt()?;
                Ok(Stmt::Let {
                    name: var,
                    expr,
                    constant: name == "const",
                    line,
                })
            }
            "wait" => {
                self.next();
                let e = self.parse_expr()?;
                self.end_of_stmt()?;
                Ok(Stmt::Wait(e, line))
            }
            "at" => {
                self.next();
                let e = self.parse_expr()?;
                self.end_of_stmt()?;
                Ok(Stmt::At(e, line))
            }
            "repeat" => {
                self.next();
                let count = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::Repeat { count, body, line })
            }
            "while" => {
                self.next();
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::While { cond, body })
            }
            "if" => self.parse_if(),
            "return" => {
                self.next();
                let value = if matches!(self.peek(), Tok::Newline | Tok::Eof | Tok::RBrace) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.end_of_stmt()?;
                Ok(Stmt::Return(value))
            }
            "print" => {
                self.next();
                let e = self.parse_expr()?;
                self.end_of_stmt()?;
                Ok(Stmt::Print(e))
            }
            "fn" => Err(format!(
                "line {line}: functions can only be declared at the top level"
            )),
            _ => {
                if matches!(self.peek_at(1), Tok::LParen) && !actions::is_action(&name) {
                    self.next();
                    let args = self.parse_call_args()?;
                    self.end_of_stmt()?;
                    return Ok(Stmt::Call { name, args, line });
                }
                if matches!(self.peek_at(1), Tok::Assign) {
                    self.next();
                    self.next();
                    let expr = self.parse_expr()?;
                    self.end_of_stmt()?;
                    return Ok(Stmt::Assign { name, expr, line });
                }
                if !actions::is_action(&name) {
                    return Err(format!(
                        "line {line}: unknown statement or action '{name}'"
                    ));
                }
                self.next();
                let args = self.parse_action_args()?;
                self.end_of_stmt()?;
                Ok(Stmt::Action { name, args, line })
            }
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        self.next(); // if
        let cond = self.parse_expr()?;
        let then_body = self.parse_block()?;
        let mut else_body = Vec::new();
        let save = self.pos;
        self.skip_newlines();
        if matches!(self.peek(), Tok::Ident(k) if k == "else") {
            self.next();
            if matches!(self.peek(), Tok::Ident(k) if k == "if") {
                else_body.push(self.parse_if()?);
            } else {
                else_body = self.parse_block()?;
            }
        } else {
            self.pos = save;
        }
        Ok(Stmt::If {
            cond,
            then_body,
            else_body,
        })
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, String> {
        self.expect(Tok::LParen, "'('")?;
        let mut args = Vec::new();
        self.skip_newlines();
        if self.eat(&Tok::RParen) {
            return Ok(args);
        }
        loop {
            self.skip_newlines();
            args.push(self.parse_expr()?);
            self.skip_newlines();
            if self.eat(&Tok::Comma) {
                continue;
            }
            self.expect(Tok::RParen, "',' or ')' in argument list")?;
            break;
        }
        Ok(args)
    }

    fn parse_action_args(&mut self) -> Result<Vec<Arg>, String> {
        let mut args = Vec::new();
        loop {
            match self.peek().clone() {
                Tok::Newline | Tok::Eof | Tok::RBrace => break,
                Tok::Dollar | Tok::Int(_) | Tok::Float(_) | Tok::Str(_) | Tok::LParen
                | Tok::Minus => {
                    args.push(Arg::Expr(self.parse_expr()?));
                }
                Tok::Ident(name) => {
                    if matches!(self.peek_at(1), Tok::LParen) {
                        args.push(Arg::Expr(self.parse_expr()?));
                    } else {
                        self.next();
                        args.push(Arg::Word(name));
                    }
                }
                other => {
                    return Err(format!(
                        "line {}: unexpected {other:?} in action arguments",
                        self.line()
                    ))
                }
            }
        }
        Ok(args)
    }

    // --- expressions -------------------------------------------------------

    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek(), Tok::OrOr) {
            let line = self.line();
            self.next();
            let rhs = self.parse_and()?;
            lhs = Expr::Bin(BinOp::Or, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_equality()?;
        while matches!(self.peek(), Tok::AndAnd) {
            let line = self.line();
            self.next();
            let rhs = self.parse_equality()?;
            lhs = Expr::Bin(BinOp::And, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Tok::Eq => BinOp::Eq,
                Tok::Ne => BinOp::Ne,
                _ => break,
            };
            let line = self.line();
            self.next();
            let rhs = self.parse_comparison()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Tok::Lt => BinOp::Lt,
                Tok::Le => BinOp::Le,
                Tok::Gt => BinOp::Gt,
                Tok::Ge => BinOp::Ge,
                _ => break,
            };
            let line = self.line();
            self.next();
            let rhs = self.parse_additive()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            let line = self.line();
            self.next();
            let rhs = self.parse_multiplicative()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => break,
            };
            let line = self.line();
            self.next();
            let rhs = self.parse_unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs), line);
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Tok::Minus => {
                self.next();
                Ok(Expr::Unary(UnOp::Neg, Box::new(self.parse_unary()?)))
            }
            Tok::Bang => {
                self.next();
                Ok(Expr::Unary(UnOp::Not, Box::new(self.parse_unary()?)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let line = self.line();
        match self.peek().clone() {
            Tok::Int(v) => {
                self.next();
                Ok(Expr::Int(v))
            }
            Tok::Float(v) => {
                self.next();
                Ok(Expr::Float(v))
            }
            Tok::Str(s) => {
                self.next();
                Ok(Expr::Str(s))
            }
            Tok::Dollar => {
                self.next();
                let name = self.expect_ident("variable name after '$'")?;
                Ok(Expr::Var(name, line))
            }
            Tok::LParen => {
                self.next();
                let e = self.parse_expr()?;
                self.expect(Tok::RParen, "')'")?;
                Ok(e)
            }
            Tok::Ident(name) => {
                self.next();
                match name.as_str() {
                    "true" => return Ok(Expr::Int(1)),
                    "false" => return Ok(Expr::Int(0)),
                    _ => {}
                }
                if matches!(self.peek(), Tok::LParen) {
                    let args = self.parse_call_args()?;
                    Ok(Expr::Call(name, args, line))
                } else {
                    Ok(Expr::Var(name, line))
                }
            }
            other => Err(format!("line {line}: expected an expression, found {other:?}")),
        }
    }
}
