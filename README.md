# NEPLg1

Workspace for the Neknaj Expression Prefix Language (NEPL) toolchain. The repository currently contains a core crate that lexes,
parses, and validates a small arithmetic-focused subset of NEPL, plus a command-line interface for compiling sources to WebAssembly or LLVM IR.

## Crates
- `nepl-core`: Loads the `.nepl` standard library files from `./stdlib`, parses prefix arithmetic expressions (`add`, `sub`, `mul`, `div`, `neg`), validates them, and emits executable WebAssembly modules or LLVM IR that returns the computed value.
- `nepl-cli`: Provides a Clap-based CLI for compiling sources, writing output artifacts, and executing WebAssembly output through `wasmi`.

## Usage
Compile a source file to WebAssembly and run it:

```bash
cargo run -p nepl-cli -- --input examples/hello.nepl --output target/hello.wasm --run
```

Emit LLVM IR instead:

```bash
cargo run -p nepl-cli -- --input examples/hello.nepl --output target/hello.ll --emit llvm
```

The CLI accepts input from stdin when `--input` is omitted.

### Supported expression forms

The current implementation supports prefix arithmetic expressions built from the operators `add`, `sub`, `mul`, `div`, and `neg`, using integer literals. Parentheses can be used to group expressions.

The pipe operator `>` is available as a convenience for threading the previous result into the next function call. For example, `1 > neg > add 2` desugars to `add (neg 1) 2`.

## Standard library layout
Place `.nepl` files under `./stdlib`. The core crate loads every `.nepl` file recursively, making them available to compilation routines. The CLI uses this bundled path by default, and you can point it to an alternate root with `--stdlib /path/to/stdlib` when testing different library layouts.

## Testing
Run host tests for all crates:

```bash
cargo test --workspace
```

Validate the core crate against the `wasm32-unknown-unknown` target:

```bash
cargo test --target wasm32-unknown-unknown --no-run -p nepl-core
```
