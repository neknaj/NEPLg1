//! Type checking and P-style resolution for NEPL (no_std, skeleton).
//!
//! このモジュールは AST から HIR への変換と型検査を担当する。
//!
//! - P-style の記法（`f x y` のような曖昧な prefix 列）は、
//!   ここでオーバーロード情報と型情報を見ながら解決する。
//! - 現段階ではまだ完全なアルゴリズムは実装せず、
//!   インタフェースと基本的な型付けロジックだけを用意する。

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::hir::{HirExpr, HirExprKind, HirIdent};
use crate::span::Span;
use crate::types::{Type, least_common_supertype};

/// Result of type checking a single AST expression.
///
/// - `expr` : 型付き HIR 表現（失敗したときは None の場合もある）
/// - `diagnostics` : 収集したエラーや警告
#[derive(Debug)]
pub struct TypeCheckResult {
    pub expr: Option<HirExpr>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Public entry point: type-check a root expression and produce HIR.
///
/// 将来的には `HirModule` や複数ファイルにまたがる解析を行うが、
/// ここではまず「ひとつの Expr」を対象とした関数を用意する。
pub fn typecheck_expr(root: &ast::Expr) -> TypeCheckResult {
    let mut checker = TypeChecker::new();
    let hir = checker.check_expr(root);
    TypeCheckResult {
        expr: hir,
        diagnostics: checker.diagnostics,
    }
}

/// Simple type environment for variables / functions.
///
/// no_std のためハッシュマップは使わず、線形探索のベクタで実装する。
#[derive(Debug, Default)]
struct TypeEnv {
    entries: Vec<(String, Type)>,
}

impl TypeEnv {
    fn new() -> Self {
        TypeEnv { entries: Vec::new() }
    }

    fn insert(&mut self, name: String, ty: Type) {
        self.entries.push((name, ty));
    }

    fn lookup(&self, name: &str) -> Option<&Type> {
        self.entries.iter().rev().find_map(|(n, t)| {
            if n == name {
                Some(t)
            } else {
                None
            }
        })
    }
}

/// Core type checker.
///
/// ここではまだ P-style の解決や複雑な制約処理は実装していない。
struct TypeChecker {
    env: TypeEnv,
    pub diagnostics: Vec<Diagnostic>,
}

impl TypeChecker {
    fn new() -> Self {
        TypeChecker {
            env: TypeEnv::new(),
            diagnostics: Vec::new(),
        }
    }

    fn error(&mut self, span: Span, msg: &str) {
        self.diagnostics.push(Diagnostic::error(msg.to_string(), span));
    }

    /// 型推論のメイン入口。
    fn check_expr(&mut self, expr: &ast::Expr) -> Option<HirExpr> {
        use ast::ExprKind;

        match &expr.kind {
            ExprKind::Literal(lit) => self.check_literal(expr.span, lit),
            ExprKind::Ident(ident) => self.check_ident(expr.span, ident),

            ExprKind::Seq(items) => self.check_pstyle_seq(expr.span, items),

            ExprKind::Pipe(pipe) => self.check_pipe(expr.span, pipe),

            ExprKind::Block(block) => self.check_block(expr.span, block),

            // ここではまだ未対応の構文については Unit 型＋診断を返す。
            _ => {
                self.error(expr.span, "type checking for this expression kind is not implemented yet");
                Some(HirExpr {
                    kind: HirExprKind::Unit,
                    ty: Type::Unit,
                    span: expr.span,
                })
            }
        }
    }

    fn check_literal(
        &mut self,
        span: Span,
        lit: &ast::LiteralKind,
    ) -> Option<HirExpr> {
        use ast::LiteralKind as L;

        let (kind, ty) = match lit {
            L::Int(text) => {
                // とりあえず i32 として扱う（将来は suffix などで変更可能）
                let value = text.parse::<i32>().unwrap_or(0);
                (HirExprKind::I32(value), Type::I32)
            }
            L::Float(text) => {
                let value = text.parse::<f64>().unwrap_or(0.0);
                (HirExprKind::F64(value), Type::F64)
            }
            L::String(s) => (HirExprKind::String(s.clone()), Type::Named("String".into())),
            L::Bool(b) => (HirExprKind::Bool(*b), Type::Bool),
        };

        Some(HirExpr { kind, ty, span })
    }

    fn check_ident(
        &mut self,
        span: Span,
        ident: &ast::Ident,
    ) -> Option<HirExpr> {
        let name = ident.name.clone();
        let ty = match self.env.lookup(&name) {
            Some(t) => t.clone(),
            None => {
                // 未定義識別子として警告し、とりあえず Unit を付ける
                self.error(span, "unresolved identifier");
                Type::Unit
            }
        };

        let hid = HirIdent { name, span };
        Some(HirExpr {
            kind: HirExprKind::Var(hid),
            ty,
            span,
        })
    }

    /// P-style prefix sequence `f x y ...`.
    ///
    /// 現段階では「最後の term の型を結果とみなす」簡易版で、
    /// 実際の関数適用やオーバーロード解決は未実装。
    fn check_pstyle_seq(
        &mut self,
        span: Span,
        items: &[ast::Expr],
    ) -> Option<HirExpr> {
        if items.is_empty() {
            self.error(span, "empty P-style sequence");
            return Some(HirExpr {
                kind: HirExprKind::Unit,
                ty: Type::Unit,
                span,
            });
        }

        // 今は単純に「各 term の型を推論し、最後の型を返す」だけにしている。
        let mut last_hir: Option<HirExpr> = None;
        for e in items {
            let h = self.check_expr(e);
            if h.is_some() {
                last_hir = h;
            }
        }

        if let Some(h) = last_hir {
            // TODO: P-style の解決と関数呼び出しへの変換をここで行う
            // 今は「実装されていない」という warning 的な Diagnostic を出す。
            self.error(span, "P-style call resolution is not implemented yet");
            Some(h)
        } else {
            self.error(span, "failed to type-check P-style sequence");
            Some(HirExpr {
                kind: HirExprKind::Unit,
                ty: Type::Unit,
                span,
            })
        }
    }

    /// Pipe chain `lhs > rhs1 > rhs2 > ...`.
    ///
    /// 現段階では「各 expr を順に型付けし、最後の型を結果」とするだけ。
    fn check_pipe(
        &mut self,
        span: Span,
        pipe: &ast::PipeChain,
    ) -> Option<HirExpr> {
        let mut last: Option<HirExpr> = None;

        let first = self.check_expr(&pipe.first);
        if first.is_some() {
            last = first;
        }

        for rhs in &pipe.rest {
            let h = self.check_expr(rhs);
            if h.is_some() {
                last = h;
            }
        }

        if let Some(h) = last {
            Some(HirExpr {
                span,
                ty: h.ty.clone(),
                kind: h.kind,
            })
        } else {
            self.error(span, "failed to type-check pipe expression");
            Some(HirExpr {
                kind: HirExprKind::Unit,
                ty: Type::Unit,
                span,
            })
        }
    }

    fn check_block(
        &mut self,
        span: Span,
        block: &ast::BlockExpr,
    ) -> Option<HirExpr> {
        if block.expressions.is_empty() {
            return Some(HirExpr {
                kind: HirExprKind::Block { expressions: Vec::new() },
                ty: Type::Unit,
                span,
            });
        }

        let mut hir_exprs = Vec::new();
        let mut last_ty = Type::Unit;

        for e in &block.expressions {
            if let Some(h) = self.check_expr(e) {
                last_ty = h.ty.clone();
                hir_exprs.push(h);
            }
        }

        Some(HirExpr {
            kind: HirExprKind::Block { expressions: hir_exprs },
            ty: last_ty,
            span,
        })
    }

    // ここに If / Match / Loop / While / Let / Set / Return / Break / Continue
    // などの型検査処理を、段階的に追加していく予定です。
    //
    // それらの実装では:
    //  - Never を bottom として扱う (least_common_supertype を利用)
    //  - Loop の中の break expr の型から Loop の型を決定する
    //  - 純粋関数( *>) と副作用あり関数( -> ) の制約をチェックする
}

/// 型の共通スーパータイプを「ブロックの最後の式」とマージする例。
///
/// 今は使っていないが、If / Match 実装時に利用する予定。
fn merge_types_for_branch(
    a: &Type,
    b: &Type,
    span: Span,
    diags: &mut Vec<Diagnostic>,
) -> Type {
    match least_common_supertype(a, b) {
        Some(t) => t,
        None => {
            diags.push(Diagnostic::error(
                "branches have incompatible types",
                span,
            ));
            Type::Unit
        }
    }
}
