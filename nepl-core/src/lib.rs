//! Core utilities for the NEPL language toolchain.

pub mod ast;
pub mod compiler;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod stdlib;

pub use compiler::{CompilationArtifact, compile_wasm, emit_llvm_ir};
pub use error::CoreError;
