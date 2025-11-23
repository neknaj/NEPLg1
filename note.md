# Progress note

- Updated wasm emission to the current `wasm-encoder` API and reordered sections so exported `main` functions validate correctly with wasmi 0.51.
- Added explicit stdlib root existence checks and recorded stdlib files in compilation artifacts; missing roots now surface as errors in both core and CLI flows.
- Reworked CLI tests to exercise the execution pipeline directly without spawning the binary and validated wasm outputs with wasmi.
- Added a core test that instantiates generated wasm with wasmi to ensure runtime compatibility.
- Current implementation still reflects the minimal arithmetic subset; broader language features from `plan.md` and `doc/starting_detail.md` (namespaces, full typing, etc.) remain TODO.
