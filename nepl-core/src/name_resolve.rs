//! Name resolution for NEPL (no_std, skeleton).
//!
//! This module is responsible for resolving names across namespaces,
//! includes, imports, uses, enums, and structs.
//!
//! 現段階ではまだ実際の解決ロジックは実装せず、
//! インタフェースとデータ構造だけを定義する。

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

use crate::ast;
use crate::diagnostic::Diagnostic;

/// Kinds of symbols that can appear in the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Value,      // variables, functions, constants
    Type,       // enums, structs, type aliases
    Namespace,  // namespace itself
    EnumVariant,
    StructField,
}

/// A resolved symbol reference.
///
/// 後でシンボルテーブルのインデックスやファイル位置などを持たせることを想定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    // TODO: module path, declaration span, etc.
}

/// Result of name resolution on a single AST root expression.
///
/// 現時点では diagnostics だけを返し、
/// 実際の解決結果はほとんど空のままにしてある。
#[derive(Debug)]
pub struct NameResolveResult {
    pub diagnostics: Vec<Diagnostic>,
    // TODO: symbol tables, mapping from AST id -> ResolvedSymbol, etc.
}

/// Perform name resolution on a single AST root expression.
///
/// 将来は複数ファイルを跨いで `include` / `import` を処理するが、
/// ここではひとまず 1 ファイル単位のインタフェースを用意する。
pub fn resolve_names(_root: &ast::Expr) -> NameResolveResult {
    NameResolveResult {
        diagnostics: Vec::new(),
    }
}
