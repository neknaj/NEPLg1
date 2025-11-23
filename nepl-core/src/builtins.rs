//! Built-in functions and values for the NEPL core (no_std).
//!
//! This module defines host-provided builtins that are visible at
//! the NEPL language level. It does **not** perform any I/O or wasm
//! interaction itself; codegen modules are responsible for mapping
//! these descriptors to actual wasm imports or host calls.

#![allow(dead_code)]

use alloc::vec::Vec;

use crate::types::{ArrowKind, Type};

/// Kind of builtin, used by backends to decide how to lower a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKind {
    /// Returns the size of a wasm memory page in bytes (typically 65536).
    WasmPageSize,

    /// Returns a random 32-bit integer (WASI or host-dependent).
    WasiRandomI32,

    /// Prints a 32-bit integer to the host's console or log.
    WasiPrintI32,
}

/// Metadata about a single builtin symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinDescriptor {
    /// Name of the builtin at the NEPL level (e.g., `page_size`).
    pub name: &'static str,

    /// Logical module or namespace name (e.g., `platform.wasi`).
    ///
    /// This is purely informational for now; name resolution can
    /// choose whether and how to use it.
    pub logical_module: &'static str,

    /// The type of the builtin in the NEPL type system.
    pub ty: Type,

    /// Kind tag used by backends.
    pub kind: BuiltinKind,
}

/// The complete list of builtins known to the core.
///
/// New backends and stdlib code should prefer referring to this
/// table instead of hard-coding builtin names.
pub const BUILTINS: &[BuiltinDescriptor] = &[
    BuiltinDescriptor {
        name: "page_size",
        logical_module: "platform.wasm_core",
        ty: Type::impure_function(Vec::new(), Type::I32),
        kind: BuiltinKind::WasmPageSize,
    },
    BuiltinDescriptor {
        name: "random_i32",
        logical_module: "platform.wasi",
        ty: Type::impure_function(Vec::new(), Type::I32),
        kind: BuiltinKind::WasiRandomI32,
    },
    BuiltinDescriptor {
        name: "print_i32",
        logical_module: "platform.wasi",
        ty: Type::impure_function(vec![Type::I32], Type::Unit),
        kind: BuiltinKind::WasiPrintI32,
    },
];

/// Look up a builtin by its NEPL-level name.
///
/// The search is linear over `BUILTINS` because the table is small.
/// If performance ever matters, this can be optimized with a more
/// elaborate indexing structure (still no_std-compatible).
pub fn find_builtin(name: &str) -> Option<&'static BuiltinDescriptor> {
    for b in BUILTINS {
        if b.name == name {
            return Some(b);
        }
    }
    None
}
