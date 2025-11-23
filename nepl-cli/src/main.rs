use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use nepl_core::builtins::BuiltinKind;
use nepl_core::stdlib::default_stdlib_root;
use nepl_core::{CompilationArtifact, compile_wasm, emit_llvm_ir};
use wasmi::{Caller, Engine, Linker, Module, Store};

/// コマンドライン引数を定義するための構造体
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    input: Option<String>,

    #[arg(short, long)]
    output: String,

    #[arg(
        long,
        value_name = "PATH",
        help = "Path to the standard library root (defaults to bundled stdlib)"
    )]
    stdlib: Option<String>,

    #[arg(
        long,
        value_name = "FORMAT",
        default_value = "wasm",
        help = "Output format: wasm, llvm"
    )]
    emit: String,

    #[arg(long, help = "Run the code if the output format is wasm")]
    run: bool,
    #[arg(
        long,
        help = "Compile as library (do not wrap top-level in an implicit main)"
    )]
    lib: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    execute(cli)
}

fn execute(cli: Cli) -> Result<()> {
    let stdlib_root = cli
        .stdlib
        .as_ref()
        .map(|path| PathBuf::from(path))
        .unwrap_or_else(default_stdlib_root);

    let source = match cli.input {
        Some(path) => fs::read_to_string(&path)
            .with_context(|| format!("failed to read input file {path}"))?,
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    match cli.emit.as_str() {
        "wasm" => {
            let artifact = compile_wasm(&source, &stdlib_root)?;
            write_output(&cli.output, &artifact.wasm)?;
            if cli.run {
                let result = run_wasm(&artifact)?;
                println!("Program exited with {result}");
            }
        }
        "llvm" => {
            let ir = emit_llvm_ir(&source, &stdlib_root)?;
            write_output(&cli.output, ir.as_bytes())?;
            if cli.run {
                eprintln!("--run is ignored for non-wasm outputs");
            }
        }
        other => return Err(anyhow::anyhow!("unsupported emit format: {other}")),
    }

    if cli.lib {
        eprintln!("--lib is acknowledged but not yet implemented in the placeholder pipeline");
    }

    Ok(())
}

fn write_output(path: &str, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = PathBuf::from(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {parent:?}"))?;
        }
    }
    fs::write(path, bytes).with_context(|| format!("failed to write output file {path}"))?;
    Ok(())
}

trait BuiltinHandler {
    fn wasm_pagesize(&mut self) -> i32;
    fn wasi_random(&mut self) -> i32;
    fn wasi_print(&mut self, value: i32) -> i32;
}

#[derive(Default)]
struct DefaultBuiltinHandler;

impl BuiltinHandler for DefaultBuiltinHandler {
    fn wasm_pagesize(&mut self) -> i32 {
        65_536
    }

    fn wasi_random(&mut self) -> i32 {
        4
    }

    fn wasi_print(&mut self, value: i32) -> i32 {
        println!("{value}");
        value
    }
}

fn run_wasm(artifact: &CompilationArtifact) -> Result<i32> {
    let (result, _state) = run_wasm_with_handler(artifact, DefaultBuiltinHandler::default())?;
    Ok(result)
}

fn run_wasm_with_handler<H>(artifact: &CompilationArtifact, handler: H) -> Result<(i32, H)>
where
    H: BuiltinHandler + 'static,
{
    let engine = Engine::default();
    let module = Module::new(&engine, &artifact.wasm).context("failed to compile wasm artifact")?;
    let mut linker = Linker::new(&engine);
    link_builtins(&mut linker, &artifact.builtins)?;
    let mut store = Store::new(&engine, handler);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .context("failed to instantiate module")?;
    let main = instance
        .get_typed_func::<(), i32>(&store, "main")
        .context("exported main function missing or has wrong type")?;
    let result = main
        .call(&mut store, ())
        .context("failed to execute main")?;
    let state = store.into_data();
    Ok((result, state))
}

fn link_builtins<H: BuiltinHandler + 'static>(
    linker: &mut Linker<H>,
    builtins: &[nepl_core::builtins::BuiltinDescriptor],
) -> Result<()> {
    for builtin in builtins {
        match builtin.kind {
            BuiltinKind::WasmPageSize => {
                linker
                    .func_wrap(
                        builtin.module.as_str(),
                        builtin.name.as_str(),
                        |mut caller: Caller<'_, H>| -> i32 { caller.data_mut().wasm_pagesize() },
                    )
                    .with_context(|| format!("failed to link builtin {}", builtin.name))?;
            }
            BuiltinKind::WasiRandom => {
                linker
                    .func_wrap(
                        builtin.module.as_str(),
                        builtin.name.as_str(),
                        |mut caller: Caller<'_, H>| -> i32 { caller.data_mut().wasi_random() },
                    )
                    .with_context(|| format!("failed to link builtin {}", builtin.name))?;
            }
            BuiltinKind::WasiPrint => {
                linker
                    .func_wrap(
                        builtin.module.as_str(),
                        builtin.name.as_str(),
                        |mut caller: Caller<'_, H>, value: i32| -> i32 {
                            caller.data_mut().wasi_print(value)
                        },
                    )
                    .with_context(|| format!("failed to link builtin {}", builtin.name))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl_core::compile_wasm;
    use nepl_core::stdlib::default_stdlib_root;
    use tempfile::tempdir;

    #[test]
    fn compiles_and_runs_wasm() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "add 1 2").expect("write input");
        let output_path = dir.path().join("out.wasm");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: None,
            emit: "wasm".to_string(),
            run: true,
            lib: false,
        };

        execute(cli).expect("cli should succeed");

        let bytes = fs::read(&output_path).expect("wasm output readable");
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("module");
        let linker = Linker::new(&engine);
        let mut store = Store::new(&engine, ());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("instantiate");
        let main = instance
            .get_typed_func::<(), i32>(&store, "main")
            .expect("typed func");
        assert_eq!(main.call(&mut store, ()).expect("run"), 3);
    }

    #[test]
    fn emits_llvm_ir() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "mul 2 3").expect("write input");
        let output_path = dir.path().join("out.ll");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: None,
            emit: "llvm".to_string(),
            run: false,
            lib: false,
        };

        execute(cli).expect("cli should succeed");

        let ir = fs::read_to_string(&output_path).expect("read ir");
        assert!(ir.contains("define i32 @main"));
    }

    #[test]
    fn supports_custom_stdlib_root() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "add 1 1").expect("write input");
        let output_path = dir.path().join("out.wasm");

        let stdlib_root = dir.path().join("stdlib");
        std::fs::create_dir_all(&stdlib_root).expect("create stdlib root");
        std::fs::write(stdlib_root.join("std.nepl"), "namespace std:")
            .expect("write stdlib placeholder");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: Some(stdlib_root.to_string_lossy().to_string()),
            emit: "wasm".to_string(),
            run: false,
            lib: false,
        };

        execute(cli).expect("cli should succeed");

        assert!(output_path.exists(), "wasm output was not created");
    }

    #[test]
    fn reports_missing_stdlib_root() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "add 1 1").expect("write input");
        let output_path = dir.path().join("out.wasm");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: Some(dir.path().join("missing").to_string_lossy().to_string()),
            emit: "wasm".to_string(),
            run: false,
            lib: false,
        };

        let err = execute(cli).expect_err("cli should fail");
        assert!(
            err.to_string()
                .contains("standard library directory was not found")
        );
    }

    #[test]
    fn reports_division_by_zero() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "div 4 0").expect("write input");
        let output_path = dir.path().join("out.wasm");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: None,
            emit: "wasm".to_string(),
            run: false,
            lib: false,
        };

        let err = execute(cli).expect_err("cli should fail");
        assert!(err.to_string().contains("division by zero"));
    }

    #[test]
    fn supports_pipe_operator() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "1 > neg > add 2").expect("write input");
        let output_path = dir.path().join("out.wasm");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: None,
            emit: "wasm".to_string(),
            run: true,
            lib: false,
        };

        execute(cli).expect("cli should succeed");

        let bytes = fs::read(&output_path).expect("wasm output readable");
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("module");
        let linker = Linker::new(&engine);
        let mut store = Store::new(&engine, ());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("instantiate");
        let main = instance
            .get_typed_func::<(), i32>(&store, "main")
            .expect("typed func");
        assert_eq!(main.call(&mut store, ()).expect("run"), 1);
    }

    #[test]
    fn runs_string_and_vector_stdlib_paths() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(
            &input_path,
            "add (len concat \"ha\" \"!\") (len pop push [1 2] 3)",
        )
        .expect("write input");
        let output_path = dir.path().join("out.wasm");

        let cli = Cli {
            input: Some(input_path.to_string_lossy().to_string()),
            output: output_path.to_string_lossy().to_string(),
            stdlib: None,
            emit: "wasm".to_string(),
            run: true,
            lib: false,
        };

        execute(cli).expect("cli should succeed");

        let bytes = fs::read(&output_path).expect("wasm output readable");
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("module");
        let linker = Linker::new(&engine);
        let mut store = Store::new(&engine, ());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("instantiate");
        let main = instance
            .get_typed_func::<(), i32>(&store, "main")
            .expect("typed func");
        assert_eq!(main.call(&mut store, ()).expect("run"), 5);
    }

    #[test]
    fn links_wasi_builtins_when_running() {
        let artifact = compile_wasm("wasi_print (wasi_random)", default_stdlib_root())
            .expect("compile should succeed");

        #[derive(Default)]
        struct WasiHost {
            random: i32,
            prints: Vec<i32>,
        }

        impl BuiltinHandler for WasiHost {
            fn wasm_pagesize(&mut self) -> i32 {
                0
            }

            fn wasi_random(&mut self) -> i32 {
                self.random
            }

            fn wasi_print(&mut self, value: i32) -> i32 {
                self.prints.push(value);
                value
            }
        }

        let (result, state) = run_wasm_with_handler(
            &artifact,
            WasiHost {
                random: 123,
                prints: vec![],
            },
        )
        .expect("run should succeed");

        assert_eq!(result, 123);
        assert_eq!(state.prints, vec![123]);
    }

    #[test]
    fn runs_with_custom_builtin_handler() {
        let artifact = compile_wasm(
            "add wasm_pagesize (wasi_print (wasi_random))",
            default_stdlib_root(),
        )
        .expect("compile should succeed");

        #[derive(Default)]
        struct RecordingHost {
            pagesize: i32,
            random_value: i32,
            printed: Vec<i32>,
        }

        impl BuiltinHandler for RecordingHost {
            fn wasm_pagesize(&mut self) -> i32 {
                self.pagesize
            }

            fn wasi_random(&mut self) -> i32 {
                self.random_value
            }

            fn wasi_print(&mut self, value: i32) -> i32 {
                self.printed.push(value);
                value
            }
        }

        let (result, handler) = run_wasm_with_handler(
            &artifact,
            RecordingHost {
                pagesize: 2_048,
                random_value: 7,
                printed: Vec::new(),
            },
        )
        .expect("run should succeed");

        assert_eq!(result, 2_048 + 7);
        assert_eq!(handler.printed, vec![7]);
    }
}
