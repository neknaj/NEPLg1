use std::collections::{BTreeSet, HashSet};

use crate::ast::Expr;
use wasm_encoder::ValType;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BuiltinKind {
    WasmPageSize,
    WasiRandom,
    WasiPrint,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuiltinDescriptor {
    pub name: String,
    pub module: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
    pub kind: BuiltinKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Builtin {
    pub name: &'static str,
    pub module: &'static str,
    pub params: &'static [ValType],
    pub results: &'static [ValType],
    pub kind: BuiltinKind,
}

const WASM_CORE_BUILTINS: &[Builtin] = &[Builtin {
    name: "wasm_pagesize",
    module: "env",
    params: &[],
    results: &[ValType::I32],
    kind: BuiltinKind::WasmPageSize,
}];

const WASI_BUILTINS: &[Builtin] = &[
    Builtin {
        name: "wasi_random",
        module: "wasi_snapshot_preview1",
        params: &[],
        results: &[ValType::I32],
        kind: BuiltinKind::WasiRandom,
    },
    Builtin {
        name: "wasi_print",
        module: "wasi_snapshot_preview1",
        params: &[ValType::I32],
        results: &[ValType::I32],
        kind: BuiltinKind::WasiPrint,
    },
];

pub fn operator_arity(name: &str) -> Option<usize> {
    lookup(name).map(|builtin| builtin.params.len())
}

pub fn lookup(name: &str) -> Option<&'static Builtin> {
    WASM_CORE_BUILTINS
        .iter()
        .chain(WASI_BUILTINS.iter())
        .find(|builtin| builtin.name == name)
}

pub fn collect_builtins(expr: &Expr) -> Vec<&'static Builtin> {
    let mut names = HashSet::new();
    collect_builtin_names(expr, &mut names);

    let mut ordered = BTreeSet::new();
    for name in names {
        if lookup(&name).is_some() {
            ordered.insert(name);
        }
    }

    ordered
        .into_iter()
        .filter_map(|name| lookup(&name))
        .collect()
}

pub fn to_descriptor(builtin: &Builtin) -> BuiltinDescriptor {
    BuiltinDescriptor {
        name: builtin.name.to_string(),
        module: builtin.module.to_string(),
        params: builtin.params.to_vec(),
        results: builtin.results.to_vec(),
        kind: builtin.kind.clone(),
    }
}

fn collect_builtin_names(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Number(_) => {}
        Expr::Call { name, args } => {
            names.insert(name.clone());
            for arg in args {
                collect_builtin_names(arg, names);
            }
        }
    }
}
