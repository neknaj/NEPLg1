//! Compiler orchestration for NEPL core (no_std).
//!
//! This module wires together the frontend (lexer, parser),
//! semantic layers (type checking / HIR), and backends (wasm / LLVM).
//!
//! ファイル I/O は一切行わず、呼び出し側から source 文字列と
//! `FileId` を受け取るだけにする。

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

use crate::codegen_llvm;
use crate::codegen_wasm;
use crate::diagnostic::{Diagnostic, Severity};
use crate::error::CoreError;
use crate::lexer;
use crate::parser;
use crate::span::FileId;
use crate::typecheck;

/// Result of a successful compilation.
///
/// 現時点では wasm バイト列だけを持つが、
/// 後でメタデータなどを追加できるよう構造体にしてある。
#[derive(Debug, Clone)]
pub struct CompilationArtifact {
    /// The compiled wasm module bytes.
    pub wasm: Vec<u8>,
}

/// Compile a NEPL source file into a wasm module.
///
/// This function runs the full pipeline:
///   lex -> parse -> typecheck -> HIR -> wasm codegen
///
/// どこかの段階でエラーが1つでも発生した場合、
/// それまでに収集した `Diagnostic` を `CoreError::Diagnostics` として返す。
pub fn compile_wasm(file_id: FileId, source: &str) -> Result<CompilationArtifact, CoreError> {
    // 1. Lexing
    let lex_result = lexer::lex(file_id, source);

    // 2. Parsing (from existing lex result to avoid re-lexing)
    let parse_result = parser::parse_tokens(source, &lex_result);

    // AST が取れないレベルでの致命的エラー
    let ast = match parse_result.expr {
        Some(ref e) => e,
        None => {
            return Err(CoreError::from_diagnostics(parse_result.diagnostics));
        }
    };

    // 3. Type checking → HIR
    let typecheck_result = typecheck::typecheck_expr(ast);

    // ここまでの diagnostics を全部まとめる
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    diagnostics.extend(lex_result.diagnostics.into_iter());
    diagnostics.extend(parse_result.diagnostics.into_iter());
    diagnostics.extend(typecheck_result.diagnostics.into_iter());

    // HIR が得られない、またはエラーが存在するならここで終了
    if has_error(&diagnostics) || typecheck_result.expr.is_none() {
        return Err(CoreError::from_diagnostics(diagnostics));
    }

    let hir_entry = typecheck_result.expr.as_ref().unwrap();

    // 4. WASM codegen
    match codegen_wasm::generate_wasm(hir_entry) {
        Ok(bytes) => Ok(CompilationArtifact { wasm: bytes }),
        Err(mut cg_diags) => {
            diagnostics.append(&mut cg_diags);
            Err(CoreError::from_diagnostics(diagnostics))
        }
    }
}

/// Compile a NEPL source file into LLVM IR (stub).
///
/// パイプラインは `compile_wasm` と同じ lex → parse → typecheck までを共有し、
/// 最後に `codegen_llvm::generate_llvm_ir` を呼び出す。
pub fn emit_llvm_ir(file_id: FileId, source: &str) -> Result<String, CoreError> {
    let lex_result = lexer::lex(file_id, source);
    let parse_result = parser::parse_tokens(source, &lex_result);

    let ast = match parse_result.expr {
        Some(ref e) => e,
        None => {
            return Err(CoreError::from_diagnostics(parse_result.diagnostics));
        }
    };

    let typecheck_result = typecheck::typecheck_expr(ast);

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    diagnostics.extend(lex_result.diagnostics.into_iter());
    diagnostics.extend(parse_result.diagnostics.into_iter());
    diagnostics.extend(typecheck_result.diagnostics.into_iter());

    if has_error(&diagnostics) || typecheck_result.expr.is_none() {
        return Err(CoreError::from_diagnostics(diagnostics));
    }

    let hir_entry = typecheck_result.expr.as_ref().unwrap();
    let ir = codegen_llvm::generate_llvm_ir(hir_entry);
    Ok(ir)
}

/// Returns true if any of the diagnostics is an error.
fn has_error(diags: &[Diagnostic]) -> bool {
    diags
        .iter()
        .any(|d| matches!(d.severity, Severity::Error))
}
