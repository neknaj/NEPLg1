//! Surface AST for NEPL core (no_std).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;

/// Identifier in the surface AST.
///
/// The parser does not decide whether an identifier refers to a
/// variable, function, type, or namespace. That resolution happens
/// in later phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// Literal values as they appear in source code.
///
/// Numeric literals are kept in textual form so that later phases
/// can decide how to interpret them (`i32` vs `i64`, `f32` vs `f64`)
/// according to the type system rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralKind {
    Int(String),
    Float(String),
    Bool(bool),
    String(String),
}

/// Expression node in the surface AST.
///
/// Each expression carries its own span. The `kind` describes the
/// syntactic form; semantic decisions (such as P-style function call
/// resolution) are deferred to later phases.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// Surface expression variants.
///
/// Note that there is deliberately **no** dedicated `Call` variant.
/// P-style prefix sequences are represented by `Seq`, and the
/// type-checking / P-style resolution phase is responsible for
/// turning those into call trees.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A P-style prefix sequence: `term0 term1 ... termN`.
    ///
    /// The parser does not decide which term is the function and
    /// which ones are arguments. That is determined later.
    Seq(Vec<Expr>),

    /// Parenthesised expression: `(expr)`.
    Paren(Box<Expr>),

    /// Pipe chain `lhs > rhs1 > rhs2 > ...`.
    ///
    /// According to the specification, this is sugar for repeated
    /// function application, but the exact desugaring is performed
    /// in a later phase.
    Pipe(PipeChain),

    /// Literal value.
    Literal(LiteralKind),

    /// Identifier.
    Ident(Ident),

    /// If expression.
    If(IfExpr),

    /// Loop expression (`loop`).
    Loop(LoopExpr),

    /// While expression (`while`).
    While(WhileExpr),

    /// Match expression.
    Match(MatchExpr),

    /// Block expression: `{ e1; e2; ...; en }`.
    Block(BlockExpr),

    /// Variable binding: `let`, `let mut`, `let hoist`, possibly `pub`.
    Let(LetExpr),

    /// Function binding: `fn name = expr`.
    LetFunction(LetFunctionExpr),

    /// Include expression: `include "path"`.
    Include(IncludeExpr),

    /// Import expression: `import name`.
    Import(ImportExpr),

    /// Namespace expression: `namespace` / `pub namespace`.
    Namespace(NamespaceExpr),

    /// Use expression: `use` / `pub use`.
    Use(UseExpr),

    /// When expression: compile-time conditional.
    When(WhenExpr),

    /// Return expression: `return` or `return expr`.
    Return(ReturnExpr),

    /// Break expression: `break` or `break expr`.
    Break(BreakExpr),

    /// Continue expression: `continue`.
    Continue(ContinueExpr),

    /// Assignment: `set target expr`.
    Set(SetExpr),

    /// Enum definition expression.
    EnumDef(EnumDefExpr),

    /// Struct definition expression.
    StructDef(StructDefExpr),
}

/// Pipe chain representation.
#[derive(Debug, Clone, PartialEq)]
pub struct PipeChain {
    pub first: Box<Expr>,
    pub rest: Vec<Expr>,
}

/// If expression: if / elseif* / else.
#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub if_branch: IfBranch,
    pub elseif_branches: Vec<IfBranch>,
    pub else_branch: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfBranch {
    pub condition: Box<Expr>,
    pub body: Box<Expr>,
}

/// Loop expression: `loop <body>`.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopExpr {
    pub body: Box<Expr>,
}

/// While expression: `while cond body`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhileExpr {
    pub condition: Box<Expr>,
    pub body: Box<Expr>,
}

/// Match expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub cases: ScopedList<MatchCase>,
}

/// A single match case: `case pattern => body`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub pattern: Pattern,
    pub body: Box<Expr>,
    pub span: Span,
}

/// Block expression: a sequence of expressions evaluated in order.
///
/// The block's result type is the type of the last expression.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockExpr {
    pub expressions: Vec<Expr>,
}

/// Scope kind for scoped expressions and scoped lists.
///
/// Braced: `{ ... }`
/// Offside: `: ...` with indentation-based block (planned; parser
/// currently constructs only `Braced`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Braced,
    Offside,
}

/// Generic scoped list, used for match cases, enum variants, and
/// struct fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopedList<T> {
    pub items: Vec<T>,
    pub kind: ScopeKind,
    pub span: Span,
}

/// Let expression: `let` / `let mut` / `let hoist` with optional `pub`.
#[derive(Debug, Clone, PartialEq)]
pub struct LetExpr {
    pub is_pub: bool,
    pub is_mut: bool,
    pub is_hoist: bool,
    pub name: Ident,
    pub value: Box<Expr>,
    pub span: Span,
}

/// Let-function expression: `fn name = expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct LetFunctionExpr {
    pub is_pub: bool,
    pub name: Ident,
    pub value: Box<Expr>,
    pub span: Span,
}

/// Include expression: `include "path"`.
#[derive(Debug, Clone, PartialEq)]
pub struct IncludeExpr {
    pub path: String, // typically a string literal
    pub span: Span,
}

/// Import expression: `import name`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportExpr {
    pub name: Ident,
    pub span: Span,
}

/// Namespace expression.
#[derive(Debug, Clone, PartialEq)]
pub struct NamespaceExpr {
    pub is_pub: bool,
    pub name: Ident,
    pub body: Box<Expr>, // usually a block / scoped list
    pub span: Span,
}

/// Use expression.
#[derive(Debug, Clone, PartialEq)]
pub struct UseExpr {
    pub is_pub: bool,
    pub path: Path,
    pub alias: Option<Ident>,
    pub span: Span,
}

/// Use path: `ident :: ident :: ...` with optional trailing glob.
#[derive(Debug, Clone, PartialEq)]
pub enum Path {
    Simple { segments: Vec<String> },
    Glob { segments: Vec<String> },
}

/// When expression: `when (expr) body`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhenExpr {
    pub condition: Box<Expr>,
    pub body: Box<Expr>, // scoped expression
    pub span: Span,
}

/// Return expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnExpr {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

/// Break expression.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakExpr {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

/// Continue expression.
#[derive(Debug, Clone, PartialEq)]
pub struct ContinueExpr {
    pub span: Span,
}

/// Assignable target used by `set`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignable {
    pub base: Box<Expr>,
    pub fields: Vec<Ident>,
}

/// Set expression: `set target expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct SetExpr {
    pub target: Assignable,
    pub value: Box<Expr>,
    pub span: Span,
}

/// Enum definition expression.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDefExpr {
    pub is_pub: bool,
    pub name: Ident,
    pub variants: ScopedList<EnumVariant>,
    pub span: Span,
}

/// A single enum variant: `Name` or `Name(T1, T2, ...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    pub payload_types: Vec<TypeExpr>,
    pub span: Span,
}

/// Struct definition expression.
#[derive(Debug, Clone, PartialEq)]
pub struct StructDefExpr {
    pub is_pub: bool,
    pub name: Ident,
    pub fields: ScopedList<StructField>,
    pub span: Span,
}

/// Struct field: `name: Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: Ident,
    pub ty: TypeExpr,
    pub span: Span,
}

/// Parsed type expression used by enum payloads and struct fields.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub span: Span,
    pub kind: TypeExprKind,
}

/// Kinds of parsed type expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExprKind {
    Named(String),
    Fn(Vec<TypeExpr>, Box<TypeExpr>, Box<TypeExpr>),
    Paren(Box<TypeExpr>),
}

/// Patterns used in match expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(Expr),
    Ident(String, Span),
    Wildcard(Span),
    Enum(EnumPattern),
    Struct(StructPattern),
}

/// Single field pattern in a struct pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct StructPatternField {
    pub field: String,
    pub pattern: Pattern,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumPattern {
    pub ctor: String,
    pub args: Vec<Pattern>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructPattern {
    pub ctor: String,
    pub fields: Vec<StructPatternField>,
    pub span: Span,
}
