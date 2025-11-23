# NEPLg1

Workspace for the Neknaj Expression Prefix Language (NEPL) toolchain. The repository currently contains a core crate that lexes,
parses, and validates a small arithmetic-focused subset of NEPL, plus a command-line interface for compiling sources to WebAssembly or LLVM IR.

## Crates
- `nepl-core`: Loads the `.nepl` standard library files from `./stdlib`, parses prefix expressions (`add`, `sub`, `mul`, `div`, `mod`, `pow`, `neg`, comparisons, bitwise ops, logic ops), validates them, and emits executable WebAssembly modules or LLVM IR that returns the computed value.
- `nepl-cli`: Provides a Clap-based CLI for compiling sources, writing output artifacts, and executing WebAssembly output through `wasmi`.

## Usage
Compile a source file to WebAssembly and run it:

```bash
cargo run -p nepl-cli -- --input examples/io_pipeline.nepl --output target/io_pipeline.wasm --run
```

Emit LLVM IR instead:

```bash
cargo run -p nepl-cli -- --input examples/io_pipeline.nepl --output target/io_pipeline.ll --emit llvm
```

The CLI accepts input from stdin when `--input` is omitted.

### Examples

Practical examples live under `examples/`:

- `io_pipeline.nepl` threads together arithmetic, vector access, string utilities, and standard I/O built-ins. It prints the length of a concatenated string and a computed value based on a host-provided random number before returning their sum.

### Supported expression forms

The current implementation supports prefix arithmetic expressions built from the operators `add`, `sub`, `mul`, `div`, `mod`, `pow`, `neg`, comparisons (`lt`, `le`, `eq`, `ne`, `gt`, `ge`), bitwise operators, and boolean operators (`and`, `or`, `not`, `xor`), using integer literals. Parentheses can be used to group expressions. Imports for target-specific built-ins are generated when you reference them in source code, and `nepl-cli --run` links default host behavior through `wasmi` 0.51:

- `wasm_pagesize` imports `env.wasm_pagesize` and returns the host-provided page size (default: 65,536 bytes).
- `wasi_random` imports `wasi_snapshot_preview1.wasi_random` and returns a deterministic host number (default: 4).
- `wasi_print <value>` imports `wasi_snapshot_preview1.wasi_print` to emit the value and return it for further chaining. The default host implementation prints to stdout.

The pipe operator `>` is available as a convenience for threading the previous result into the next function call. For example, `1 > neg > add 2` desugars to `add (neg 1) 2`.

## Web playground

An experimental browser playground lives under `web/`. To embed the external editor used for NEPL snippets, clone the editor repository before serving the page:

```bash
scripts/fetch_editorsample.sh
```

The script clones `https://github.com/bem130/editorsample` into `web/vendor/editorsample`. Set `EDITOR_SAMPLE_REPO`, `EDITOR_SAMPLE_REF`, or `EDITOR_SAMPLE_DEST` to override the repository, ref, or destination root for testing or offline mirroring. After cloning, serve `web/index.html` (for example with `trunk serve` or any static file server) to load the editor iframe and the playground scaffolding.

## Standard library layout
Place `.nepl` files under `./stdlib`. The core crate loads every `.nepl` file recursively and records the relative path and contents in the `CompilationArtifact` so downstream tooling can embed or inspect the bundled library. The CLI uses this bundled path by default, and you can point it to an alternate root with `--stdlib /path/to/stdlib` when testing different library layouts. Platform shims live under `stdlib/platform` and wrap the WASM/WASI built-ins exposed by the compiler.

## Testing
Run host tests for all crates:

```bash
cargo test --workspace
```

Validate the core crate against the `wasm32-unknown-unknown` target:

```bash
cargo test --target wasm32-unknown-unknown --no-run -p nepl-core
```
