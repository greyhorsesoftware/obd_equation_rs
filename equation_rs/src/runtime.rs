/// Unified result type that can hold numeric, string, or boolean values —
/// the shape every evaluation returns (kept in `runtime` for source
/// compatibility with the pre-NA5 two-backend layout).
#[derive(Debug, Clone)]
pub enum ExpressionResult {
    Numeric(f64),
    Text(String),
    Bool(bool),
}

impl ExpressionResult {
    /// Coerce to f64 for backward compatibility
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ExpressionResult::Numeric(v) => Some(*v),
            ExpressionResult::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
            ExpressionResult::Text(s) => s.parse::<f64>().ok(),
        }
    }
}
