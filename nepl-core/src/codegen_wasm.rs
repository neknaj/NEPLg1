//! WASM backend for NEPL core (no_std).
//!
//! This module translates typed HIR into a minimal wasm module using
//! the `wasm-encoder` crate. For now, it supports only a very small
//! subset of HIR (e.g. a single i32 literal as the entry expression)
//! and returns diagnostics for unsupported constructs.

#![allow(dead_code)]

use alloc::vec::Vec;

use crate::diagnostic::Diagnostic;
use crate::hir::{HirExpr, HirExprKind};
use crate::span::Span;
use crate::types::Type;

// NOTE: this assumes the `wasm-encoder` crate is available.
// The exact version and API may need adjustment in the real project.
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection, ValType,
};

/// Generate a wasm module for the given *entry expression*.
///
/// 現段階では、エントリ式が `Type::I32` かつ `HirExprKind::I32` の場合だけ
/// 対応し、それ以外は "codegen not implemented" の Diagnostic を返す。
pub fn generate_wasm(entry: &HirExpr) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // 現時点では、エントリは i32 型の式とする。
    if entry.ty != Type::I32 {
        diagnostics.push(Diagnostic::error(
            "entry expression must have type i32 for wasm backend",
            entry.span,
        ));
        return Err(diagnostics);
    }

    match &entry.kind {
        HirExprKind::I32(value) => Ok(emit_const_i32_module(*value)),
        _ => {
            diagnostics.push(Diagnostic::error(
                "code generation for this expression kind is not implemented yet",
                entry.span,
            ));
            Err(diagnostics)
        }
    }
}

/// Build a simple wasm module with:
///
/// - a single function `main : () -> i32`
/// - body: `i32.const value`
fn emit_const_i32_module(value: i32) -> Vec<u8> {
    let mut module = Module::new();

    // Type section: (-> i32)
    let mut types = TypeSection::new();
    let type_index = types.len();
    types.ty().function(Vec::<ValType>::new(), [ValType::I32]);
    module.section(&types);

    // Function section: one function of the given type
    let mut functions = FunctionSection::new();
    functions.function(type_index);
    module.section(&functions);

    // Export section: export the function as "main"
    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, 0);
    module.section(&exports);

    // Code section: body of the function
    let mut code = CodeSection::new();
    let mut func = Function::new(Vec::new());
    func.instruction(&Instruction::I32Const(value));
    func.instruction(&Instruction::End);
    code.function(&func);
    module.section(&code);

    module.finish()
}

/// Utility for creating a simple internal error diagnostic.
///
/// 今は使っていないが、将来 HIR lowering 中の internal error を
/// 記録するのに使うかもしれない。
fn internal_error(span: Span, msg: &str) -> Diagnostic {
    Diagnostic::error(
        alloc::format!("internal codegen error: {}", msg),
        span,
    )
}
