use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use nepl_core::stdlib::default_stdlib_root;
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
        value_name = "PATH",
        help = "Path to the standard library root (defaults to bundled stdlib)",
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

fn run_wasm(artifact: &CompilationArtifact) -> Result<i32> {
    let engine = Engine::default();
    let module = Module::new(&engine, &artifact.wasm).context("failed to compile wasm artifact")?;
    let mut linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &module)
        .context("failed to instantiate module")?
        .start(&mut store)
        .context("failed to start module")?;
    let main = instance
        .get_typed_func::<(), i32>(&store, "main")
        .context("exported main function missing or has wrong type")?;
    let result = main
        .call(&mut store, ())
        .context("failed to execute main")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_cmd::Command;
    use predicates::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn compiles_and_runs_wasm() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "add 1 2").expect("write input");
        let output_path = dir.path().join("out.wasm");

        Command::cargo_bin("nepl-cli")
            .expect("binary exists")
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .arg("--run")
            .assert()
            .success()
            .stdout(predicate::str::contains("Program exited with 3"));

        assert!(output_path.exists(), "wasm output was not created");
    }

    #[test]
    fn emits_llvm_ir() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "mul 2 3").expect("write input");
        let output_path = dir.path().join("out.ll");

        Command::cargo_bin("nepl-cli")
            .expect("binary exists")
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .arg("--emit")
            .arg("llvm")
            .assert()
            .success();

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
        std::fs::write(stdlib_root.join("std.nepl"), "namespace std:").expect("write stdlib placeholder");

        Command::cargo_bin("nepl-cli")
            .expect("binary exists")
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .arg("--stdlib")
            .arg(&stdlib_root)
            .assert()
            .success();

        assert!(output_path.exists(), "wasm output was not created");
    }

    #[test]
    fn reports_missing_stdlib_root() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "add 1 1").expect("write input");
        let output_path = dir.path().join("out.wasm");

        Command::cargo_bin("nepl-cli")
            .expect("binary exists")
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .arg("--stdlib")
            .arg(dir.path().join("missing"))
            .assert()
            .failure()
            .stderr(predicate::str::contains("MissingStdlib"));
    }

    #[test]
    fn reports_division_by_zero() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "div 4 0").expect("write input");
        let output_path = dir.path().join("out.wasm");

        Command::cargo_bin("nepl-cli")
            .expect("binary exists")
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .assert()
            .failure()
            .stderr(predicate::str::contains("division by zero"));
    }

    #[test]
    fn supports_pipe_operator() {
        let dir = tempdir().expect("tempdir");
        let input_path = dir.path().join("input.nepl");
        fs::write(&input_path, "1 > neg > add 2").expect("write input");
        let output_path = dir.path().join("out.wasm");

        Command::cargo_bin("nepl-cli")
            .expect("binary exists")
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .arg("--run")
            .assert()
            .success()
            .stdout(predicate::str::contains("Program exited with 1"));

        assert!(output_path.exists(), "wasm output was not created");
    }
}
