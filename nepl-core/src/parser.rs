use crate::ast::Expr;
use crate::builtins;
use crate::error::CoreError;
use crate::lexer::{Token, TokenKind, lex};

pub fn parse(input: &str) -> Result<Expr, CoreError> {
    let tokens = lex(input)?;
    let mut position = 0;
    let expr = parse_pipe_expr(&tokens, &mut position)?;
    if position != tokens.len() {
        return Err(CoreError::ParseError(
            "unexpected trailing input".to_string(),
        ));
    }
    Ok(expr)
}

fn parse_pipe_expr(tokens: &[Token], position: &mut usize) -> Result<Expr, CoreError> {
    let mut lhs = parse_prefix_expr(tokens, position, false)?;
    while let Some(Token {
        kind: TokenKind::Pipe,
        ..
    }) = tokens.get(*position)
    {
        *position += 1;
        let rhs = parse_prefix_expr(tokens, position, true)?;
        lhs = desugar_pipe(lhs, rhs)?;
    }
    Ok(lhs)
}

fn parse_prefix_expr(
    tokens: &[Token],
    position: &mut usize,
    allow_partial_call: bool,
) -> Result<Expr, CoreError> {
    let token = tokens
        .get(*position)
        .ok_or_else(|| CoreError::ParseError("unexpected end of input".to_string()))?;
    *position += 1;
    match &token.kind {
        TokenKind::Number(value) => Ok(Expr::Number(*value)),
        TokenKind::LParen => {
            let expr = parse_pipe_expr(tokens, position)?;
            let closing = tokens
                .get(*position)
                .ok_or_else(|| CoreError::ParseError("unterminated parenthesis".to_string()))?;
            if matches!(closing.kind, TokenKind::RParen) {
                *position += 1;
                Ok(expr)
            } else {
                Err(CoreError::ParseError("expected ')'".to_string()))
            }
        }
        TokenKind::RParen => Err(CoreError::ParseError("unexpected ')'".to_string())),
        TokenKind::Pipe => Err(CoreError::ParseError("unexpected '>'".to_string())),
        TokenKind::Ident(name) => {
            let arity = operator_arity(name)
                .ok_or_else(|| CoreError::SemanticError(format!("unknown operator '{name}'")))?;
            let required_args = if allow_partial_call && arity > 0 {
                arity - 1
            } else {
                arity
            };
            let mut args = Vec::with_capacity(arity);
            for _ in 0..required_args {
                let arg = parse_pipe_expr(tokens, position)?;
                args.push(arg);
            }
            Ok(Expr::Call {
                name: name.clone(),
                args,
            })
        }
    }
}

fn desugar_pipe(lhs: Expr, rhs: Expr) -> Result<Expr, CoreError> {
    match rhs {
        Expr::Call { name, mut args } => {
            args.insert(0, lhs);
            Ok(Expr::Call { name, args })
        }
        _ => Err(CoreError::SemanticError(
            "pipe target must be a function application".to_string(),
        )),
    }
}

fn operator_arity(name: &str) -> Option<usize> {
    match name {
        "add" | "sub" | "mul" | "div" | "mod" | "pow" => Some(2),
        "and" | "or" | "xor" => Some(2),
        "lt" | "le" | "eq" | "ne" | "gt" | "ge" => Some(2),
        "bit_and" | "bit_or" | "bit_xor" | "bit_shl" | "bit_shr" => Some(2),
        "gcd" | "lcm" | "permutation" | "combination" => Some(2),
        "neg" | "not" | "bit_not" | "factorial" => Some(1),
        _ => builtins::operator_arity(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_expression() {
        let expr = parse("add 1 2").expect("parse");
        assert!(matches!(expr, Expr::Call { .. }));
    }

    #[test]
    fn parses_nested_expression_with_parens() {
        let expr = parse("add 1 (mul 2 3)").expect("parse");
        if let Expr::Call { name, args } = expr {
            assert_eq!(name, "add");
            assert_eq!(args.len(), 2);
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn rejects_unknown_operator() {
        let err = parse("foo 1 2").unwrap_err();
        assert!(matches!(err, CoreError::SemanticError(_)));
    }

    #[test]
    fn rejects_trailing_tokens() {
        let err = parse("add 1 2 3").unwrap_err();
        assert!(matches!(err, CoreError::ParseError(_)));
    }

    #[test]
    fn parses_pipe_chain() {
        let expr = parse("1 > neg").expect("parse");
        if let Expr::Call { name, args } = expr {
            assert_eq!(name, "neg");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Expr::Number(1)));
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn desugars_left_associative_pipe() {
        let expr = parse("1 > neg > add 2").expect("parse");
        if let Expr::Call { name, args } = expr {
            assert_eq!(name, "add");
            assert_eq!(args.len(), 2);
            assert!(matches!(args[1], Expr::Number(2)));
            if let Expr::Call {
                name: inner_name,
                args: inner_args,
            } = &args[0]
            {
                assert_eq!(inner_name, "neg");
                assert_eq!(inner_args.len(), 1);
            } else {
                panic!("expected nested call");
            }
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn rejects_pipe_without_call_rhs() {
        let err = parse("1 > 2").unwrap_err();
        assert!(matches!(err, CoreError::SemanticError(_)));
    }

    #[test]
    fn parses_extended_operators() {
        let expr = parse("pow 2 3").expect("parse pow");
        assert!(expr.is_call("pow"));

        let expr = parse("factorial 4").expect("parse factorial");
        assert!(expr.is_call("factorial"));
    }

    #[test]
    fn parses_builtins() {
        let expr = parse("wasm_pagesize").expect("parse wasm builtin");
        assert!(expr.is_call("wasm_pagesize"));

        let expr = parse("wasi_print 1").expect("parse wasi builtin");
        assert!(expr.is_call("wasi_print"));
    }
}
