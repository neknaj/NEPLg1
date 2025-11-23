//! LLVM backend for NEPL core (no_std, stub).
//!
//! This module currently provides only a stub implementation that
//! returns a placeholder LLVM IR string. A real backend can be
//! implemented later or in a separate crate.

#![allow(dead_code)]

use alloc::string::String;

use crate::hir::HirExpr;

/// Generate LLVM IR for the given HIR expression.
///
/// 現在はまだ LLVM バックエンドが未実装なので、
/// プレースホルダのコメントのみを返す。
pub fn generate_llvm_ir(_entry: &HirExpr) -> String {
    "; LLVM backend is not implemented yet.\n".into()
}
