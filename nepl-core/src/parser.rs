//! Parser for NEPL core (no_std).
//!
//! This module turns tokens from the lexer into the surface AST defined
//! in `ast.rs`. It is **P-style aware** in the sense that:
//!
//! * P-style prefix sequences like `f x y` are represented as
//!   `ExprKind::Seq(Vec<Expr>)` without deciding which term is the
//!   function and which ones are arguments.
//! * Pipe chains `lhs > rhs1 > rhs2` are represented as `ExprKind::Pipe`.
//!
//! More semantic decisions (call trees, overload resolution, etc.) are
//! deferred to later phases.
//!
//! Error handling:
//! * The parser accumulates `Diagnostic`s instead of failing fast.
//! * When it hits an unexpected token, it tries simple error recovery
//!   (skipping to synchronisation points like `;`, `}`) so that it can
//!   report more than one error per file.

#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::lexer::{lex, LexResult, Token, TokenKind};
use crate::span::{FileId, Span};

/// Result of parsing a single NEPL source file.
#[derive(Debug)]
pub struct ParseResult {
    /// The top-level expression.
    ///
    /// According to the spec, a NEPL file is a single `<expr>`. If
    /// parsing fails badly, this may be `None` but diagnostics are
    /// still returned.
    pub expr: Option<Expr>,

    /// All diagnostics produced during lexing **and** parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a file in one shot: lex + parse.
///
/// This is a convenience entry point for CLI / playground usage. If
/// you already have a `LexResult`, you can call [`parse_tokens`]
/// instead.
pub fn parse_file(file_id: FileId, source: &str) -> ParseResult {
    let LexResult {
        tokens,
        diagnostics: mut lex_diags,
    } = lex(file_id, source);

    let mut parser = Parser::new(source, &tokens);
    let expr = parser.parse_root_expr();
    let mut diagnostics = lex_diags;
    diagnostics.extend(parser.into_diagnostics());

    ParseResult { expr, diagnostics }
}

/// Parse from an existing `LexResult`.
pub fn parse_tokens(source: &str, lex: &LexResult) -> ParseResult {
    let mut parser = Parser::new(source, &lex.tokens);
    let expr = parser.parse_root_expr();
    let mut diagnostics = lex.diagnostics.clone();
    diagnostics.extend(parser.into_diagnostics());
    ParseResult { expr, diagnostics }
}

/// Internal recursive-descent parser.
struct Parser<'src> {
    source: &'src str,
    tokens: &'src [Token],
    pos: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'src> Parser<'src> {
    fn new(source: &'src str, tokens: &'src [Token]) -> Self {
        Parser {
            source,
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
        }
    }

    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    // === basic cursor helpers =================================================

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn current(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or_else(|| self.tokens.last().expect("lexer always emits at least EOF"))
    }

    fn peek_kind(&self) -> TokenKind {
        self.current().kind.clone()
    }

    fn nth_kind(&self, n: usize) -> TokenKind {
        self.tokens
            .get(self.pos + n)
            .map(|t| t.kind.clone())
            .unwrap_or(TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.current().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn consume_if(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek_kind() == kind {
            Some(self.advance())
        } else {
            None
        }
    }

    fn expect(&mut self, expected: TokenKind, msg: &str) -> Option<Token> {
        let actual = self.peek_kind();
        if actual == expected {
            Some(self.advance())
        } else {
            let span = self.current().span;
            let diag = Diagnostic::error(msg, span);
            self.diagnostics.push(diag);
            None
        }
    }

    fn error(&mut self, span: Span, msg: &str) {
        let diag = Diagnostic::error(msg.to_string(), span);
        self.diagnostics.push(diag);
    }

    /// Merge two spans; if they are from different files, return the
    /// first span.
    fn merge_spans(&self, a: Span, b: Span) -> Span {
        if let Some(joined) = a.join(b) {
            joined
        } else {
            a
        }
    }

    /// Quick-and-dirty synchronisation: skip tokens until we see one
    /// of a few "safe" boundary tokens.
    fn sync_to_boundary(&mut self) {
        loop {
            match self.peek_kind() {
                TokenKind::Eof
                | TokenKind::Semi
                | TokenKind::RBrace
                | TokenKind::Case
                | TokenKind::Else
                | TokenKind::Enum
                | TokenKind::Struct => {
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    // === roots ================================================================

    /// Parse the whole file as a single `<expr>`, then ensure that
    /// only optional semicolons / whitespace remain.
    fn parse_root_expr(&mut self) -> Option<Expr> {
        if self.is_eof() {
            return None;
        }

        let expr = self.parse_expr()?;

        // Allow trailing semicolons or whitespace, but complain on
        // junk after the main expression.
        while self.consume_if(TokenKind::Semi).is_some() {}

        if !self.is_eof() {
            let span = self.current().span;
            self.error(span, "extra tokens after top-level expression");
        }

        Some(expr)
    }

    // === core expression grammar ==============================================

    /// `<expr>` = `<pipe_chain>`
    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_pipe_chain()
    }

    /// `lhs > rhs1 > rhs2 > ...`
    fn parse_pipe_chain(&mut self) -> Option<Expr> {
        let first = self.parse_seq_expr()?;
        let mut rest = Vec::new();

        while self.peek_kind() == TokenKind::Greater {
            let op_tok = self.advance(); // '>'
            if let Some(rhs) = self.parse_seq_expr() {
                rest.push(rhs);
            } else {
                // error already reported by inner parser; try to resync
                self.error(op_tok.span, "expected expression after '>'");
                self.sync_to_boundary();
                break;
            }
        }

        if rest.is_empty() {
            Some(first)
        } else {
            let span = self
                .merge_spans(first.span, rest.last().map(|e| e.span).unwrap_or(first.span));
            Some(Expr {
                kind: ExprKind::Pipe(PipeChain {
                    first: Box::new(first),
                    rest,
                }),
                span,
            })
        }
    }

    /// P-style prefix sequence:
    ///
    /// * At least one atom: `<atom_expr>`
    /// * Then zero or more atoms that follow **without** separators
    ///   like `;`, `}`, `)`, `>`, `case`, `else`, etc.
    ///
    /// `f x (g y)` becomes:
    ///   `Seq([Ident(f), Ident(x), Paren(Seq([...]))])`
    fn parse_seq_expr(&mut self) -> Option<Expr> {
        let mut items = Vec::new();

        let first = self.parse_atom_expr()?;
        items.push(first);

        while self.can_start_atom() && !self.is_seq_terminator() {
            if let Some(expr) = self.parse_atom_expr() {
                items.push(expr);
            } else {
                break;
            }
        }

        if items.len() == 1 {
            Some(items.pop().unwrap())
        } else {
            let first_span = items.first().unwrap().span;
            let last_span = items.last().unwrap().span;
            let span = self.merge_spans(first_span, last_span);
            Some(Expr {
                kind: ExprKind::Seq(items),
                span,
            })
        }
    }

    fn can_start_atom(&self) -> bool {
        match self.peek_kind() {
            TokenKind::Eof
            | TokenKind::RParen
            | TokenKind::RBrace
            | TokenKind::RBracket
            | TokenKind::Semi
            | TokenKind::Comma
            | TokenKind::Then
            | TokenKind::Else
            | TokenKind::Case
            | TokenKind::FatArrow
            | TokenKind::Greater => false,
            _ => true,
        }
    }

    fn is_seq_terminator(&self) -> bool {
        match self.peek_kind() {
            TokenKind::Eof
            | TokenKind::RParen
            | TokenKind::RBrace
            | TokenKind::RBracket
            | TokenKind::Semi
            | TokenKind::Comma
            | TokenKind::Then
            | TokenKind::Else
            | TokenKind::Case
            | TokenKind::FatArrow
            | TokenKind::Greater => true,
            _ => false,
        }
    }

    /// `<atom_expr>`: any expression form that can be a single term in
    /// a P-style sequence.
    fn parse_atom_expr(&mut self) -> Option<Expr> {
        match self.peek_kind() {
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Loop => self.parse_loop_expr(),
            TokenKind::While => self.parse_while_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::LBrace => self.parse_block_expr(),

            TokenKind::Let => self.parse_let_expr(),
            TokenKind::Fn => self.parse_let_function_expr(),
            TokenKind::Include => self.parse_include_expr(),
            TokenKind::Import => self.parse_import_expr(),
            TokenKind::Namespace | TokenKind::Pub => self.parse_namespace_or_use_or_type_def(),
            TokenKind::Use => self.parse_use_expr(),
            TokenKind::When => self.parse_when_expr(),

            TokenKind::Return => self.parse_return_expr(),
            TokenKind::Break => self.parse_break_expr(),
            TokenKind::Continue => self.parse_continue_expr(),
            TokenKind::Set => self.parse_set_expr(),

            TokenKind::Enum | TokenKind::Struct => self.parse_enum_or_struct_def(),

            TokenKind::LParen => self.parse_paren_expr(),
            TokenKind::IntLiteral
            | TokenKind::FloatLiteral
            | TokenKind::StringLiteral
            | TokenKind::BoolLiteral => self.parse_literal_expr(),
            TokenKind::Ident => self.parse_ident_expr(),

            // Anything else is unexpected here.
            _ => {
                let span = self.current().span;
                self.error(span, "expected expression");
                None
            }
        }
    }

    // === simple atoms: literals, identifiers, parens, blocks ================

    fn parse_literal_expr(&mut self) -> Option<Expr> {
        let tok = self.advance();
        let text = self.slice_token_text(&tok);
        let kind = match tok.kind {
            TokenKind::IntLiteral => LiteralKind::Int(text),
            TokenKind::FloatLiteral => LiteralKind::Float(text),
            TokenKind::StringLiteral => LiteralKind::String(text),
            TokenKind::BoolLiteral => {
                let v = match text.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => {
                        // lexing should ensure only "true"/"false"
                        self.error(tok.span, "invalid bool literal");
                        true
                    }
                };
                LiteralKind::Bool(v)
            }
            _ => {
                self.error(tok.span, "internal error: non-literal token in parse_literal_expr");
                return None;
            }
        };

        Some(Expr {
            kind: ExprKind::Literal(kind),
            span: tok.span,
        })
    }

    fn parse_ident_expr(&mut self) -> Option<Expr> {
        let tok = self.expect(TokenKind::Ident, "expected identifier")?;
        let name = self.slice_token_text(&tok);
        let ident = Ident {
            name,
            span: tok.span,
        };
        Some(Expr {
            kind: ExprKind::Ident(ident),
            span: tok.span,
        })
    }

    fn parse_paren_expr(&mut self) -> Option<Expr> {
        let l = self.expect(TokenKind::LParen, "expected '('")?;
        let expr = match self.parse_expr() {
            Some(e) => e,
            None => {
                self.error(l.span, "expected expression inside parentheses");
                self.sync_to_boundary();
                return None;
            }
        };
        let r = self.expect(TokenKind::RParen, "expected ')'")?;
        let span = self.merge_spans(l.span, r.span);
        Some(Expr {
            kind: ExprKind::Paren(Box::new(expr)),
            span,
        })
    }

    fn parse_block_expr(&mut self) -> Option<Expr> {
        let lbrace = self.expect(TokenKind::LBrace, "expected '{'")?;
        let mut exprs = Vec::new();

        while !self.is_eof() && self.peek_kind() != TokenKind::RBrace {
            if self.peek_kind() == TokenKind::Semi {
                self.advance();
                continue;
            }

            if let Some(e) = self.parse_expr() {
                exprs.push(e);
                // optional semicolon after each expression
                while self.consume_if(TokenKind::Semi).is_some() {}
            } else {
                self.sync_to_boundary();
                if self.peek_kind() == TokenKind::RBrace {
                    break;
                }
            }
        }

        let rbrace = self.expect(TokenKind::RBrace, "expected '}' to close block")?;
        let span = self.merge_spans(lbrace.span, rbrace.span);
        Some(Expr {
            kind: ExprKind::Block(BlockExpr { expressions: exprs }),
            span,
        })
    }

    // === control flow: if / loop / while / match ============================

    fn parse_if_expr(&mut self) -> Option<Expr> {
        let if_tok = self.expect(TokenKind::If, "expected 'if'")?;
        let cond = self.parse_expr().unwrap_or_else(|| {
            self.error(if_tok.span, "expected condition after 'if'");
            Expr {
                kind: ExprKind::Literal(LiteralKind::Bool(true)),
                span: if_tok.span,
            }
        });

        let _then = self.expect(TokenKind::Then, "expected 'then' after if condition")?;
        let then_body = self.parse_scoped_expr().unwrap_or_else(|| {
            self.error(cond.span, "expected body after 'then'");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: cond.span,
            }
        });

        let if_branch = IfBranch {
            condition: cond,
            body: then_body,
        };

        // zero or more `elseif`
        let mut elseif_branches = Vec::new();
        while self.peek_kind() == TokenKind::ElseIf {
            let elseif_tok = self.advance();
            let cond = self.parse_expr().unwrap_or_else(|| {
                self.error(elseif_tok.span, "expected condition after 'elseif'");
                Expr {
                    kind: ExprKind::Literal(LiteralKind::Bool(true)),
                    span: elseif_tok.span,
                }
            });
            let _then = self.expect(TokenKind::Then, "expected 'then' after elseif condition")?;
            let body = self.parse_scoped_expr().unwrap_or_else(|| {
                self.error(cond.span, "expected body after 'then'");
                Expr {
                    kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                    span: cond.span,
                }
            });
            elseif_branches.push(IfBranch { condition: cond, body });
        }

        // optional `else`, required by the spec
        let else_tok = self.expect(TokenKind::Else, "expected 'else' in if-expression")?;
        let else_body = self.parse_scoped_expr().unwrap_or_else(|| {
            self.error(else_tok.span, "expected else-body");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: else_tok.span,
            }
        });

        let span = self.merge_spans(if_tok.span, else_body.span);
        Some(Expr {
            kind: ExprKind::If(IfExpr {
                if_branch,
                elseif_branches,
                else_branch: else_body,
            }),
            span,
        })
    }

    fn parse_loop_expr(&mut self) -> Option<Expr> {
        let loop_tok = self.expect(TokenKind::Loop, "expected 'loop'")?;
        let body = self.parse_scoped_expr().unwrap_or_else(|| {
            self.error(loop_tok.span, "expected body after 'loop'");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: loop_tok.span,
            }
        });
        let span = self.merge_spans(loop_tok.span, body.span);
        Some(Expr {
            kind: ExprKind::Loop(LoopExpr { body }),
            span,
        })
    }

    fn parse_while_expr(&mut self) -> Option<Expr> {
        let while_tok = self.expect(TokenKind::While, "expected 'while'")?;
        let cond = self.parse_expr().unwrap_or_else(|| {
            self.error(while_tok.span, "expected condition after 'while'");
            Expr {
                kind: ExprKind::Literal(LiteralKind::Bool(true)),
                span: while_tok.span,
            }
        });
        let body = self.parse_scoped_expr().unwrap_or_else(|| {
            self.error(cond.span, "expected body after while condition");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: cond.span,
            }
        });
        let span = self.merge_spans(while_tok.span, body.span);
        Some(Expr {
            kind: ExprKind::While(WhileExpr { condition: cond, body }),
            span,
        })
    }

    fn parse_match_expr(&mut self) -> Option<Expr> {
        let match_tok = self.expect(TokenKind::Match, "expected 'match'")?;
        let scrutinee = self.parse_expr().unwrap_or_else(|| {
            self.error(match_tok.span, "expected scrutinee after 'match'");
            Expr {
                kind: ExprKind::Literal(LiteralKind::Bool(true)),
                span: match_tok.span,
            }
        });

        let cases = self.parse_scoped_list(|p| p.parse_match_case())?;
        let span = self.merge_spans(match_tok.span, cases.span);

        Some(Expr {
            kind: ExprKind::Match(MatchExpr { scrutinee, cases }),
            span,
        })
    }

    fn parse_match_case(&mut self) -> Option<MatchCase> {
        let case_tok = self.expect(TokenKind::Case, "expected 'case'")?;
        let pattern = self.parse_pattern()?;
        let arrow = self.expect(TokenKind::FatArrow, "expected '=>' after pattern")?;
        let body = self.parse_expr().unwrap_or_else(|| {
            self.error(arrow.span, "expected expression after '=>'");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: arrow.span,
            }
        });
        let span = self.merge_spans(case_tok.span, body.span);
        Some(MatchCase { pattern, body, span })
    }

    // === scoped_expr and scoped_list =========================================

    /// `<scoped_expr>`: either `{ exprs }` or `: expr` / `: { exprs }`.
    ///
    /// For now we fully support **braced** blocks and only very simple
    /// handling of the `:` offside form (we record `ScopeKind::Offside`
    /// for lists, but still rely on braces or simple termination).
    fn parse_scoped_expr(&mut self) -> Option<Expr> {
        match self.peek_kind() {
            TokenKind::LBrace => self.parse_block_expr(),
            TokenKind::Colon => {
                let colon = self.advance();
                // For now, treat `: expr` as "expr in a block" with
                // Offside scope. We don't yet track indentation.
                let expr = self.parse_expr().unwrap_or_else(|| {
                    self.error(colon.span, "expected expression after ':'");
                    Expr {
                        kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                        span: colon.span,
                    }
                });
                // Represent as a single-expression block.
                let span = self.merge_spans(colon.span, expr.span);
                Some(Expr {
                    kind: ExprKind::Block(BlockExpr {
                        expressions: vec![expr],
                    }),
                    span,
                })
            }
            _ => self.parse_expr(),
        }
    }

    /// `<scoped_list<T>>` helper used by `match`, `enum`, `struct`.
    ///
    /// Supports:
    /// * `{ item1; item2; ... }`  (ScopeKind::Braced)
    /// * `: item1; item2; ...`    (ScopeKind::Offside, simplified)
    fn parse_scoped_list<T>(
        &mut self,
        mut parse_item: impl FnMut(&mut Parser<'src>) -> Option<T>,
    ) -> Option<ScopedList<T>> {
        match self.peek_kind() {
            TokenKind::LBrace => self.parse_scoped_list_braced(&mut parse_item),
            TokenKind::Colon => self.parse_scoped_list_offside(&mut parse_item),
            _ => {
                let span = self.current().span;
                self.error(span, "expected '{' or ':' to start scoped list");
                None
            }
        }
    }

    fn parse_scoped_list_braced<T>(
        &mut self,
        parse_item: &mut impl FnMut(&mut Parser<'src>) -> Option<T>,
    ) -> Option<ScopedList<T>> {
        let lbrace = self.expect(TokenKind::LBrace, "expected '{'")?;
        let mut items = Vec::new();

        while !self.is_eof() && self.peek_kind() != TokenKind::RBrace {
            if self.peek_kind() == TokenKind::Semi {
                self.advance();
                continue;
            }
            if let Some(item) = parse_item(self) {
                items.push(item);
                // optional semicolon after each item
                while self.consume_if(TokenKind::Semi).is_some() {}
            } else {
                self.sync_to_boundary();
                if self.peek_kind() == TokenKind::RBrace {
                    break;
                }
            }
        }

        let rbrace = self.expect(TokenKind::RBrace, "expected '}'")?;
        let span = self.merge_spans(lbrace.span, rbrace.span);

        Some(ScopedList {
            kind: ScopeKind::Braced,
            items,
            span,
        })
    }

    fn parse_scoped_list_offside<T>(
        &mut self,
        parse_item: &mut impl FnMut(&mut Parser<'src>) -> Option<T>,
    ) -> Option<ScopedList<T>> {
        let colon = self.expect(TokenKind::Colon, "expected ':'")?;
        let mut items = Vec::new();

        // Very simple offside-style parsing:
        // parse items separated by semicolons or new expression starts
        // until we hit EOF or a token that clearly can't belong here
        // (like '}' or 'case' of an outer match).
        while !self.is_eof() {
            if matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Enum | TokenKind::Struct)
            {
                break;
            }
            if self.peek_kind() == TokenKind::Semi {
                self.advance();
                continue;
            }
            if let Some(item) = parse_item(self) {
                items.push(item);
                while self.consume_if(TokenKind::Semi).is_some() {}
            } else {
                self.sync_to_boundary();
                break;
            }
        }

        let span = if let Some(last_item_span) = items
            .last()
            .map(|_| colon.span) // approximation: we don't track per-item span here
        {
            self.merge_spans(colon.span, last_item_span)
        } else {
            colon.span
        };

        Some(ScopedList {
            kind: ScopeKind::Offside,
            items,
            span,
        })
    }

    // === let / fn / include / import / namespace / use / when ================

    fn parse_let_expr(&mut self) -> Option<Expr> {
        let let_tok = self.expect(TokenKind::Let, "expected 'let'")?;

        let mut is_mut = false;
        let mut is_hoist = false;

        if self.peek_kind() == TokenKind::Mut {
            self.advance();
            is_mut = true;
        }
        if self.peek_kind() == TokenKind::Hoist {
            self.advance();
            is_hoist = true;
        }

        let name_tok = self.expect(TokenKind::Ident, "expected identifier after 'let'")?;
        let name = self.slice_token_text(&name_tok);
        let ident = Ident {
            name,
            span: name_tok.span,
        };

        let value = if self.peek_kind() == TokenKind::Equal {
            self.advance();
            self.parse_expr().unwrap_or_else(|| {
                self.error(name_tok.span, "expected expression after '='");
                Expr {
                    kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                    span: name_tok.span,
                }
            })
        } else {
            self.error(name_tok.span, "expected '=' after let binding name");
            return None;
        };

        let span = self.merge_spans(let_tok.span, value.span);
        Some(Expr {
            kind: ExprKind::Let(LetExpr {
                is_pub: false,
                is_mut,
                is_hoist,
                name: ident,
                value,
            }),
            span,
        })
    }

    fn parse_let_function_expr(&mut self) -> Option<Expr> {
        let fn_tok = self.expect(TokenKind::Fn, "expected 'fn'")?;
        let name_tok = self.expect(TokenKind::Ident, "expected function name after 'fn'")?;
        let name = self.slice_token_text(&name_tok);
        let ident = Ident {
            name,
            span: name_tok.span,
        };

        let value = if self.peek_kind() == TokenKind::Equal {
            self.advance();
            self.parse_expr().unwrap_or_else(|| {
                self.error(name_tok.span, "expected function body after '='");
                Expr {
                    kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                    span: name_tok.span,
                }
            })
        } else {
            self.error(name_tok.span, "expected '=' after function name");
            return None;
        };

        let span = self.merge_spans(fn_tok.span, value.span);
        Some(Expr {
            kind: ExprKind::LetFunction(LetFunctionExpr {
                is_pub: false,
                name: ident,
                value,
            }),
            span,
        })
    }

    fn parse_include_expr(&mut self) -> Option<Expr> {
        let inc_tok = self.expect(TokenKind::Include, "expected 'include'")?;
        let path_tok = self.expect(TokenKind::StringLiteral, "expected string after 'include'")?;
        let path = self.slice_token_text(&path_tok);

        let span = self.merge_spans(inc_tok.span, path_tok.span);
        Some(Expr {
            kind: ExprKind::Include(IncludeExpr { path }),
            span,
        })
    }

    fn parse_import_expr(&mut self) -> Option<Expr> {
        let imp_tok = self.expect(TokenKind::Import, "expected 'import'")?;
        let name_tok = self.expect(TokenKind::Ident, "expected identifier after 'import'")?;
        let name = self.slice_token_text(&name_tok);
        let ident = Ident {
            name,
            span: name_tok.span,
        };
        let span = self.merge_spans(imp_tok.span, name_tok.span);
        Some(Expr {
            kind: ExprKind::Import(ImportExpr { name: ident }),
            span,
        })
    }

    /// Handles:
    /// * `namespace` ...
    /// * `pub namespace` ...
    /// * `enum` / `struct` with optional `pub`
    /// * `pub use` ...
    fn parse_namespace_or_use_or_type_def(&mut self) -> Option<Expr> {
        // If it's `pub` followed by `namespace` or `use` or `enum`/`struct`.
        if self.peek_kind() == TokenKind::Pub {
            let pub_tok = self.advance();
            match self.peek_kind() {
                TokenKind::Namespace => self.parse_namespace_expr_with_pub(pub_tok.span, true),
                TokenKind::Use => self.parse_use_expr_with_pub(pub_tok.span, true),
                TokenKind::Enum | TokenKind::Struct => {
                    self.parse_enum_or_struct_def_with_pub(pub_tok.span, true)
                }
                _ => {
                    self.error(pub_tok.span, "expected 'namespace', 'use', 'enum' or 'struct' after 'pub'");
                    None
                }
            }
        } else {
            match self.peek_kind() {
                TokenKind::Namespace => self.parse_namespace_expr_with_pub(self.current().span, false),
                TokenKind::Use => self.parse_use_expr_with_pub(self.current().span, false),
                TokenKind::Enum | TokenKind::Struct => {
                    self.parse_enum_or_struct_def_with_pub(self.current().span, false)
                }
                _ => {
                    let span = self.current().span;
                    self.error(span, "expected 'namespace', 'use', 'enum', or 'struct'");
                    None
                }
            }
        }
    }

    fn parse_namespace_expr_with_pub(&mut self, pub_span: Span, is_pub: bool) -> Option<Expr> {
        let ns_tok = self.expect(TokenKind::Namespace, "expected 'namespace'")?;
        let name_tok = self.expect(TokenKind::Ident, "expected namespace name")?;
        let name = self.slice_token_text(&name_tok);
        let ident = Ident {
            name,
            span: name_tok.span,
        };
        let body = self.parse_scoped_expr().unwrap_or_else(|| {
            self.error(ns_tok.span, "expected namespace body");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: ns_tok.span,
            }
        });
        let span = self.merge_spans(if is_pub { pub_span } else { ns_tok.span }, body.span);
        Some(Expr {
            kind: ExprKind::Namespace(NamespaceExpr {
                is_pub,
                name: ident,
                body,
            }),
            span,
        })
    }

    fn parse_namespace_expr(&mut self) -> Option<Expr> {
        self.parse_namespace_expr_with_pub(self.current().span, false)
    }

    fn parse_use_expr(&mut self) -> Option<Expr> {
        self.parse_use_expr_with_pub(self.current().span, false)
    }

    fn parse_use_expr_with_pub(&mut self, pub_span: Span, is_pub: bool) -> Option<Expr> {
        let use_tok = self.expect(TokenKind::Use, "expected 'use'")?;

        // Parse a path like `ns1::ns2::*` or a plain identifier.
        let path = self.parse_use_path()?;
        let alias = if self.peek_kind() == TokenKind::As {
            self.advance();
            let alias_tok = self.expect(TokenKind::Ident, "expected alias name after 'as'")?;
            let name = self.slice_token_text(&alias_tok);
            Some(Ident {
                name,
                span: alias_tok.span,
            })
        } else {
            None
        };

        let span = self.merge_spans(if is_pub { pub_span } else { use_tok.span }, path_span(&path));
        Some(Expr {
            kind: ExprKind::Use(UseExpr {
                is_pub,
                path,
                alias,
            }),
            span,
        })
    }

    fn parse_use_path(&mut self) -> Option<Path> {
        // Simple path: `segment (:: segment)* [:: *]`
        let mut segments = Vec::new();
        let first = self.expect(TokenKind::Ident, "expected path segment")?;
        segments.push(self.slice_token_text(&first));

        while self.peek_kind() == TokenKind::DoubleColon {
            self.advance();
            if self.peek_kind() == TokenKind::Star {
                self.advance();
                return Some(Path::Glob {
                    segments,
                    // `*` span is approximated as current token's span
                });
            }
            let seg_tok = self.expect(TokenKind::Ident, "expected path segment after '::'")?;
            segments.push(self.slice_token_text(&seg_tok));
        }

        Some(Path::Simple { segments })
    }

    fn parse_when_expr(&mut self) -> Option<Expr> {
        let when_tok = self.expect(TokenKind::When, "expected 'when'")?;
        let cond = self.parse_expr().unwrap_or_else(|| {
            self.error(when_tok.span, "expected condition after 'when'");
            Expr {
                kind: ExprKind::Literal(LiteralKind::Bool(true)),
                span: when_tok.span,
            }
        });
        let body = self.parse_scoped_expr().unwrap_or_else(|| {
            self.error(cond.span, "expected body after 'when' condition");
            Expr {
                kind: ExprKind::Block(BlockExpr { expressions: Vec::new() }),
                span: cond.span,
            }
        });
        let span = self.merge_spans(when_tok.span, body.span);
        Some(Expr {
            kind: ExprKind::When(WhenExpr { condition: cond, body }),
            span,
        })
    }

    // === return / break / continue / set ======================================

    fn parse_return_expr(&mut self) -> Option<Expr> {
        let ret_tok = self.expect(TokenKind::Return, "expected 'return'")?;
        let expr = if self.is_seq_terminator() {
            None
        } else {
            self.parse_expr()
        };
        let span = if let Some(ref e) = expr {
            self.merge_spans(ret_tok.span, e.span)
        } else {
            ret_tok.span
        };
        Some(Expr {
            kind: ExprKind::Return(ReturnExpr { value: expr }),
            span,
        })
    }

    fn parse_break_expr(&mut self) -> Option<Expr> {
        let br_tok = self.expect(TokenKind::Break, "expected 'break'")?;
        let expr = if self.is_seq_terminator() {
            None
        } else {
            self.parse_expr()
        };
        let span = if let Some(ref e) = expr {
            self.merge_spans(br_tok.span, e.span)
        } else {
            br_tok.span
        };
        Some(Expr {
            kind: ExprKind::Break(BreakExpr { value: expr }),
            span,
        })
    }

    fn parse_continue_expr(&mut self) -> Option<Expr> {
        let c_tok = self.expect(TokenKind::Continue, "expected 'continue'")?;
        Some(Expr {
            kind: ExprKind::Continue(ContinueExpr {}),
            span: c_tok.span,
        })
    }

    fn parse_set_expr(&mut self) -> Option<Expr> {
        let set_tok = self.expect(TokenKind::Set, "expected 'set'")?;
        let target = self.parse_assignable().unwrap_or_else(|| {
            self.error(set_tok.span, "expected assignment target after 'set'");
            Assignable {
                base: Expr {
                    kind: ExprKind::Ident(Ident {
                        name: "<error>".to_string(),
                        span: set_tok.span,
                    }),
                    span: set_tok.span,
                },
                fields: Vec::new(),
            }
        });
        let value = self.parse_expr().unwrap_or_else(|| {
            self.error(set_tok.span, "expected value expression after assignment target");
            Expr {
                kind: ExprKind::Literal(LiteralKind::Bool(true)),
                span: set_tok.span,
            }
        });
        let span = self.merge_spans(set_tok.span, value.span);
        Some(Expr {
            kind: ExprKind::Set(SetExpr { target, value }),
            span,
        })
    }

    fn parse_assignable(&mut self) -> Option<Assignable> {
        // For now, parse a simple identifier or `expr.field`
        let base = self.parse_atom_expr()?;
        let mut fields = Vec::new();

        loop {
            if self.peek_kind() == TokenKind::Dot {
                self.advance();
                let field_tok =
                    self.expect(TokenKind::Ident, "expected field name after '.' in assignment")?;
                let name = self.slice_token_text(&field_tok);
                fields.push(Ident {
                    name,
                    span: field_tok.span,
                });
            } else {
                break;
            }
        }

        Some(Assignable { base, fields })
    }

    // === enum / struct definitions ===========================================

    fn parse_enum_or_struct_def(&mut self) -> Option<Expr> {
        self.parse_enum_or_struct_def_with_pub(self.current().span, false)
    }

    fn parse_enum_or_struct_def_with_pub(
        &mut self,
        pub_span: Span,
        is_pub: bool,
    ) -> Option<Expr> {
        match self.peek_kind() {
            TokenKind::Enum => self.parse_enum_def(pub_span, is_pub),
            TokenKind::Struct => self.parse_struct_def(pub_span, is_pub),
            _ => {
                let span = self.current().span;
                self.error(span, "expected 'enum' or 'struct'");
                None
            }
        }
    }

    fn parse_enum_def(&mut self, pub_span: Span, is_pub: bool) -> Option<Expr> {
        let enum_tok = self.expect(TokenKind::Enum, "expected 'enum'")?;
        let name_tok = self.expect(TokenKind::Ident, "expected enum name")?;
        let name = self.slice_token_text(&name_tok);
        let ident = Ident {
            name,
            span: name_tok.span,
        };

        let variants = self.parse_scoped_list(|p| p.parse_enum_variant())?;
        let span = self.merge_spans(if is_pub { pub_span } else { enum_tok.span }, variants.span);

        Some(Expr {
            kind: ExprKind::EnumDef(EnumDefExpr {
                is_pub,
                name: ident,
                variants,
            }),
            span,
        })
    }

    fn parse_enum_variant(&mut self) -> Option<EnumVariant> {
        let name_tok = self.expect(TokenKind::Ident, "expected enum variant name")?;
        let name = self.slice_token_text(&name_tok);
        let name_ident = Ident {
            name,
            span: name_tok.span,
        };

        let mut payload_types = Vec::new();
        if self.peek_kind() == TokenKind::LParen {
            self.advance();
            // simple `type_list` grammar: type (',' type)*
            if self.peek_kind() != TokenKind::RParen {
                loop {
                    let ty = self.parse_type_expr()?;
                    payload_types.push(ty);
                    if self.peek_kind() == TokenKind::Comma {
                        self.advance();
                        continue;
                    } else {
                        break;
                    }
                }
            }
            let _ = self.expect(TokenKind::RParen, "expected ')' after enum variant payload");
        }

        Some(EnumVariant {
            name: name_ident,
            payload_types,
        })
    }

    fn parse_struct_def(&mut self, pub_span: Span, is_pub: bool) -> Option<Expr> {
        let struct_tok = self.expect(TokenKind::Struct, "expected 'struct'")?;
        let name_tok = self.expect(TokenKind::Ident, "expected struct name")?;
        let name = self.slice_token_text(&name_tok);
        let ident = Ident {
            name,
            span: name_tok.span,
        };

        let fields = self.parse_scoped_list(|p| p.parse_struct_field())?;
        let span =
            self.merge_spans(if is_pub { pub_span } else { struct_tok.span }, fields.span);

        Some(Expr {
            kind: ExprKind::StructDef(StructDefExpr {
                is_pub,
                name: ident,
                fields,
            }),
            span,
        })
    }

    fn parse_struct_field(&mut self) -> Option<StructField> {
        let name_tok = self.expect(TokenKind::Ident, "expected field name")?;
        let name = self.slice_token_text(&name_tok);
        let name_ident = Ident {
            name,
            span: name_tok.span,
        };
        self.expect(TokenKind::Colon, "expected ':' after field name")?;
        let ty = self.parse_type_expr()?;
        Some(StructField { name: name_ident, ty })
    }

    // === pattern parsing ======================================================

    fn parse_pattern(&mut self) -> Option<Pattern> {
        match self.peek_kind() {
            TokenKind::Ident => self.parse_ident_or_composite_pattern(),
            TokenKind::IntLiteral
            | TokenKind::FloatLiteral
            | TokenKind::StringLiteral
            | TokenKind::BoolLiteral => {
                let lit_expr = self.parse_literal_expr()?;
                Some(Pattern::Literal(lit_expr))
            }
            TokenKind::Underscore => {
                let tok = self.advance();
                Some(Pattern::Wildcard(tok.span))
            }
            _ => {
                let span = self.current().span;
                self.error(span, "expected pattern");
                None
            }
        }
    }

    fn parse_ident_or_composite_pattern(&mut self) -> Option<Pattern> {
        let head_tok = self.expect(TokenKind::Ident, "expected identifier in pattern")?;
        let head_name = self.slice_token_text(&head_tok);

        // Look ahead:
        // * `Ident` alone  -> variable pattern
        // * `Ident` '(' ... ')' -> enum constructor pattern
        // * `Ident` '{' ... '}' -> struct pattern
        //
        // Later phases will distinguish between "enum name" vs
        // "variable name".
        match self.peek_kind() {
            TokenKind::LParen => {
                // Enum variant pattern: Name(p1, p2, ...)
                self.advance(); // '('
                let mut args = Vec::new();
                if self.peek_kind() != TokenKind::RParen {
                    loop {
                        let pat = self.parse_pattern()?;
                        args.push(pat);
                        if self.peek_kind() == TokenKind::Comma {
                            self.advance();
                            continue;
                        } else {
                            break;
                        }
                    }
                }
                let _ = self.expect(TokenKind::RParen, "expected ')' in enum pattern");
                Some(Pattern::Enum(EnumPattern {
                    ctor: head_name,
                    args,
                    span: head_tok.span,
                }))
            }
            TokenKind::LBrace => {
                // Struct pattern: Name { field: pat, ... }
                self.advance(); // '{'
                let mut fields = Vec::new();
                while !self.is_eof() && self.peek_kind() != TokenKind::RBrace {
                    let field_tok =
                        self.expect(TokenKind::Ident, "expected field name in struct pattern")?;
                    let field_name = self.slice_token_text(&field_tok);
                    self.expect(TokenKind::Colon, "expected ':' after field name in pattern")?;
                    let pat = self.parse_pattern()?;
                    fields.push(StructPatternField {
                        field: field_name,
                        pattern: pat,
                    });
                    if self.peek_kind() == TokenKind::Comma || self.peek_kind() == TokenKind::Semi {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let _ = self.expect(TokenKind::RBrace, "expected '}' to close struct pattern");
                Some(Pattern::Struct(StructPattern {
                    ctor: head_name,
                    fields,
                    span: head_tok.span,
                }))
            }
            _ => {
                // Simple variable pattern.
                Some(Pattern::Ident(head_name, head_tok.span))
            }
        }
    }

    // === type expressions (for enum payloads / struct fields) =================
    //
    // For now we use a very simple subset that matches the current
    // specification enough to build AST nodes. This can be extended
    // later.

    fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        // Very simple type grammar:
        // <type> = <type_atom> [ '->' <type> ]
        let lhs = self.parse_type_atom()?;
        if self.peek_kind() == TokenKind::Arrow {
            let arrow_tok = self.advance();
            let rhs = self.parse_type_expr().unwrap_or_else(|| {
                self.error(arrow_tok.span, "expected type after '->'");
                lhs.clone()
            });
            let span = self.merge_spans(lhs.span, rhs.span);
            Some(TypeExpr {
                span,
                kind: TypeExprKind::Fn(Vec::new(), Box::new(lhs), Box::new(rhs)),
            })
        } else {
            Some(lhs)
        }
    }

    fn parse_type_atom(&mut self) -> Option<TypeExpr> {
        match self.peek_kind() {
            TokenKind::Ident => {
                let tok = self.advance();
                let name = self.slice_token_text(&tok);
                Some(TypeExpr {
                    span: tok.span,
                    kind: TypeExprKind::Named(name),
                })
            }
            TokenKind::LParen => {
                let l = self.advance();
                let inner = self.parse_type_expr()?;
                let r = self.expect(TokenKind::RParen, "expected ')' in type")?;
                let span = self.merge_spans(l.span, r.span);
                Some(TypeExpr {
                    span,
                    kind: TypeExprKind::Paren(Box::new(inner)),
                })
            }
            _ => {
                let span = self.current().span;
                self.error(span, "expected type");
                None
            }
        }
    }

    // === helpers =============================================================

    fn slice_token_text(&self, tok: &Token) -> String {
        let start = tok.text_start as usize;
        let end = tok.text_end as usize;
        self.source[start..end].to_owned()
    }
}

/// Helper to approximate a `Span` covering a `Path`.
fn path_span(path: &Path) -> Span {
    match path {
        Path::Simple { .. } | Path::Glob { .. } => Span::dummy(),
    }
}
