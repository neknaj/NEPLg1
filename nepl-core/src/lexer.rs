//! Lexer for NEPL core (no_std).

use alloc::vec::Vec;

use crate::diagnostic::Diagnostic;
use crate::span::{FileId, Span};

/// Kind of a token produced by the lexer.
///
/// The lexer is intentionally simple: it does not attach any
/// semantic meaning beyond recognizing keywords and basic literals.
/// Higher layers interpret identifiers and prefix sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Special
    Eof,

    // Identifiers and literals
    Ident,
    IntLiteral,
    FloatLiteral,
    StringLiteral,
    BoolLiteral, // true / false

    // Punctuation
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]
    Comma,      // ,
    Semi,       // ;
    Colon,      // :
    Dot,        // .
    Equal,      // =
    Greater,    // >

    // Compound punctuation / operators
    Arrow,      // ->
    StarArrow,  // *>
    DoubleColon,// ::

    Star,       // *
    FatArrow,   // =>

    // Keywords
    If,
    Then,
    ElseIf,
    Else,
    Loop,
    While,
    Match,
    Case,
    Break,
    Continue,
    Return,
    Let,
    Mut,
    Hoist,
    Pub,
    Fn,
    Include,
    Import,
    Namespace,
    Use,
    As,
    When,
    Enum,
    Struct,
    Set,
}

/// A single token with its kind and span.
///
/// The `text_start` / `text_end` fields are byte offsets into the
/// original source string, so that higher layers can retrieve the
/// concrete text when needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// Span in terms of file id and byte offsets.
    pub span: Span,
    /// Byte offsets into the source.
    pub text_start: u32,
    pub text_end: u32,
}

/// Result of lexing a source file.
#[derive(Debug)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lex a source string into tokens.
///
/// The returned `Token`s refer to slices of `source` via byte
/// offsets; the caller is responsible for keeping `source` alive
/// as long as those tokens are needed.
pub fn lex(file_id: FileId, source: &str) -> LexResult {
    let mut lexer = Lexer {
        file_id,
        source,
        chars: source.as_bytes(),
        len: source.len(),
        index: 0,
        diagnostics: Vec::new(),
    };
    lexer.run()
}

struct Lexer<'src> {
    file_id: FileId,
    source: &'src str,
    chars: &'src [u8],
    len: usize,
    index: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> Lexer<'src> {
    fn run(&mut self) -> LexResult {
        let mut tokens = Vec::new();

        while let Some(ch) = self.peek_char() {
            if is_whitespace(ch) {
                self.consume_char();
                continue;
            }

            let start = self.index as u32;
            let token = match ch {
                b'(' => {
                    self.consume_char();
                    self.simple_token(TokenKind::LParen, start)
                }
                b')' => {
                    self.consume_char();
                    self.simple_token(TokenKind::RParen, start)
                }
                b'{' => {
                    self.consume_char();
                    self.simple_token(TokenKind::LBrace, start)
                }
                b'}' => {
                    self.consume_char();
                    self.simple_token(TokenKind::RBrace, start)
                }
                b'[' => {
                    self.consume_char();
                    self.simple_token(TokenKind::LBracket, start)
                }
                b']' => {
                    self.consume_char();
                    self.simple_token(TokenKind::RBracket, start)
                }
                b',' => {
                    self.consume_char();
                    self.simple_token(TokenKind::Comma, start)
                }
                b';' => {
                    self.consume_char();
                    self.simple_token(TokenKind::Semi, start)
                }
                b':' => {
                    self.consume_char();
                    if self.peek_char() == Some(b':') {
                        self.consume_char();
                        self.simple_token(TokenKind::DoubleColon, start)
                    } else {
                        self.simple_token(TokenKind::Colon, start)
                    }
                }
                b'.' => {
                    self.consume_char();
                    self.simple_token(TokenKind::Dot, start)
                }
                b'=' => {
                    self.consume_char();
                    if self.peek_char() == Some(b'>') {
                        self.consume_char();
                        self.simple_token(TokenKind::FatArrow, start)
                    } else {
                        self.simple_token(TokenKind::Equal, start)
                    }
                }
                b'>' => {
                    self.consume_char();
                    self.simple_token(TokenKind::Greater, start)
                }
                b'-' => {
                    // Look for "->"
                    if self.peek_next() == Some(b'>') {
                        self.consume_char(); // '-'
                        self.consume_char(); // '>'
                        self.simple_token(TokenKind::Arrow, start)
                    } else {
                        self.consume_char();
                        self.unexpected_char(start)
                    }
                }
                b'*' => {
                    // Look for "*>"
                    if self.peek_next() == Some(b'>') {
                        self.consume_char(); // '*'
                        self.consume_char(); // '>'
                        self.simple_token(TokenKind::StarArrow, start)
                    } else {
                        self.consume_char();
                        self.simple_token(TokenKind::Star, start)
                    }
                }
                b'"' => self.lex_string(start),
                b'0'..=b'9' => self.lex_number(start),
                _ => {
                    if is_ident_start(ch) {
                        self.lex_ident_or_keyword(start)
                    } else {
                        self.consume_char();
                        self.unexpected_char(start)
                    }
                }
            };

            if let Some(tok) = token {
                tokens.push(tok);
            }
        }

        // EOF token at end
        let eof_span = Span::new(self.file_id, self.len as u32, self.len as u32);
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: eof_span,
            text_start: self.len as u32,
            text_end: self.len as u32,
        });

        LexResult {
            tokens,
            diagnostics: core::mem::take(&mut self.diagnostics),
        }
    }

    fn simple_token(&self, kind: TokenKind, start: u32) -> Option<Token> {
        let end = self.index as u32;
        Some(Token {
            kind,
            span: Span::new(self.file_id, start, end),
            text_start: start,
            text_end: end,
        })
    }

    fn unexpected_char(&mut self, start: u32) -> Option<Token> {
        let end = self.index as u32;
        let span = Span::new(self.file_id, start, end);
        let diag = Diagnostic::error("unexpected character", span)
            .with_code("E0001");
        self.diagnostics.push(diag);
        None
    }

    fn lex_string(&mut self, start: u32) -> Option<Token> {
        // Consume the opening quote
        self.consume_char();

        let content_start = self.index;
        while let Some(ch) = self.peek_char() {
            match ch {
                b'"' => {
                    let content_end = self.index;
                    self.consume_char(); // closing quote
                    let span = Span::new(self.file_id, start, self.index as u32);
                    return Some(Token {
                        kind: TokenKind::StringLiteral,
                        span,
                        text_start: content_start as u32,
                        text_end: content_end as u32,
                    });
                }
                b'\\' => {
                    // Skip over escape sequence: backslash + next char (if any)
                    self.consume_char();
                    if self.peek_char().is_some() {
                        self.consume_char();
                    }
                }
                _ => {
                    self.consume_char();
                }
            }
        }

        // Unterminated string
        let span = Span::new(self.file_id, start, self.index as u32);
        let diag = Diagnostic::error("unterminated string literal", span)
            .with_code("E0002");
        self.diagnostics.push(diag);
        None
    }

    fn lex_number(&mut self, start: u32) -> Option<Token> {
        // integer or float: digits [ '.' digits ]?
        while let Some(ch) = self.peek_char() {
            if matches!(ch, b'0'..=b'9' | b'_') {
                self.consume_char();
            } else {
                break;
            }
        }

        let mut is_float = false;

        if self.peek_char() == Some(b'.') {
            // Look ahead: if '.' followed by digit, treat as float.
            if let Some(next) = self.peek_next() {
                if (b'0'..=b'9').contains(&next) {
                    is_float = true;
                    self.consume_char(); // '.'
                    while let Some(ch) = self.peek_char() {
                        if matches!(ch, b'0'..=b'9' | b'_') {
                            self.consume_char();
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        let end = self.index as u32;
        let span = Span::new(self.file_id, start, end);
        let kind = if is_float {
            TokenKind::FloatLiteral
        } else {
            TokenKind::IntLiteral
        };
        Some(Token {
            kind,
            span,
            text_start: start,
            text_end: end,
        })
    }

    fn lex_ident_or_keyword(&mut self, start: u32) -> Option<Token> {
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                self.consume_char();
            } else {
                break;
            }
        }

        let end = self.index as u32;
        let span = Span::new(self.file_id, start, end);
        let text = &self.source[start as usize..end as usize];

        let kind = match text {
            "if" => TokenKind::If,
            "then" => TokenKind::Then,
            "elseif" => TokenKind::ElseIf,
            "else" => TokenKind::Else,
            "loop" => TokenKind::Loop,
            "while" => TokenKind::While,
            "match" => TokenKind::Match,
            "case" => TokenKind::Case,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" => TokenKind::Return,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "hoist" => TokenKind::Hoist,
            "pub" => TokenKind::Pub,
            "fn" => TokenKind::Fn,
            "include" => TokenKind::Include,
            "import" => TokenKind::Import,
            "namespace" => TokenKind::Namespace,
            "use" => TokenKind::Use,
            "as" => TokenKind::As,
            "when" => TokenKind::When,
            "enum" => TokenKind::Enum,
            "struct" => TokenKind::Struct,
            "set" => TokenKind::Set,
            "true" => TokenKind::BoolLiteral,
            "false" => TokenKind::BoolLiteral,
            _ => TokenKind::Ident,
        };

        Some(Token {
            kind,
            span,
            text_start: start,
            text_end: end,
        })
    }

    fn peek_char(&self) -> Option<u8> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.chars.get(self.index + 1).copied()
    }

    fn consume_char(&mut self) {
        if self.index < self.len {
            self.index += 1;
        }
    }
}

fn is_whitespace(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_ident_start(ch: u8) -> bool {
    (b'a'..=b'z').contains(&ch)
        || (b'A'..=b'Z').contains(&ch)
        || ch == b'_'
}

fn is_ident_continue(ch: u8) -> bool {
    is_ident_start(ch) || (b'0'..=b'9').contains(&ch)
}
