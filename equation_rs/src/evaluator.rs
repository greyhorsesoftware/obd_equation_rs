use crate::{
    callbacks::{DefaultPlatformCallbacks, PlatformCallbacks},
    error::Result,
    runtime::ExpressionResult,
    state::FunctionState,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct ExpressionEvaluator {
    states: Arc<Mutex<FunctionState>>,
    platform_callbacks: Box<dyn PlatformCallbacks + Sync>,
    /// NA4: compiled-AST memo — a dp's equation is constant, so one entry
    /// suffices (EQP2: one evaluator handle per dp).
    native_cache: Option<(String, Arc<crate::native::Ast>)>,
}

impl ExpressionEvaluator {
    /// Create a new evaluator with default platform callbacks
    pub fn new() -> Result<Self> {
        Self::new_with_platform_callbacks(Box::new(DefaultPlatformCallbacks))
    }

    /// Create a new evaluator with custom platform callbacks
    pub fn new_with_platform_callbacks(
        platform_callbacks: Box<dyn PlatformCallbacks + Sync>,
    ) -> Result<Self> {
        let states = Arc::new(Mutex::new(FunctionState::default()));

        Ok(Self {
            states,
            platform_callbacks,
            native_cache: None,
        })
    }

    /// EQP3: drop the compiled-expression memo while KEEPING the
    /// stateful-function accumulators — running averages survive. The iPad
    /// build wires this to memory-warning notifications; the next
    /// evaluation lazily recompiles (microseconds).
    pub fn release_runtime(&mut self) {
        self.native_cache = None;
    }

    pub fn evaluate(&mut self, expression: &str, variables: &HashMap<String, f64>) -> Result<f64> {
        let unified = self.eval_unified_impl(expression, variables, None, None)?;
        unified.as_f64().ok_or_else(|| {
            crate::error::EvaluatorError::TypeError("Expected numeric result".to_string())
        })
    }

    pub fn evaluate_with_bytes(
        &mut self,
        expression: &str,
        variables: &HashMap<String, f64>,
        bytes: &[u8],
    ) -> Result<f64> {
        let unified = self.eval_unified_impl(expression, variables, Some(bytes), None)?;
        unified.as_f64().ok_or_else(|| {
            crate::error::EvaluatorError::TypeError("Expected numeric result".to_string())
        })
    }

    /// Evaluate and return a unified result (numeric, string, or bool)
    pub fn evaluate_unified(
        &mut self,
        expression: &str,
        variables: &HashMap<String, f64>,
    ) -> Result<ExpressionResult> {
        self.eval_unified_impl(expression, variables, None, None)
    }

    /// Evaluate with bytes and return a unified result
    pub fn evaluate_with_bytes_unified(
        &mut self,
        expression: &str,
        variables: &HashMap<String, f64>,
        bytes: &[u8],
    ) -> Result<ExpressionResult> {
        self.eval_unified_impl(expression, variables, Some(bytes), None)
    }

    /// Evaluate with bytes and PID cross-references, return a unified result
    pub fn evaluate_with_bytes_and_pids_unified(
        &mut self,
        expression: &str,
        variables: &HashMap<String, f64>,
        bytes: &[u8],
        pid_values: &HashMap<String, f64>,
    ) -> Result<ExpressionResult> {
        self.eval_unified_impl(expression, variables, Some(bytes), Some(pid_values))
    }

    /// NA5: the native AST backend is the ONLY backend (QuickJS retired
    /// 2026-08-11; the recorded-oracle regression suite in
    /// tests/corpus_snapshot.rs pins dialect behavior to its last build).
    fn eval_unified_impl(
        &mut self,
        expression: &str,
        variables: &HashMap<String, f64>,
        bytes: Option<&[u8]>,
        pid_values: Option<&HashMap<String, f64>>,
    ) -> Result<ExpressionResult> {
        let ast = match &self.native_cache {
            Some((cached_expr, ast)) if cached_expr == expression => Arc::clone(ast),
            _ => {
                let ast = Arc::new(
                    crate::native::compile(expression)
                        .map_err(|e| crate::error::EvaluatorError::ParseError(e.to_string()))?,
                );
                self.native_cache = Some((expression.to_string(), Arc::clone(&ast)));
                ast
            }
        };
        let ctx = crate::native::EvalCtx {
            variables,
            bytes,
            pid_values,
            state: Arc::clone(&self.states),
            callbacks: &*self.platform_callbacks,
        };
        crate::native::eval(&ast, &ctx)
    }
}
