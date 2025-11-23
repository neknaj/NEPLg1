use std::path::Path;

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, FunctionType, Instruction,
    Module, TypeSection, ValType,
};

use crate::ast::Expr;
use crate::error::CoreError;
use crate::parser::parse;
use crate::stdlib::{StdlibFile, load_stdlib_files};

#[derive(Debug, PartialEq, Eq)]
pub struct CompilationArtifact {
    pub wasm: Vec<u8>,
    pub stdlib: Vec<StdlibFile>,
}

pub fn compile_wasm(
    source: &str,
    stdlib_root: impl AsRef<Path>,
) -> Result<CompilationArtifact, CoreError> {
    let stdlib_root = stdlib_root.as_ref();
    let stdlib = load_stdlib_files(stdlib_root)
        .map_err(|_| CoreError::MissingStdlib(stdlib_root.to_path_buf()))?;

    let expr = parse(source)?;
    validate(&expr)?;

    let mut module = Module::new();
    let mut types = TypeSection::new();
    let type_index = types.function(FunctionType {
        params: Vec::<ValType>::new().into(),
        results: vec![ValType::I32].into(),
    });
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(type_index);
    module.section(&functions);

    let mut code = CodeSection::new();
    let mut function = Function::new(vec![]);
    emit_expression(&expr, &mut function)?;
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);

    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, 0);
    module.section(&exports);

    Ok(CompilationArtifact {
        wasm: module.finish(),
        stdlib,
    })
}

pub fn emit_llvm_ir(source: &str, stdlib_root: impl AsRef<Path>) -> Result<String, CoreError> {
    let stdlib_root = stdlib_root.as_ref();
    let stdlib = load_stdlib_files(stdlib_root)
        .map_err(|_| CoreError::MissingStdlib(stdlib_root.to_path_buf()))?;
    let expr = parse(source)?;
    let value = evaluate(&expr)?;

    let header = "; ModuleID = \"nepl-placeholder\"\nsource_filename = \"nepl-input\"\n";
    let mut lines = String::new();
    lines.push_str("define i32 @main() {\n");
    lines.push_str(&format!("  ret i32 {}\n", value));
    lines.push_str("}\n");
    lines.push_str(&format!("; stdlib files: {}\n", stdlib.len()));

    Ok(format!("{}{}", header, lines))
}

fn emit_expression(expr: &Expr, function: &mut Function) -> Result<(), CoreError> {
    match expr {
        Expr::Number(value) => {
            function.instruction(&Instruction::I32Const(*value));
        }
        Expr::Call { name, args } => {
            for arg in args {
                emit_expression(arg, function)?;
            }
            match name.as_str() {
                "add" => {
                    expect_arity(name, args.len(), 2)?;
                    function.instruction(&Instruction::I32Add);
                }
                "sub" => {
                    expect_arity(name, args.len(), 2)?;
                    function.instruction(&Instruction::I32Sub);
                }
                "mul" => {
                    expect_arity(name, args.len(), 2)?;
                    function.instruction(&Instruction::I32Mul);
                }
                "div" => {
                    expect_arity(name, args.len(), 2)?;
                    function.instruction(&Instruction::I32DivS);
                }
                "neg" => {
                    expect_arity(name, args.len(), 1)?;
                    function.instruction(&Instruction::I32Const(-1));
                    function.instruction(&Instruction::I32Mul);
                }
                other => {
                    return Err(CoreError::SemanticError(format!(
                        "codegen for operator '{other}' is not implemented",
                    )));
                }
            }
        }
    }

    Ok(())
}

fn validate(expr: &Expr) -> Result<(), CoreError> {
    evaluate(expr).map(|_| ())
}

fn evaluate(expr: &Expr) -> Result<i32, CoreError> {
    match expr {
        Expr::Number(value) => Ok(*value),
        Expr::Call { name, args } => match name.as_str() {
            "neg" => {
                expect_arity(name, args.len(), 1)?;
                let value = evaluate(&args[0])?;
                Ok(value.wrapping_neg())
            }
            "add" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left.wrapping_add(right))
            }
            "sub" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left.wrapping_sub(right))
            }
            "mul" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left.wrapping_mul(right))
            }
            "div" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                if right == 0 {
                    return Err(CoreError::SemanticError(
                        "division by zero is not allowed".to_string(),
                    ));
                }
                Ok(left.wrapping_div(right))
            }
            other => Err(CoreError::SemanticError(format!(
                "evaluation for operator '{other}' is not implemented",
            ))),
        },
    }
}

fn expect_arity(name: &str, given: usize, expected: usize) -> Result<(), CoreError> {
    if given == expected {
        return Ok(());
    }
    Err(CoreError::SemanticError(format!(
        "operator '{name}' expects {expected} arguments but received {given}",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::stdlib::default_stdlib_root;
    use std::path::PathBuf;
    use wasmparser::Parser;
    use tempfile;

    #[test]
    fn builds_arithmetic_wasm_module() {
        let artifact =
            compile_wasm("add 1 (mul 2 3)", default_stdlib_root()).expect("compile should succeed");
        let mut parser = Parser::new(0);
        let payload = parser
            .parse(artifact.wasm.as_slice(), true)
            .expect("payload");
        assert!(matches!(payload, wasmparser::Chunk::Parsed { .. }));
    }

    #[test]
    fn emits_llvm_ir_with_calculated_value() {
        let ir = emit_llvm_ir("sub 10 7", default_stdlib_root()).expect("emit should succeed");
        assert!(ir.contains("ret i32 3"));
    }

    #[test]
    fn validates_division_by_zero() {
        let err = emit_llvm_ir("div 1 0", default_stdlib_root()).unwrap_err();
        assert!(matches!(err, CoreError::SemanticError(_)));
    }

    #[test]
    fn reports_missing_stdlib_root() {
        let missing_root = PathBuf::from("./path/that/does/not/exist");
        let err = compile_wasm("add 1 2", &missing_root).unwrap_err();
        assert!(matches!(err, CoreError::MissingStdlib(_)));
    }

    #[test]
    fn captures_loaded_stdlib_files_in_artifact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stdlib_root = dir.path();
        let nested = stdlib_root.join("nested");
        std::fs::create_dir_all(&nested).expect("create nested dir");
        std::fs::write(stdlib_root.join("std.nepl"), "namespace std:")
            .expect("write stdlib root file");
        std::fs::write(nested.join("math.nepl"), "namespace math:")
            .expect("write nested stdlib file");

        let artifact = compile_wasm("add 1 2", stdlib_root).expect("compile should succeed");
        assert_eq!(artifact.stdlib.len(), 2, "expected two stdlib files recorded");

        let paths: Vec<_> = artifact
            .stdlib
            .iter()
            .map(|file| file.path.clone())
            .collect();
        assert!(paths.contains(&PathBuf::from("std.nepl")));
        assert!(paths.contains(&PathBuf::from("nested/math.nepl")));
    }

    #[test]
    fn evaluates_expression() {
        let expr = parse("add 1 (mul 2 3)").expect("parse");
        assert_eq!(evaluate(&expr).unwrap(), 7);
    }

    #[test]
    fn evaluates_pipe_expression() {
        let expr = parse("1 > neg > add 2").expect("parse");
        assert_eq!(evaluate(&expr).unwrap(), 1);
    }
}
