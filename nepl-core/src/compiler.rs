use std::path::Path;

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection, ValType,
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
    let type_index = types.len();
    types.ty().function([], [ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(type_index);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new(vec![]);
    emit_expression(&expr, &mut function)?;
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);

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
    let value = evaluate(expr)?;
    function.instruction(&Instruction::I32Const(value));
    Ok(())
}

fn validate(expr: &Expr) -> Result<(), CoreError> {
    evaluate(expr).map(|_| ())
}

fn evaluate(expr: &Expr) -> Result<i32, CoreError> {
    match expr {
        Expr::Number(value) => Ok(*value),
        Expr::Call { name, args } => match name.as_str() {
            "and" => {
                expect_arity(name, args.len(), 2)?;
                let left = truthy(evaluate(&args[0])?);
                let right = truthy(evaluate(&args[1])?);
                Ok(bool_to_i32(left && right))
            }
            "or" => {
                expect_arity(name, args.len(), 2)?;
                let left = truthy(evaluate(&args[0])?);
                let right = truthy(evaluate(&args[1])?);
                Ok(bool_to_i32(left || right))
            }
            "xor" => {
                expect_arity(name, args.len(), 2)?;
                let left = truthy(evaluate(&args[0])?);
                let right = truthy(evaluate(&args[1])?);
                Ok(bool_to_i32(left ^ right))
            }
            "neg" => {
                expect_arity(name, args.len(), 1)?;
                let value = evaluate(&args[0])?;
                Ok(value.wrapping_neg())
            }
            "not" => {
                expect_arity(name, args.len(), 1)?;
                let value = evaluate(&args[0])?;
                Ok(bool_to_i32(!truthy(value)))
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
            "mod" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                if right == 0 {
                    return Err(CoreError::SemanticError(
                        "modulo by zero is not allowed".to_string(),
                    ));
                }
                Ok(left.wrapping_rem(right))
            }
            "pow" => {
                expect_arity(name, args.len(), 2)?;
                let base = evaluate(&args[0])?;
                let exponent = evaluate(&args[1])?;
                if exponent < 0 {
                    return Err(CoreError::SemanticError(
                        "negative exponents are not supported".to_string(),
                    ));
                }
                base.checked_pow(exponent as u32)
                    .ok_or_else(|| CoreError::SemanticError("overflow during pow".to_string()))
            }
            "lt" => compare(name, args, |l, r| l < r),
            "le" => compare(name, args, |l, r| l <= r),
            "eq" => compare(name, args, |l, r| l == r),
            "ne" => compare(name, args, |l, r| l != r),
            "gt" => compare(name, args, |l, r| l > r),
            "ge" => compare(name, args, |l, r| l >= r),
            "bit_and" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left & right)
            }
            "bit_or" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left | right)
            }
            "bit_xor" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left ^ right)
            }
            "bit_not" => {
                expect_arity(name, args.len(), 1)?;
                let value = evaluate(&args[0])?;
                Ok(!value)
            }
            "bit_shl" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(left.wrapping_shl(right as u32))
            }
            "bit_shr" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(((left as u32).wrapping_shr(right as u32)) as i32)
            }
            "gcd" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                Ok(gcd(left, right))
            }
            "lcm" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                let divisor = gcd(left, right);
                left.checked_div(divisor)
                    .and_then(|v| v.checked_mul(right))
                    .ok_or_else(|| CoreError::SemanticError("overflow during lcm".to_string()))
            }
            "factorial" => {
                expect_arity(name, args.len(), 1)?;
                let value = evaluate(&args[0])?;
                factorial(value)
            }
            "permutation" => {
                expect_arity(name, args.len(), 2)?;
                let n = evaluate(&args[0])?;
                let r = evaluate(&args[1])?;
                permutation(n, r)
            }
            "combination" => {
                expect_arity(name, args.len(), 2)?;
                let n = evaluate(&args[0])?;
                let r = evaluate(&args[1])?;
                combination(n, r)
            }
            other => Err(CoreError::SemanticError(format!(
                "evaluation for operator '{other}' is not implemented",
            ))),
        },
    }
}

fn compare(
    name: &str,
    args: &[Expr],
    predicate: impl FnOnce(i32, i32) -> bool,
) -> Result<i32, CoreError> {
    expect_arity(name, args.len(), 2)?;
    let left = evaluate(&args[0])?;
    let right = evaluate(&args[1])?;
    Ok(bool_to_i32(predicate(left, right)))
}

fn truthy(value: i32) -> bool {
    value != 0
}

fn bool_to_i32(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

fn gcd(mut a: i32, mut b: i32) -> i32 {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a.abs()
}

fn factorial(value: i32) -> Result<i32, CoreError> {
    if value < 0 {
        return Err(CoreError::SemanticError(
            "factorial is undefined for negative values".to_string(),
        ));
    }
    let mut acc: i64 = 1;
    for i in 1..=value as i64 {
        acc = acc
            .checked_mul(i)
            .ok_or_else(|| CoreError::SemanticError("overflow during factorial".to_string()))?;
    }
    i32::try_from(acc)
        .map_err(|_| CoreError::SemanticError("overflow during factorial".to_string()))
}

fn permutation(n: i32, r: i32) -> Result<i32, CoreError> {
    if n < 0 || r < 0 || r > n {
        return Err(CoreError::SemanticError(
            "permutation requires 0 <= r <= n".to_string(),
        ));
    }
    let mut acc: i64 = 1;
    for i in 0..r {
        let term = (n - i) as i64;
        acc = acc
            .checked_mul(term)
            .ok_or_else(|| CoreError::SemanticError("overflow during permutation".to_string()))?;
    }
    i32::try_from(acc)
        .map_err(|_| CoreError::SemanticError("overflow during permutation".to_string()))
}

fn combination(n: i32, r: i32) -> Result<i32, CoreError> {
    if n < 0 || r < 0 || r > n {
        return Err(CoreError::SemanticError(
            "combination requires 0 <= r <= n".to_string(),
        ));
    }
    let numerator = permutation(n, r)? as i64;
    let denom = factorial(r)? as i64;
    numerator
        .checked_div(denom)
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(|| CoreError::SemanticError("overflow during combination".to_string()))
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
    use tempfile;
    use wasmparser::Parser;

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
    fn executes_generated_wasm_with_wasmi() {
        let artifact = compile_wasm("add 4 (sub 10 3)", default_stdlib_root())
            .expect("compile should succeed");

        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, &artifact.wasm).expect("module");
        let linker = wasmi::Linker::new(&engine);
        let mut store = wasmi::Store::new(&engine, ());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("instantiate");

        let main = instance
            .get_typed_func::<(), i32>(&store, "main")
            .expect("typed func");
        let result = main.call(&mut store, ()).expect("execute main");
        assert_eq!(result, 11);
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
        assert_eq!(
            artifact.stdlib.len(),
            2,
            "expected two stdlib files recorded"
        );

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

    #[test]
    fn evaluates_extended_math_operations() {
        assert_eq!(evaluate(&parse("mod 10 3").unwrap()).unwrap(), 1);
        assert_eq!(evaluate(&parse("pow 2 4").unwrap()).unwrap(), 16);
        assert_eq!(evaluate(&parse("gcd 54 24").unwrap()).unwrap(), 6);
        assert_eq!(evaluate(&parse("lcm 6 8").unwrap()).unwrap(), 24);
        assert_eq!(evaluate(&parse("factorial 5").unwrap()).unwrap(), 120);
        assert_eq!(evaluate(&parse("permutation 5 2").unwrap()).unwrap(), 20);
        assert_eq!(evaluate(&parse("combination 5 2").unwrap()).unwrap(), 10);
    }

    #[test]
    fn evaluates_comparison_and_bitwise_operations() {
        assert_eq!(evaluate(&parse("lt 1 2").unwrap()).unwrap(), 1);
        assert_eq!(evaluate(&parse("ge 3 7").unwrap()).unwrap(), 0);
        assert_eq!(evaluate(&parse("bit_and 6 3").unwrap()).unwrap(), 2);
        assert_eq!(evaluate(&parse("bit_or 4 1").unwrap()).unwrap(), 5);
        assert_eq!(evaluate(&parse("bit_not 0").unwrap()).unwrap(), -1);
        assert_eq!(evaluate(&parse("bit_shl 1 3").unwrap()).unwrap(), 8);
    }

    #[test]
    fn rejects_invalid_numeric_operations() {
        let mod_err = evaluate(&parse("mod 1 0").unwrap()).unwrap_err();
        assert!(matches!(mod_err, CoreError::SemanticError(_)));

        let pow_err = evaluate(&parse("pow 2 -1").unwrap()).unwrap_err();
        assert!(matches!(pow_err, CoreError::SemanticError(_)));

        let fact_err = evaluate(&parse("factorial -2").unwrap()).unwrap_err();
        assert!(matches!(fact_err, CoreError::SemanticError(_)));

        let perm_err = evaluate(&parse("permutation 2 5").unwrap()).unwrap_err();
        assert!(matches!(perm_err, CoreError::SemanticError(_)));
    }
}
