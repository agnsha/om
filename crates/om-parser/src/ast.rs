//! The abstract syntax tree produced by the parser.

use om_lexer::Span;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    pub functions: Vec<Function>,
    pub structs: Vec<StructDef>,
    pub enums: Vec<EnumDef>,
    pub impls: Vec<ImplBlock>,
    pub consts: Vec<ConstDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    /// `None` means the function returns nothing (unit).
    pub ret: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
}

/// How a parameter receives its argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    /// `x: T`: the value is moved or copied in.
    Value,
    /// `x: &T`: read-only access to the caller's value.
    Ref,
    /// `x: &mut T`: the callee may change the caller's value.
    RefMut,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    /// `None` only for `self`, whose type is the `impl` type.
    pub ty: Option<TypeExpr>,
    pub mode: ParamMode,
    /// `mut x: T` makes the local copy mutable.
    pub mutable: bool,
    /// Default value used when a call leaves this argument out: `count: i64 = 1`.
    pub default: Option<Expr>,
    pub span: Span,
}

/// A type as written in source, e.g. `i64` or `Point`. Resolved later by the checker.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, TypeExpr)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<Variant>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    /// Payload types of a tuple variant like `Circle(f64)`; empty for `Red`.
    pub fields: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    pub ty: TypeExpr,
    pub methods: Vec<Function>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDef {
    pub name: String,
    pub ty: TypeExpr,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Final expression without a trailing `;`: the block's value.
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        mutable: bool,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    /// `target = value`, or `target += value` when `op` is set.
    Assign {
        target: Expr,
        op: Option<BinOp>,
        value: Expr,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    /// `for var in start..end` (or `..=` when `inclusive`).
    For {
        var: String,
        start: Expr,
        end: Expr,
        inclusive: bool,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        value: Option<Expr>,
        span: Span,
    },
    Continue {
        span: Span,
    },
    /// `defer expr`: run `expr` when the enclosing block exits, in reverse order.
    Defer {
        body: Expr,
        span: Span,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// A name, or a path such as `Color::Red`.
    Var(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// A call of a function or path, such as `f(x)`, `Point::new(1, 2)` or `Shape::Circle(1.0)`.
    Call(String, Vec<Arg>),
    /// `receiver.method(args)`
    MethodCall(Box<Expr>, String, Vec<Arg>),
    /// `value.field`
    Field(Box<Expr>, String),
    /// `Point { x: 1, y: 2 }`
    StructLit(String, Vec<(String, Expr)>),
    /// `&x` or `&mut x` passed as an argument.
    Ref(bool, Box<Expr>),
    /// `value as Type`
    Cast(Box<Expr>, TypeExpr),
    /// An interpolated string: `"x = {x}"`.
    Interp(Vec<InterpPart>),
    If(Box<Expr>, Block, Option<Block>),
    Match(Box<Expr>, Vec<Arm>),
    Loop(Block),
    Block(Block),
}

/// A call argument, optionally named: `f(1, width: 2)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpPart {
    Lit(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub pat: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub kind: PatKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatKind {
    /// `_`
    Wild,
    /// A lowercase name binds the matched value.
    Bind(String),
    Int(i64),
    Bool(bool),
    /// `1..=5` or `0..10`
    Range(i64, i64, bool),
    /// `Shape::Circle(r)` or `Color::Red`
    Variant(String, Vec<Pattern>),
    /// `A | B`
    Or(Vec<Pattern>),
}
