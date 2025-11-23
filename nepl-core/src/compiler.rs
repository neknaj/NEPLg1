use std::collections::HashMap;
use std::path::Path;

use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Instruction, Module, TypeSection, ValType,
};

use crate::ast::Expr;
use crate::builtins::{self, BuiltinDescriptor};
use crate::error::CoreError;
use crate::parser::parse;
use crate::stdlib::{StdlibFile, load_stdlib_files};

#[derive(Debug, PartialEq, Eq)]
pub struct CompilationArtifact {
    pub wasm: Vec<u8>,
    pub stdlib: Vec<StdlibFile>,
    pub builtins: Vec<BuiltinDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Number(i32),
    Str(String),
    Vector(Vec<Value>),
}

impl Value {
    fn as_i32(self) -> Result<i32, CoreError> {
        match self {
            Value::Number(value) => Ok(value),
            other => Err(CoreError::SemanticError(format!(
                "expected numeric value but found {:?}",
                other,
            ))),
        }
    }
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

    let used_builtins = builtins::collect_builtins(&expr);

    let mut module = Module::new();
    let mut types = TypeSection::new();
    let mut builtin_type_indices = HashMap::new();
    let mut builtin_function_indices = HashMap::new();

    for builtin in &used_builtins {
        let type_index = types.len();
        types.ty().function(
            builtin.params.iter().copied(),
            builtin.results.iter().copied(),
        );
        builtin_type_indices.insert(builtin.name.to_string(), type_index);
    }

    let main_type_index = types.len();
    types.ty().function([], [ValType::I32]);

    module.section(&types);

    let mut imports = ImportSection::new();
    for (idx, builtin) in used_builtins.iter().enumerate() {
        let type_index = *builtin_type_indices
            .get(builtin.name)
            .expect("builtin type index present");
        imports.import(
            builtin.module,
            builtin.name,
            EntityType::Function(type_index as u32),
        );
        builtin_function_indices.insert(builtin.name.to_string(), idx as u32);
    }
    if !used_builtins.is_empty() {
        module.section(&imports);
    }

    let mut functions = FunctionSection::new();
    functions.function(main_type_index);
    module.section(&functions);

    let mut exports = ExportSection::new();
    let main_index = used_builtins.len() as u32;
    exports.export("main", ExportKind::Func, main_index);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new(vec![]);
    emit_expression(&expr, &mut function, &builtin_function_indices)?;
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);

    Ok(CompilationArtifact {
        wasm: module.finish(),
        stdlib,
        builtins: used_builtins
            .iter()
            .map(|builtin| builtins::to_descriptor(builtin))
            .collect(),
    })
}

pub fn emit_llvm_ir(source: &str, stdlib_root: impl AsRef<Path>) -> Result<String, CoreError> {
    let stdlib_root = stdlib_root.as_ref();
    let stdlib = load_stdlib_files(stdlib_root)
        .map_err(|_| CoreError::MissingStdlib(stdlib_root.to_path_buf()))?;
    let expr = parse(source)?;
    let value = evaluate(&expr)?.as_i32()?;

    let header = "; ModuleID = \"nepl-placeholder\"\nsource_filename = \"nepl-input\"\n";
    let mut lines = String::new();
    lines.push_str("define i32 @main() {\n");
    lines.push_str(&format!("  ret i32 {}\n", value));
    lines.push_str("}\n");
    lines.push_str(&format!("; stdlib files: {}\n", stdlib.len()));

    Ok(format!("{}{}", header, lines))
}

fn emit_expression(
    expr: &Expr,
    function: &mut Function,
    builtin_indices: &HashMap<String, u32>,
) -> Result<(), CoreError> {
    match expr {
        Expr::Number(value) => {
            function.instruction(&Instruction::I32Const(*value));
        }
        Expr::String(_) | Expr::Vector(_) => {
            emit_constant(expr, function)?;
        }
        Expr::Call { name, args } => match name.as_str() {
            "add" => emit_binary(args, function, builtin_indices, Instruction::I32Add)?,
            "sub" => emit_binary(args, function, builtin_indices, Instruction::I32Sub)?,
            "mul" => emit_binary(args, function, builtin_indices, Instruction::I32Mul)?,
            "div" => emit_binary(args, function, builtin_indices, Instruction::I32DivS)?,
            "mod" => emit_binary(args, function, builtin_indices, Instruction::I32RemS)?,
            "neg" => {
                function.instruction(&Instruction::I32Const(0));
                emit_expression(&args[0], function, builtin_indices)?;
                function.instruction(&Instruction::I32Sub);
            }
            "and" => emit_logic(args, function, builtin_indices, Instruction::I32And)?,
            "or" => emit_logic(args, function, builtin_indices, Instruction::I32Or)?,
            "xor" => emit_logic(args, function, builtin_indices, Instruction::I32Xor)?,
            "not" => {
                emit_expression(&args[0], function, builtin_indices)?;
                function.instruction(&Instruction::I32Eqz);
                function.instruction(&Instruction::I32Const(1));
                function.instruction(&Instruction::I32And);
            }
            "lt" => emit_compare(args, function, builtin_indices, Instruction::I32LtS)?,
            "le" => emit_compare(args, function, builtin_indices, Instruction::I32LeS)?,
            "eq" => emit_compare(args, function, builtin_indices, Instruction::I32Eq)?,
            "ne" => emit_compare(args, function, builtin_indices, Instruction::I32Ne)?,
            "gt" => emit_compare(args, function, builtin_indices, Instruction::I32GtS)?,
            "ge" => emit_compare(args, function, builtin_indices, Instruction::I32GeS)?,
            "bit_and" => emit_binary(args, function, builtin_indices, Instruction::I32And)?,
            "bit_or" => emit_binary(args, function, builtin_indices, Instruction::I32Or)?,
            "bit_xor" => emit_binary(args, function, builtin_indices, Instruction::I32Xor)?,
            "bit_not" => {
                function.instruction(&Instruction::I32Const(-1));
                emit_expression(&args[0], function, builtin_indices)?;
                function.instruction(&Instruction::I32Xor);
            }
            "bit_shl" => emit_binary(args, function, builtin_indices, Instruction::I32Shl)?,
            "bit_shr" => emit_binary(args, function, builtin_indices, Instruction::I32ShrS)?,
            builtin_name if builtin_indices.contains_key(builtin_name) => {
                for arg in args {
                    emit_expression(arg, function, builtin_indices)?;
                }
                let index = *builtin_indices.get(builtin_name).expect("index present");
                function.instruction(&Instruction::Call(index));
            }
            "len" | "concat" | "get" | "push" | "pop" => emit_constant(expr, function)?,
            _ => emit_constant(expr, function)?,
        },
    }
    Ok(())
}

fn emit_constant(expr: &Expr, function: &mut Function) -> Result<(), CoreError> {
    let value = evaluate(expr)?.as_i32()?;
    function.instruction(&Instruction::I32Const(value));
    Ok(())
}

fn emit_binary(
    args: &[Expr],
    function: &mut Function,
    builtin_indices: &HashMap<String, u32>,
    op: Instruction,
) -> Result<(), CoreError> {
    emit_expression(&args[0], function, builtin_indices)?;
    emit_expression(&args[1], function, builtin_indices)?;
    function.instruction(&op);
    Ok(())
}

fn emit_logic(
    args: &[Expr],
    function: &mut Function,
    builtin_indices: &HashMap<String, u32>,
    op: Instruction,
) -> Result<(), CoreError> {
    emit_truthy(&args[0], function, builtin_indices)?;
    emit_truthy(&args[1], function, builtin_indices)?;
    function.instruction(&op);
    Ok(())
}

fn emit_compare(
    args: &[Expr],
    function: &mut Function,
    builtin_indices: &HashMap<String, u32>,
    op: Instruction,
) -> Result<(), CoreError> {
    emit_expression(&args[0], function, builtin_indices)?;
    emit_expression(&args[1], function, builtin_indices)?;
    function.instruction(&op);
    Ok(())
}

fn emit_truthy(
    expr: &Expr,
    function: &mut Function,
    builtin_indices: &HashMap<String, u32>,
) -> Result<(), CoreError> {
    emit_expression(expr, function, builtin_indices)?;
    function.instruction(&Instruction::I32Const(0));
    function.instruction(&Instruction::I32Ne);
    Ok(())
}

fn validate(expr: &Expr) -> Result<(), CoreError> {
    evaluate(expr)?.as_i32().map(|_| ())
}

fn evaluate(expr: &Expr) -> Result<Value, CoreError> {
    match expr {
        Expr::Number(value) => Ok(Value::Number(*value)),
        Expr::String(value) => Ok(Value::Str(value.clone())),
        Expr::Vector(values) => {
            let mut evaluated = Vec::with_capacity(values.len());
            for value in values {
                evaluated.push(evaluate(value)?);
            }
            Ok(Value::Vector(evaluated))
        }
        Expr::Call { name, args } => match name.as_str() {
            "and" => {
                expect_arity(name, args.len(), 2)?;
                let left = truthy(evaluate(&args[0])?);
                let right = truthy(evaluate(&args[1])?);
                Ok(Value::Number(bool_to_i32(left && right)))
            }
            "or" => {
                expect_arity(name, args.len(), 2)?;
                let left = truthy(evaluate(&args[0])?);
                let right = truthy(evaluate(&args[1])?);
                Ok(Value::Number(bool_to_i32(left || right)))
            }
            "xor" => {
                expect_arity(name, args.len(), 2)?;
                let left = truthy(evaluate(&args[0])?);
                let right = truthy(evaluate(&args[1])?);
                Ok(Value::Number(bool_to_i32(left ^ right)))
            }
            "neg" => {
                expect_arity(name, args.len(), 1)?;
                let value = expect_number(evaluate(&args[0])?, "neg operand")?;
                Ok(Value::Number(value.wrapping_neg()))
            }
            "not" => {
                expect_arity(name, args.len(), 1)?;
                let value = truthy(evaluate(&args[0])?);
                Ok(Value::Number(bool_to_i32(!value)))
            }
            "add" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "add lhs")?;
                let right = expect_number(evaluate(&args[1])?, "add rhs")?;
                Ok(Value::Number(left.wrapping_add(right)))
            }
            "sub" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "sub lhs")?;
                let right = expect_number(evaluate(&args[1])?, "sub rhs")?;
                Ok(Value::Number(left.wrapping_sub(right)))
            }
            "mul" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "mul lhs")?;
                let right = expect_number(evaluate(&args[1])?, "mul rhs")?;
                Ok(Value::Number(left.wrapping_mul(right)))
            }
            "div" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "div lhs")?;
                let right = expect_number(evaluate(&args[1])?, "div rhs")?;
                if right == 0 {
                    return Err(CoreError::SemanticError(
                        "division by zero is not allowed".to_string(),
                    ));
                }
                Ok(Value::Number(left.wrapping_div(right)))
            }
            "mod" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "mod lhs")?;
                let right = expect_number(evaluate(&args[1])?, "mod rhs")?;
                if right == 0 {
                    return Err(CoreError::SemanticError(
                        "modulo by zero is not allowed".to_string(),
                    ));
                }
                Ok(Value::Number(left.wrapping_rem(right)))
            }
            "pow" => {
                expect_arity(name, args.len(), 2)?;
                let base = expect_number(evaluate(&args[0])?, "pow base")?;
                let exponent = expect_number(evaluate(&args[1])?, "pow exponent")?;
                if exponent < 0 {
                    return Err(CoreError::SemanticError(
                        "negative exponents are not supported".to_string(),
                    ));
                }
                let value = base
                    .checked_pow(exponent as u32)
                    .ok_or_else(|| CoreError::SemanticError("overflow during pow".to_string()))?;
                Ok(Value::Number(value))
            }
            "lt" => compare(name, args, |l, r| l < r),
            "le" => compare(name, args, |l, r| l <= r),
            "eq" => compare(name, args, |l, r| l == r),
            "ne" => compare(name, args, |l, r| l != r),
            "gt" => compare(name, args, |l, r| l > r),
            "ge" => compare(name, args, |l, r| l >= r),
            "bit_and" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "bit_and lhs")?;
                let right = expect_number(evaluate(&args[1])?, "bit_and rhs")?;
                Ok(Value::Number(left & right))
            }
            "bit_or" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "bit_or lhs")?;
                let right = expect_number(evaluate(&args[1])?, "bit_or rhs")?;
                Ok(Value::Number(left | right))
            }
            "bit_xor" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "bit_xor lhs")?;
                let right = expect_number(evaluate(&args[1])?, "bit_xor rhs")?;
                Ok(Value::Number(left ^ right))
            }
            "bit_not" => {
                expect_arity(name, args.len(), 1)?;
                let value = expect_number(evaluate(&args[0])?, "bit_not operand")?;
                Ok(Value::Number(!value))
            }
            "bit_shl" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "bit_shl lhs")?;
                let right = expect_number(evaluate(&args[1])?, "bit_shl rhs")?;
                Ok(Value::Number(left.wrapping_shl(right as u32)))
            }
            "bit_shr" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "bit_shr lhs")?;
                let right = expect_number(evaluate(&args[1])?, "bit_shr rhs")?;
                Ok(Value::Number(
                    ((left as u32).wrapping_shr(right as u32)) as i32,
                ))
            }
            "wasm_pagesize" => {
                expect_arity(name, args.len(), 0)?;
                Ok(Value::Number(65536))
            }
            "wasi_random" => {
                expect_arity(name, args.len(), 0)?;
                Ok(Value::Number(4))
            }
            "wasi_print" => {
                expect_arity(name, args.len(), 1)?;
                let value = expect_number(evaluate(&args[0])?, "wasi_print operand")?;
                Ok(Value::Number(value))
            }
            "gcd" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "gcd lhs")?;
                let right = expect_number(evaluate(&args[1])?, "gcd rhs")?;
                Ok(Value::Number(gcd(left, right)))
            }
            "lcm" => {
                expect_arity(name, args.len(), 2)?;
                let left = expect_number(evaluate(&args[0])?, "lcm lhs")?;
                let right = expect_number(evaluate(&args[1])?, "lcm rhs")?;
                let divisor = gcd(left, right);
                let value = left
                    .checked_div(divisor)
                    .and_then(|v| v.checked_mul(right))
                    .ok_or_else(|| CoreError::SemanticError("overflow during lcm".to_string()))?;
                Ok(Value::Number(value))
            }
            "factorial" => {
                expect_arity(name, args.len(), 1)?;
                let value = expect_number(evaluate(&args[0])?, "factorial operand")?;
                Ok(Value::Number(factorial(value)?))
            }
            "permutation" => {
                expect_arity(name, args.len(), 2)?;
                let n = expect_number(evaluate(&args[0])?, "permutation n")?;
                let r = expect_number(evaluate(&args[1])?, "permutation r")?;
                Ok(Value::Number(permutation(n, r)?))
            }
            "combination" => {
                expect_arity(name, args.len(), 2)?;
                let n = expect_number(evaluate(&args[0])?, "combination n")?;
                let r = expect_number(evaluate(&args[1])?, "combination r")?;
                Ok(Value::Number(combination(n, r)?))
            }
            "len" => {
                expect_arity(name, args.len(), 1)?;
                match evaluate(&args[0])? {
                    Value::Str(value) => Ok(Value::Number(value.chars().count() as i32)),
                    Value::Vector(values) => Ok(Value::Number(values.len() as i32)),
                    other => Err(CoreError::SemanticError(format!(
                        "len expects string or vector but found {:?}",
                        other,
                    ))),
                }
            }
            "concat" => {
                expect_arity(name, args.len(), 2)?;
                let left = evaluate(&args[0])?;
                let right = evaluate(&args[1])?;
                match (left, right) {
                    (Value::Str(l), Value::Str(r)) => Ok(Value::Str(format!("{l}{r}"))),
                    (Value::Vector(mut l), Value::Vector(r)) => {
                        l.extend(r);
                        Ok(Value::Vector(l))
                    }
                    (l, r) => Err(CoreError::SemanticError(format!(
                        "concat expects matching string or vector arguments but found {:?} and {:?}",
                        l, r,
                    ))),
                }
            }
            "get" => {
                expect_arity(name, args.len(), 2)?;
                let collection = evaluate(&args[0])?;
                let index = expect_number(evaluate(&args[1])?, "get index")?;
                match collection {
                    Value::Str(text) => {
                        let idx = to_index(index, text.chars().count(), "string index")?;
                        let value = text
                            .chars()
                            .nth(idx)
                            .map(|c| Value::Str(c.to_string()))
                            .unwrap_or_else(|| Value::Str(String::new()));
                        Ok(value)
                    }
                    Value::Vector(values) => {
                        let idx = to_index(index, values.len(), "vector index")?;
                        Ok(values.get(idx).cloned().ok_or_else(|| {
                            CoreError::SemanticError("index out of bounds".to_string())
                        })?)
                    }
                    other => Err(CoreError::SemanticError(format!(
                        "get expects string or vector but found {:?}",
                        other,
                    ))),
                }
            }
            "push" => {
                expect_arity(name, args.len(), 2)?;
                let collection = evaluate(&args[0])?;
                match collection {
                    Value::Str(mut text) => {
                        let suffix = match evaluate(&args[1])? {
                            Value::Str(value) => value,
                            other => {
                                return Err(CoreError::SemanticError(format!(
                                    "push on string expects string suffix but found {:?}",
                                    other,
                                )));
                            }
                        };
                        text.push_str(&suffix);
                        Ok(Value::Str(text))
                    }
                    Value::Vector(mut values) => {
                        let item = evaluate(&args[1])?;
                        values.push(item);
                        Ok(Value::Vector(values))
                    }
                    other => Err(CoreError::SemanticError(format!(
                        "push expects string or vector but found {:?}",
                        other,
                    ))),
                }
            }
            "pop" => {
                expect_arity(name, args.len(), 1)?;
                match evaluate(&args[0])? {
                    Value::Str(mut text) => {
                        if text.is_empty() {
                            return Err(CoreError::SemanticError(
                                "cannot pop from empty string".to_string(),
                            ));
                        }
                        text.pop();
                        Ok(Value::Str(text))
                    }
                    Value::Vector(mut values) => {
                        values.pop().ok_or_else(|| {
                            CoreError::SemanticError("cannot pop from empty vector".to_string())
                        })?;
                        Ok(Value::Vector(values))
                    }
                    other => Err(CoreError::SemanticError(format!(
                        "pop expects string or vector but found {:?}",
                        other,
                    ))),
                }
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
) -> Result<Value, CoreError> {
    expect_arity(name, args.len(), 2)?;
    let left = expect_number(evaluate(&args[0])?, "comparison lhs")?;
    let right = expect_number(evaluate(&args[1])?, "comparison rhs")?;
    Ok(Value::Number(bool_to_i32(predicate(left, right))))
}

fn truthy(value: Value) -> bool {
    match value {
        Value::Number(number) => number != 0,
        Value::Str(text) => !text.is_empty(),
        Value::Vector(values) => !values.is_empty(),
    }
}

fn bool_to_i32(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

fn expect_number(value: Value, context: &str) -> Result<i32, CoreError> {
    match value {
        Value::Number(number) => Ok(number),
        other => Err(CoreError::SemanticError(format!(
            "{context} expects numeric value but found {:?}",
            other,
        ))),
    }
}

fn to_index(index: i32, len: usize, context: &str) -> Result<usize, CoreError> {
    if index < 0 {
        return Err(CoreError::SemanticError(format!(
            "{context} cannot be negative",
        )));
    }
    let idx = index as usize;
    if idx >= len {
        return Err(CoreError::SemanticError(format!(
            "{context} {index} out of bounds for length {len}",
        )));
    }
    Ok(idx)
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
    fn records_and_executes_wasm_builtin() {
        let artifact =
            compile_wasm("wasm_pagesize", default_stdlib_root()).expect("compile should succeed");
        assert_eq!(artifact.builtins.len(), 1);
        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, &artifact.wasm).expect("module");
        let mut linker = wasmi::Linker::new(&engine);
        linker
            .func_wrap("env", "wasm_pagesize", || -> i32 { 4096 })
            .expect("link builtin");
        let mut store = wasmi::Store::new(&engine, ());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("instantiate");
        let main = instance
            .get_typed_func::<(), i32>(&store, "main")
            .expect("typed func");
        let result = main.call(&mut store, ()).expect("execute main");
        assert_eq!(result, 4096);
    }

    fn run_wasm_expression(source: &str) -> i32 {
        let artifact = compile_wasm(source, default_stdlib_root()).expect("compile should succeed");
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
        main.call(&mut store, ()).expect("run")
    }

    #[test]
    fn executes_logic_and_comparisons_in_wasm() {
        let result = run_wasm_expression("and (lt 1 2) (ge 5 5)");
        assert_eq!(result, 1);

        let result = run_wasm_expression("or (eq 0 1) (not 0)");
        assert_eq!(result, 1);
    }

    #[test]
    fn executes_bitwise_operations_in_wasm() {
        let result = run_wasm_expression("bit_or (bit_and 6 3) (bit_shl 1 2)");
        assert_eq!(result, 6);

        let result = run_wasm_expression("bit_xor (bit_not 0) 1");
        assert_eq!(result, -2);
    }

    #[test]
    fn executes_arithmetic_chains_in_wasm() {
        let result = run_wasm_expression("mod (sub 10 3) 4");
        assert_eq!(result, 3);

        let result = run_wasm_expression("add (mul 2 5) (div 20 4)");
        assert_eq!(result, 15);
    }

    #[test]
    fn executes_string_and_vector_expressions_in_wasm() {
        let string_result = run_wasm_expression("len concat \"abc\" \"de\"");
        assert_eq!(string_result, 5);

        let vector_result = run_wasm_expression("len pop push [4 5] 6");
        assert_eq!(vector_result, 2);
    }

    #[test]
    fn records_and_executes_wasi_builtins() {
        let artifact = compile_wasm("wasi_print (wasi_random)", default_stdlib_root())
            .expect("compile should succeed");
        assert_eq!(artifact.builtins.len(), 2);

        #[derive(Default)]
        struct HostState {
            log: Vec<i32>,
        }

        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, &artifact.wasm).expect("module");
        let mut linker = wasmi::Linker::new(&engine);
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "wasi_random",
                |mut caller: wasmi::Caller<'_, HostState>| -> i32 {
                    caller.data_mut().log.push(99);
                    99
                },
            )
            .expect("link random");
        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "wasi_print",
                |mut caller: wasmi::Caller<'_, HostState>, value: i32| -> i32 {
                    caller.data_mut().log.push(value);
                    value
                },
            )
            .expect("link print");

        let mut store = wasmi::Store::new(&engine, HostState::default());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .expect("instantiate");
        let main = instance
            .get_typed_func::<(), i32>(&store, "main")
            .expect("typed func");
        let result = main.call(&mut store, ()).expect("execute main");
        assert_eq!(result, 99);
        assert_eq!(store.data().log, vec![99, 99]);
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
    fn exposes_stdlib_contents_for_consumers() {
        let artifact =
            compile_wasm("add 1 2", default_stdlib_root()).expect("compile should succeed");

        let std_entry = artifact
            .stdlib
            .iter()
            .find(|file| file.path == PathBuf::from("std.nepl"))
            .expect("std.nepl should be present");

        assert!(std_entry.contents.contains("namespace std:"));
        assert!(artifact.builtins.is_empty());
    }

    #[test]
    fn evaluates_expression() {
        let expr = parse("add 1 (mul 2 3)").expect("parse");
        assert_eq!(evaluate(&expr).unwrap().as_i32().unwrap(), 7);
    }

    #[test]
    fn evaluates_pipe_expression() {
        let expr = parse("1 > neg > add 2").expect("parse");
        assert_eq!(evaluate(&expr).unwrap().as_i32().unwrap(), 1);
    }

    #[test]
    fn evaluates_extended_math_operations() {
        assert_eq!(
            evaluate(&parse("mod 10 3").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            1
        );
        assert_eq!(
            evaluate(&parse("pow 2 4").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            16
        );
        assert_eq!(
            evaluate(&parse("gcd 54 24").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            6
        );
        assert_eq!(
            evaluate(&parse("lcm 6 8").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            24
        );
        assert_eq!(
            evaluate(&parse("factorial 5").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            120
        );
        assert_eq!(
            evaluate(&parse("permutation 5 2").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            20
        );
        assert_eq!(
            evaluate(&parse("combination 5 2").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            10
        );
    }

    #[test]
    fn evaluates_comparison_and_bitwise_operations() {
        assert_eq!(
            evaluate(&parse("lt 1 2").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            1
        );
        assert_eq!(
            evaluate(&parse("ge 3 7").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            0
        );
        assert_eq!(
            evaluate(&parse("bit_and 6 3").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            2
        );
        assert_eq!(
            evaluate(&parse("bit_or 4 1").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            5
        );
        assert_eq!(
            evaluate(&parse("bit_not 0").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            -1
        );
        assert_eq!(
            evaluate(&parse("bit_shl 1 3").unwrap())
                .unwrap()
                .as_i32()
                .unwrap(),
            8
        );
    }

    #[test]
    fn evaluates_string_and_vector_operations() {
        let concat_len = evaluate(&parse("len concat \"na\" \"no\"").unwrap())
            .unwrap()
            .as_i32()
            .unwrap();
        assert_eq!(concat_len, 4);

        let trimmed_len = evaluate(&parse("len pop push \"hi\" \"!\"").unwrap())
            .unwrap()
            .as_i32()
            .unwrap();
        assert_eq!(trimmed_len, 2);

        let vector_value = evaluate(&parse("get [10 20 30] 1").unwrap())
            .unwrap()
            .as_i32()
            .unwrap();
        assert_eq!(vector_value, 20);

        let vector_len = evaluate(&parse("len pop push [1 2] 3").unwrap())
            .unwrap()
            .as_i32()
            .unwrap();
        assert_eq!(vector_len, 2);
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
