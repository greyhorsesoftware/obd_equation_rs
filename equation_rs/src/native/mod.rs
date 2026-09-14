//! Native AST evaluator for the Torque equation dialect (NA0–NA3).
//!
//! Compile once (`compile`) → evaluate many times (`eval`) with no string
//! rewriting and no JS engine. The dialect is implemented as its own small
//! language: f64 IEEE arithmetic, conventional operator precedence, numbers/
//! strings/bools as the only value kinds, nonzero-truth conditions —
//! extended exactly where the corpus or the bundled function library demanded
//! parity with the retired QuickJS backend (ToInt32 bitwise coercion, the
//! LOOKUP/CLOSEST arm quirks, string results from IF/LOOKUP/IFANY, and
//! `string + number` concatenation which the dialect docs use in
//! `LOOKUP(A::1='moo')+3`).
//!
//! Thread model: `Ast` is immutable and `Send + Sync`; `EvalCtx` borrows
//! shared inputs and a `Sync` callback object, with all mutable state behind
//! the existing `Arc<Mutex<FunctionState>>` — evaluate inline on any thread.
//!
//! Since NA5 (2026-08-11) this is the ONLY backend — the QuickJS path it
//! was built beside is retired. Dialect parity with that engine's last
//! build is pinned by the recorded-oracle suite in tests/corpus_snapshot.rs.

pub mod ast;
pub mod eval;
pub mod funcs;
pub mod parse;

pub use ast::{Ast, Expr, Span};
pub use parse::{compile, ParseError};

use crate::callbacks::PlatformCallbacks;
use crate::error::Result;
use crate::runtime::ExpressionResult;
use crate::state::FunctionState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Everything an evaluation needs. Mirrors the inputs of
/// `ExpressionEvaluator::evaluate_with_bytes_and_pids_unified`:
/// user variables, optional response bytes (byte variables A, B, … A1, …),
/// an optional PID-value map for `val{…}` (falls back to the platform
/// callback when absent), the shared stateful-function state, and the
/// platform callbacks (BARO).
pub struct EvalCtx<'a> {
    pub variables: &'a HashMap<String, f64>,
    pub bytes: Option<&'a [u8]>,
    pub pid_values: Option<&'a HashMap<String, f64>>,
    pub state: Arc<Mutex<FunctionState>>,
    pub callbacks: &'a (dyn PlatformCallbacks + Sync),
}

/// Evaluate a compiled expression, producing the same unified result type
/// (and the same string→number normalization) as the QuickJS backend.
pub fn eval(ast: &Ast, ctx: &EvalCtx) -> Result<ExpressionResult> {
    let v = eval::eval_expr(ast.root(), ctx, 0)?;
    Ok(eval::to_expression_result(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callbacks::DefaultPlatformCallbacks;

    // Compile-time Send + Sync assertions (NA2 ground rule).
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn ast_and_ctx_are_send_sync() {
        assert_send_sync::<Ast>();
        assert_send_sync::<EvalCtx<'static>>();
        assert_send_sync::<ExpressionResult>();
    }

    fn eval_str(src: &str, bytes: &[u8]) -> ExpressionResult {
        let ast = compile(src).expect("compile");
        let vars = HashMap::new();
        let cb = DefaultPlatformCallbacks;
        let ctx = EvalCtx {
            variables: &vars,
            bytes: Some(bytes),
            pid_values: None,
            state: Arc::new(Mutex::new(FunctionState::default())),
            callbacks: &cb,
        };
        eval(&ast, &ctx).expect("eval")
    }

    fn eval_num(src: &str, bytes: &[u8]) -> f64 {
        match eval_str(src, bytes) {
            ExpressionResult::Numeric(n) => n,
            other => panic!("expected numeric for {src:?}, got {other:?}"),
        }
    }

    #[test]
    fn smoke_arithmetic() {
        assert_eq!(eval_num("2 + 3", &[]), 5.0);
        assert_eq!(eval_num("A*256+B", &[0x1F, 0x40]), 8000.0);
        assert_eq!(eval_num("(A-48)", &[50]), 2.0);
        assert_eq!(eval_num("0.25 * (A*256 + B)", &[0x1F, 0x40]), 2000.0);
    }

    #[test]
    fn smoke_dialect() {
        assert_eq!(eval_num("BIT(A:3)", &[0b0000_1000]), 1.0);
        assert_eq!(eval_num("INT16(A:B)", &[1, 2]), 258.0);
        assert_eq!(eval_num("SIGNED(A)", &[200]), -56.0);
        assert_eq!(eval_num("IF A > 128 THEN 1 ELSE 0", &[200]), 1.0);
        match eval_str("IF BIT(A:0)==1 THEN 'ON' ELSE 'OFF'", &[1]) {
            ExpressionResult::Text(s) => assert_eq!(s, "ON"),
            other => panic!("expected text, got {other:?}"),
        }
        assert_eq!(eval_num("LOOKUP(A:0:1=100:2=200:3=300)", &[2]), 200.0);
        match eval_str(
            "LOOKUP(G:'Reserved':0='System Off':1='Active Cooling')",
            &[0, 0, 0, 0, 0, 0, 9],
        ) {
            ExpressionResult::Text(s) => assert_eq!(s, "Reserved"),
            other => panic!("expected text, got {other:?}"),
        }
        assert_eq!(
            eval_num(
                "((((A*16777216)+(B*65536)+(C*256)+D)>>24)&1)",
                &[0xFF, 0, 0, 0]
            ),
            1.0
        );
        assert_eq!(
            eval_num("ZEROREF( B == 128, 0, '0.7812 * B - 100' )", &[0, 64]),
            0.7812 * 64.0 - 100.0
        );
        assert_eq!(
            eval_num("ZEROREF( B == 128, 0, '0.7812 * B - 100' )", &[0, 128]),
            0.0
        );
    }

    #[test]
    fn smoke_division_by_zero_is_infinity() {
        assert_eq!(eval_num("1/A", &[0]), f64::INFINITY);
        assert_eq!(eval_num("-1/A", &[0]), f64::NEG_INFINITY);
        assert!(eval_num("A/B", &[0, 0]).is_nan());
    }
}
