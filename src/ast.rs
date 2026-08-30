#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Var(String, usize),
    Unary(UnOp, Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>, usize),
    Call(String, Vec<Expr>, usize),
}

/// An argument of an in-game action line: either a bare literal word
/// (`down`, `w`, `stick_right_x`) or an evaluated expression (`$dir`, `10 * 2`).
#[derive(Debug, Clone)]
pub enum Arg {
    Word(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Import {
        path: String,
        line: usize,
    },
    Let {
        name: String,
        expr: Expr,
        constant: bool,
        line: usize,
    },
    Assign {
        name: String,
        expr: Expr,
        line: usize,
    },
    Wait(Expr, usize),
    At(Expr, usize),
    Action {
        name: String,
        args: Vec<Arg>,
        line: usize,
    },
    Call {
        name: String,
        args: Vec<Expr>,
        line: usize,
    },
    Repeat {
        count: Expr,
        body: Vec<Stmt>,
        line: usize,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    Print(Expr),
}

#[derive(Debug, Clone)]
pub struct Func {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub funcs: Vec<Func>,
    pub main: Vec<Stmt>,
}
