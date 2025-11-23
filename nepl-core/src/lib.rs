#![no_std]

//! Core utilities for the NEPL language toolchain.
//!
//! This crate provides the core compiler pipeline for the NEPL language.
//! The pipeline is roughly:
//!
//!   source .nepl
//!     -> lexer      (tokens)
//!     -> parser     (surface AST / P-style sequences)
//!     -> name_resolve + typecheck (types + HIR)
//!     -> codegen_wasm (wasm-encoder)
//!
//! Higher-level tools (CLI, web playground, etc.) should depend on this
//! crate rather than reimplementing the pipeline.

extern crate alloc;

// ---------------------------------------------------------------------
// Error handling and diagnostics
// ---------------------------------------------------------------------

pub mod span;
pub mod diagnostic;
pub mod error;

// ---------------------------------------------------------------------
// Front-end: lexing and parsing
// ---------------------------------------------------------------------

pub mod lexer;
pub mod parser;
pub mod ast;

// ---------------------------------------------------------------------
// Semantic layers: types, name resolution, type checking, HIR
// ---------------------------------------------------------------------

pub mod types;
pub mod name_resolve;
pub mod typecheck;
pub mod hir;

// ---------------------------------------------------------------------
// Builtins and stdlib integration
// ---------------------------------------------------------------------

pub mod builtins;
pub mod stdlib;

// ---------------------------------------------------------------------
// Back-end: code generation and compiler orchestration
// ---------------------------------------------------------------------

pub mod codegen_wasm;
pub mod codegen_llvm;
pub mod compiler;

// ---------------------------------------------------------------------
// Public API re-exports
// ---------------------------------------------------------------------

pub use compiler::{CompilationArtifact, compile_wasm, emit_llvm_ir};
pub use error::CoreError;
