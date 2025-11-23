use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use nepl_core::span::FileId;
use nepl_core::{CompilationArtifact, compile_wasm, emit_llvm_ir};
use wasmi::{Engine, Linker, Module, Store};

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
    let source = match cli.input {
        Some(path) => fs::read_to_string(&path)
            .with_context(|| format!("failed to read input file {path}"))?,
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    let file_id = FileId(0);

    match cli.emit.as_str() {
        "wasm" => {
            let artifact = compile_wasm(file_id, &source)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            write_output(&cli.output, &artifact.wasm)?;
            if cli.run {
                let result = run_wasm(&artifact)?;
                println!("Program exited with {result}");
            }
        }
        "llvm" => {
            let ir = emit_llvm_ir(file_id, &source)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
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

fn run_wasm(artifact: &CompilationArtifact) -> Result<i32> {
    let engine = Engine::default();
    let module = Module::new(&engine, &artifact.wasm).context("failed to compile wasm artifact")?;
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .context("failed to instantiate module")?;
    let main = instance
        .get_typed_func::<(), i32>(&store, "main")
        .context("exported main function missing or has wrong type")?;
    main.call(&mut store, ()).context("failed to execute main")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_defaults() {
        let cli = Cli::parse_from(["nepl-cli", "--output", "out.wasm"]);
        assert_eq!(cli.emit, "wasm");
        assert!(!cli.run);
    }
}
