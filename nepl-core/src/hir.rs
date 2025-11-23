//! High-level intermediate representation (HIR) for NEPL (no_std).
//!
//! HIR is the typed, desugared form that later phases of the
//! compiler will consume. It is separate from the current untyped
//! AST so that the implementation can evolve toward the full
//! language described in the design documents.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;
use crate::types::{ArrowKind, Type};

/// Identifier in HIR.
///
/// For now this is just a string plus a span, but in later stages it
/// can be extended with symbol table indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirIdent {
    pub name: String,
    pub span: Span,
}

/// Function parameter in HIR.
#[derive(Debug, Clone, PartialEq)]
pub struct HirParam {
    pub name: HirIdent,
    pub ty: Type,
    /// Whether the parameter is mutable.
    pub mutable: bool,
}

/// Represents a single function in HIR.
///
/// Overloaded functions will be represented as sets of HirFunction
/// values associated with the same name at a higher level.
#[derive(Debug, Clone, PartialEq)]
pub struct HirFunction {
    pub name: HirIdent,
    pub params: Vec<HirParam>,
    pub result: Type,
    pub arrow: ArrowKind,
    pub body: HirExpr,
}

/// A HIR module / compilation unit.
///
/// Laterこの中に `enum` や `struct`、トップレベル `let` なども入れていく想定。
#[derive(Debug, Clone, PartialEq)]
pub struct HirModule {
    pub functions: Vec<HirFunction>,
    // TODO: enums, structs, global values, namespaces
}

/// Assignable expressions used by `set`.
///
/// For now, this supports variables and simple field access paths.
#[derive(Debug, Clone, PartialEq)]
pub struct HirAssignable {
    pub base: Box<HirExpr>,
    pub fields: Vec<HirIdent>,
}

/// Expression node in HIR.
///
/// `HirExpr` は必ず `Type` と `Span` を持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct HirExpr {
    pub kind: HirExprKind,
    pub ty: Type,
    pub span: Span,
}

/// HIR expression kind.
///
/// この段階では P-style の解決やオーバーロード解決が終わっている前提。
#[derive(Debug, Clone, PartialEq)]
pub enum HirExprKind {
    // Literals
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Unit,
    String(String),

    // Variable reference
    Var(HirIdent),

    // Function call
    Call {
        callee: HirIdent,
        args: Vec<HirExpr>,
    },

    // let / let mut
    Let {
        name: HirIdent,
        mutable: bool,
        init: Box<HirExpr>,
        body: Box<HirExpr>,
    },

    // set expression (assignment); result typeは Unit のはず
    Set {
        target: Box<HirAssignable>,
        value: Box<HirExpr>,
    },

    // if expression
    If {
        cond: Box<HirExpr>,
        then_branch: Box<HirExpr>,
        else_branch: Box<HirExpr>,
    },

    // while loop (usually Unit)
    While {
        cond: Box<HirExpr>,
        body: Box<HirExpr>,
    },

    // typed loop expression
    Loop {
        body: Box<HirExpr>,
    },

    // break / break expr / continue / return (Never 型)
    Break {
        value: Option<Box<HirExpr>>,
    },

    Continue,

    Return {
        value: Option<Box<HirExpr>>,
    },

    // match expression
    Match {
        scrutinee: Box<HirExpr>,
        arms: Vec<HirMatchArm>,
    },

    // Block expression: sequence of expressions; value = last expr.
    Block {
        expressions: Vec<HirExpr>,
    },
}

/// A single match arm: `pattern => expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub body: HirExpr,
}

/// Patterns for match expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum HirPattern {
    // literal patterns
    LitI32(i32),
    LitI64(i64),
    LitF32(f32),
    LitF64(f64),
    LitBool(bool),
    LitString(String),

    // identifier pattern (binds a new variable)
    Ident(HirIdent),

    // wildcard `_`
    Wildcard(Span),

    // enum variant patterns: Variant(p1, p2, ...)
    EnumVariant {
        ctor: HirIdent,
        args: Vec<HirPattern>,
    },

    // struct patterns: Point { x: x1, y: _ }
    Struct {
        ctor: HirIdent,
        fields: Vec<HirStructPatternField>,
    },
}

/// Field pattern in a struct pattern: `field_name: pattern`.
#[derive(Debug, Clone, PartialEq)]
pub struct HirStructPatternField {
    pub field: HirIdent,
    pub pattern: HirPattern,
}

impl HirExpr {
    /// Helper to create a Never-typed break expression.
    pub fn never_break(span: Span, value: Option<HirExpr>) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Break {
                value: value.map(Box::new),
            },
            ty: Type::Never,
            span,
        }
    }

    /// Helper to create a Never-typed continue expression.
    pub fn never_continue(span: Span) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Continue,
            ty: Type::Never,
            span,
        }
    }

    /// Helper to create a Never-typed return expression.
    pub fn never_return(span: Span, value: Option<HirExpr>) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Return {
                value: value.map(Box::new),
            },
            ty: Type::Never,
            span,
        }
    }
}
