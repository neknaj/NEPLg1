#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Number(i32),
    Call { name: String, args: Vec<Expr> },
}

impl Expr {
    pub fn is_call(&self, name: &str) -> bool {
        matches!(self, Expr::Call { name: n, .. } if n == name)
    }
}
