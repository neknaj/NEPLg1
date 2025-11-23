use crate::ast::Expr;
use crate::error::CoreError;
use crate::lexer::{Token, TokenKind, lex};

pub fn parse(input: &str) -> Result<Expr, CoreError> {
    let tokens = lex(input)?;
    let mut position = 0;
    let expr = parse_expr(&tokens, &mut position)?;
    if position != tokens.len() {
        return Err(CoreError::ParseError(
            "unexpected trailing input".to_string(),
        ));
    }
    Ok(expr)
}

fn parse_expr(tokens: &[Token], position: &mut usize) -> Result<Expr, CoreError> {
    let token = tokens
        .get(*position)
        .ok_or_else(|| CoreError::ParseError("unexpected end of input".to_string()))?;
    *position += 1;
    match &token.kind {
        TokenKind::Number(value) => Ok(Expr::Number(*value)),
        TokenKind::LParen => {
            let expr = parse_expr(tokens, position)?;
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
        TokenKind::Ident(name) => {
            let arity = operator_arity(name)
                .ok_or_else(|| CoreError::SemanticError(format!("unknown operator '{name}'")))?;
            let mut args = Vec::with_capacity(arity);
            for _ in 0..arity {
                let arg = parse_expr(tokens, position)?;
                args.push(arg);
            }
            Ok(Expr::Call {
                name: name.clone(),
                args,
            })
        }
    }
}

fn operator_arity(name: &str) -> Option<usize> {
    match name {
        "add" | "sub" | "mul" | "div" => Some(2),
        "neg" => Some(1),
        _ => None,
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
}
