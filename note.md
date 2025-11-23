# Progress note

- Parser and AST were realigned: added the missing `types` module, boxed recursive AST/HIR fields to break drop cycles, and synchronized parsing structures (use paths, type expressions, patterns, assignments) with the AST shapes.
- Updated wasm codegen to the current `wasm-encoder` API and ensured helper functions avoid const-evaluation restrictions.
- Simplified CLI to match the minimal core pipeline (no stdlib/builtins wiring yet) and refreshed its tests to only cover argument parsing while keeping the deploy helper tests intact.

- Updated wasm emission to the current `wasm-encoder` API and reordered sections so exported `main` functions validate correctly with wasmi 0.51.
- Added explicit stdlib root existence checks and recorded stdlib files in compilation artifacts; missing roots now surface as errors in both core and CLI flows.
- Reworked CLI tests to exercise the execution pipeline directly without spawning the binary and validated wasm outputs with wasmi.
- Added a core test that instantiates generated wasm with wasmi to ensure runtime compatibility.
- Current implementation reflects a minimal prefix-expression subset:
  - Literals: numbers, strings, vectors.
  - Operators: arithmetic, logic, comparison, bitwise, combinatorics, and some string/vector operations.
  - Built-in wasm/wasi functions and a structured stdlib layout.
  - No user-defined variables/functions, namespaces, or type system beyond “expression must evaluate to i32”.
- Broader language features from `plan.md`, `plan2.md`, and `doc/starting_detail.md` (P-style ambiguous expressions, full typing, namespaces, include/import/use, enum/struct, `loop`/`match`/`set`, `Never`, overload resolution) remain TODO.
- Introduced a `types` module that defines:
  - Primitive types (`i32`, `i64`, `f32`, `f64`, `Bool`, `Unit`, `Never`).
  - Function types `(T1, ..., Tn) -> R` and `(T1, ..., Tn) *> R` with an `ArrowKind` enum.
  - Subtyping utilities that treat `Never` as a bottom type and a `least_common_supertype` helper for control-flow typing.
- Introduced a `hir` module that defines:
  - A typed high-level IR `HIRExpr` with support for literals, variables, calls, `Let`, `Set`, `If`, `While`, `Loop`, `Match`, `Return`, `Break`, `Continue`, and block expressions.
  - Structures for function definitions, parameters, assignable expressions, and patterns used by `match`.

## Next implementation steps (planned)

- Add a `name_resolve` module that builds symbol tables for namespaces, functions, and types across all input files, following the rules in the design documents:
  - `namespace`, `include`, `import`, and `use` control how names become visible.
  - `enum` and `struct` declarations introduce new named types and associated constructors/fields.
- Add a `typecheck` module that consumes an untyped AST and produces HIR:
  - Perform name resolution and treat `Never` as a bottom type when typing `if` and `match`.
  - Enforce the typing rules for `loop` and `while`, including `break`/`break expr`/`continue`.
  - Enforce purity rules for `*>` functions, mutable parameters, `let mut`, and `set`.
  - Implement overload resolution for functions and operators according to `plan2.md`.
- Gradually extend the lexer and parser to cover:
  - P-style ambiguous sequences and the frame-based call resolution algorithm.
  - Structured control flow (`if`, `while`, `loop`, `match`) and scoping constructs.
  - Namespaces, `include`, `import`, `use`, `enum`, `struct`, `return`, `break`, and `continue`.
- Refactor `compiler.rs` so that:
  - It compiles HIR, not the current minimal `Expr`, into wasm.
  - It relies on type checking rather than evaluating expressions to validate them.
  - It preserves and extends the existing builtin and stdlib integration.
