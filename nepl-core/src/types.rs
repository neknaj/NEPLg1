//! Core type system for the NEPL language (no_std).
//!
//! This module defines the built-in types, user-defined types,
//! and function types described in the language plans. It is
//! intentionally self-contained and does not depend on parsing
//! or code generation.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

/// ArrowKind distinguishes ordinary and pure function types.
///
/// (T1, ..., Tn) -> R  : impure function
/// (T1, ..., Tn) *> R  : pure function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowKind {
    Impure,
    Pure,
}

/// Represents the types of values and expressions in NEPL.
///
/// This enum mirrors the specification in the design documents
/// but keeps the representation simple enough for initial
/// implementation and testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    // Built-in primitive types
    I32,
    I64,
    F32,
    F64,
    Bool,
    Unit,
    /// Bottom type used for expressions that never return
    /// (return, break, continue, etc.).
    Never,

    /// User-defined named types (enum, struct, etc.).
    ///
    /// In later phases, this will be tied to symbol table entries.
    Named(String),

    /// Function types: (T1, ..., Tn) -> R or (T1, ..., Tn) *> R.
    Function {
        params: Vec<Type>,
        result: alloc::boxed::Box<Type>,
        arrow: ArrowKind,
    },
}

impl Type {
    /// Returns true if this type is the Never type.
    pub fn is_never(&self) -> bool {
        matches!(self, Type::Never)
    }

    /// Returns true if this type is the Unit type.
    pub fn is_unit(&self) -> bool {
        matches!(self, Type::Unit)
    }

    /// Construct an impure function type.
    pub fn impure_function(params: Vec<Type>, result: Type) -> Type {
        Type::Function {
            params,
            result: alloc::boxed::Box::new(result),
            arrow: ArrowKind::Impure,
        }
    }

    /// Construct a pure function type.
    pub fn pure_function(params: Vec<Type>, result: Type) -> Type {
        Type::Function {
            params,
            result: alloc::boxed::Box::new(result),
            arrow: ArrowKind::Pure,
        }
    }
}

/// Result of a subtyping check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtypeResult {
    /// Left is a strict subtype of right.
    Strict,
    /// Left and right are equal types.
    Equal,
    /// Left is not a subtype of right.
    NotSubtype,
}

/// Check whether `left` is a subtype of `right`.
///
/// 最重要ルール:
/// * Never <: T for all T
///
/// 関数型は構造的に比較します (現段階では「同じ型ならOK」という単純な扱い)。
pub fn is_subtype(left: &Type, right: &Type) -> SubtypeResult {
    use SubtypeResult::*;

    // Never is bottom: Never <: T
    if matches!(left, Type::Never) {
        if matches!(right, Type::Never) {
            return Equal;
        }
        return Strict;
    }

    // Reflexivity
    if left == right {
        return Equal;
    }

    match (left, right) {
        // Non-Never primitives and Unit: only equal to themselves (handled above).
        (Type::I32, _)
        | (Type::I64, _)
        | (Type::F32, _)
        | (Type::F64, _)
        | (Type::Bool, _)
        | (Type::Unit, _)
        | (Type::Named(_), _) => NotSubtype,

        // Function subtyping can be refined later if needed
        // (contra-variance in params, co-variance in result).
        (
            Type::Function {
                params: lp,
                result: lr,
                arrow: la,
            },
            Type::Function {
                params: rp,
                result: rr,
                arrow: ra,
            },
        ) => {
            if la != ra || lp.len() != rp.len() {
                return NotSubtype;
            }

            // Simple structural equality for now: all params and result must match.
            for (lt, rt) in lp.iter().zip(rp.iter()) {
                if is_subtype(lt, rt) != Equal {
                    return NotSubtype;
                }
            }
            if is_subtype(lr, rr) != Equal {
                return NotSubtype;
            }
            Strict
        }

        // Any other combination is not a subtype.
        _ => NotSubtype,
    }
}

/// Compute the least common supertype (LCS) of two types, if it exists.
///
/// This is used for typing `if` and `match` expressions, where
/// Never is treated as bottom.
///
/// * lcs(Never, T) = T
/// * lcs(T, Never) = T
/// * lcs(T, T)     = T
///
/// For now, this function supports a few simple cases and returns
/// None when a unique common supertype cannot be determined.
pub fn least_common_supertype(a: &Type, b: &Type) -> Option<Type> {
    use Type::*;

    // Handle Never bottom rules first.
    if matches!(a, Never) {
        return Some(b.clone());
    }
    if matches!(b, Never) {
        return Some(a.clone());
    }

    // Identical types
    if a == b {
        return Some(a.clone());
    }

    // No implicit numeric promotions or other hierarchies yet.
    match (a, b) {
        (Unit, _) | (_, Unit) => None,
        (Bool, _) | (_, Bool) => None,
        (Named(_), _) | (_, Named(_)) => None,
        (
            Function {
                params: ap,
                result: ar,
                arrow: aa,
            },
            Function {
                params: bp,
                result: br,
                arrow: ba,
            },
        ) => {
            if aa != ba || ap.len() != bp.len() {
                return None;
            }
            // Require identical param and result types for now.
            if ap == bp && ar == br {
                Some(a.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
