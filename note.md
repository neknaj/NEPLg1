# Progress note

- Updated wasm emission to the current `wasm-encoder` API and reordered sections so exported `main` functions validate correctly with wasmi 0.51.
- Added explicit stdlib root existence checks and recorded stdlib files in compilation artifacts; missing roots now surface as errors in both core and CLI flows.
- Reworked CLI tests to exercise the execution pipeline directly without spawning the binary and validated wasm outputs with wasmi.
- Added a core test that instantiates generated wasm with wasmi to ensure runtime compatibility.
- Current implementation still reflects the minimal arithmetic subset; broader language features from `plan.md` and `doc/starting_detail.md` (namespaces, full typing, etc.) remain TODO.
- Implemented a structured standard library layout under `stdlib/` with math, logic, and bitwise namespaces and expanded the evaluator to cover those operators, including combinatorics and overflow checks.
- Extended the standard library surface with string, vector, and platform placeholder modules and updated stdlib loading tests to assert their presence.
- Added a built-in facility for WASM/WASI imports, recorded them in compilation artifacts, and linked them in the CLI runtime so `--run` can execute modules that depend on host intrinsics.
- Finished the platform stdlib modules to wrap the new built-ins and added runtime tests (core + CLI) that exercise `wasm_pagesize`, `wasi_random`, and `wasi_print` through wasmi 0.51.
- Introduced a reusable builtin handler trait in the CLI so host environments can override WASM/WASI bindings; tests now assert custom handlers receive calls and can log values.
- Added artifact-facing checks that surface bundled stdlib contents for consumers and documented the default host behaviors in the README to keep the CLI and stdlib expectations aligned.
- Added wasm execution tests that cover logical, comparison, bitwise, and arithmetic chains to verify code generation works end-to-end through wasmi 0.51, and updated README coverage of the supported operators.
- Expanded the parser, lexer, and evaluator to understand string literals and vector literals, added runtime support for their stdlib operators (len/push/pop/get/concat), and validated them through new core and CLI wasm execution tests.
- Added a `convert` stdlib namespace for `parse_i32` / `to_string` / `to_bool`, wired the evaluator to handle the conversions (including error cases), refreshed stdlib loading assertions, and exercised the new helpers through wasm execution paths.
