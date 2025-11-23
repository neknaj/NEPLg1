# Progress note

- Implemented a basic NEPL arithmetic pipeline: lexing, parsing, validating, and code generating prefix expressions (`add`, `sub`, `mul`, `div`, `neg`) to WebAssembly and LLVM IR.
- Added semantic validation to catch errors such as division by zero during compilation.
- Updated the CLI integration tests to execute generated wasm and to report validation errors.
- Added support for the pipe operator `>` with desugaring through the parser, compiler evaluation, and CLI execution paths.
- Documented the supported subset (including the pipe operator) and workflow in README.md. Broader language features from plan.md and doc/starting_detail.md (namespaces, types, overloads, etc.) are still outstanding.
