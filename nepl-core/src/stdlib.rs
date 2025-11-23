//! Standard library layout and indexing (no_std, no I/O).
//!
//! This module describes the logical structure of the NEPL standard
//! library without performing any file system operations.
//!
//! The actual loading of `*.nepl` files from disk, WASM virtual
//! filesystems, HTTP, etc. is the responsibility of higher-level
//! crates (CLI, web playground, etc.).
//!
//! nepl-core only provides:
//!   - logical module names (e.g. `core.math`, `platform.wasi`)
//!   - relative paths under the `stdlib/` directory
//!   - simple lookup helpers.

#![allow(dead_code)]

/// Description of a single stdlib module.
///
/// `stdlib_root` はプラットフォーム側 (CLI / Web など) が管理し、
/// ここにある `relative_path` と結合して実ファイルパスを構成する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibModuleSpec {
    /// Logical module name, e.g. "core.math", "platform.wasi".
    pub logical_name: &'static str,

    /// Relative path under the `stdlib/` directory, e.g.
    /// "core/math.nepl", "platform/wasi.nepl".
    pub relative_path: &'static str,
}

/// The list of all stdlib modules known to the core.
///
/// 必要に応じてここにモジュールを追加していく。
pub const STDLIB_MODULES: &[StdlibModuleSpec] = &[
    // Core math / logic / bitwise modules
    StdlibModuleSpec {
        logical_name: "core.math",
        relative_path: "core/math.nepl",
    },
    StdlibModuleSpec {
        logical_name: "core.logic",
        relative_path: "core/logic.nepl",
    },
    StdlibModuleSpec {
        logical_name: "core.bit",
        relative_path: "core/bit.nepl",
    },

    // Platform-specific modules
    StdlibModuleSpec {
        logical_name: "platform.wasm_core",
        relative_path: "platform/wasm_core.nepl",
    },
    StdlibModuleSpec {
        logical_name: "platform.wasi",
        relative_path: "platform/wasi.nepl",
    },
];

/// Iterate over all stdlib module specifications.
///
/// 上位レイヤーはこれを使って stdlib のファイルを列挙し、
/// 実際のファイルシステムから読み込む。
pub fn iter_stdlib_modules() -> &'static [StdlibModuleSpec] {
    STDLIB_MODULES
}

/// Find a stdlib module by its logical name (e.g. "core.math").
pub fn find_by_logical_name(name: &str) -> Option<&'static StdlibModuleSpec> {
    for m in STDLIB_MODULES {
        if m.logical_name == name {
            return Some(m);
        }
    }
    None
}

/// Find a stdlib module by its relative path (e.g. "core/math.nepl").
pub fn find_by_relative_path(path: &str) -> Option<&'static StdlibModuleSpec> {
    for m in STDLIB_MODULES {
        if m.relative_path == path {
            return Some(m);
        }
    }
    None
}
