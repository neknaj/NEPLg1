use crate::error::CoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    Number(i32),
    LParen,
    RParen,
    Pipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: usize,
}

pub fn lex(input: &str) -> Result<Vec<Token>, CoreError> {
    let mut tokens = Vec::new();
    let mut iter = input.char_indices().peekable();

    while let Some((idx, ch)) = iter.next() {
        match ch {
            '(' => tokens.push(Token {
                kind: TokenKind::LParen,
                position: idx,
            }),
            ')' => tokens.push(Token {
                kind: TokenKind::RParen,
                position: idx,
            }),
            '>' => tokens.push(Token {
                kind: TokenKind::Pipe,
                position: idx,
            }),
            c if c.is_whitespace() => continue,
            c if c.is_ascii_digit() || c == '-' => {
                let start = idx;
                let mut end = idx + ch.len_utf8();
                while let Some(&(next_idx, next_ch)) = iter.peek() {
                    if next_ch.is_ascii_digit() {
                        end = next_idx + next_ch.len_utf8();
                        iter.next();
                    } else {
                        break;
                    }
                }
                let slice = &input[start..end];
                let value: i32 = slice.parse().map_err(|_| CoreError::LexError {
                    position: start,
                    message: format!("invalid number literal '{slice}'"),
                })?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    position: start,
                });
            }
            c if is_ident_start(c) => {
                let start = idx;
                let mut end = idx + c.len_utf8();
                while let Some(&(next_idx, next_ch)) = iter.peek() {
                    if is_ident_continue(next_ch) {
                        end = next_idx + next_ch.len_utf8();
                        iter.next();
                    } else {
                        break;
                    }
                }
                let ident = input[start..end].to_string();
                tokens.push(Token {
                    kind: TokenKind::Ident(ident),
                    position: start,
                });
            }
            _ => {
                return Err(CoreError::LexError {
                    position: idx,
                    message: format!("unexpected character '{ch}'"),
                });
            }
        }
    }

    Ok(tokens)
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_numbers_and_idents() {
        let tokens = lex("add 1 (sub 3 2)").expect("lex should succeed");
        assert_eq!(tokens.len(), 7);
    }

    #[test]
    fn lexes_pipe_operator() {
        let tokens = lex("1 > neg").expect("lex should succeed");
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Pipe)));
    }

    #[test]
    fn reports_unexpected_character() {
        let err = lex("add $ 1").unwrap_err();
        assert!(matches!(err, CoreError::LexError { .. }));
    }
}
