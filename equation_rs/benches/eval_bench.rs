// NA6 criterion bench: warm-eval latency of the native AST backend.
//
// Acceptance targets (Native_AST_Evaluator_Plan NA6):
//   * simple equation warm eval — median ≤ 200 ns
//   * heavy corpus warm eval    — p95   ≤ 500 ns
// Reference numbers from the retired QuickJS backend (EQP3 era):
//   ~606 µs cold / 2–5 µs warm (18 µs with cached-runtime rebuild),
//   ~207 KB runtime footprint per gauge.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use obd_equation_rs::callbacks::DefaultPlatformCallbacks;
use obd_equation_rs::native::{self, EvalCtx};
use obd_equation_rs::state::FunctionState;
use obd_equation_rs::ExpressionEvaluator;
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::{Arc, Mutex};

const BYTES_LEN: usize = 42;

fn corpus() -> Vec<String> {
    let path = format!("{}/tests/corpus/equations.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .expect("corpus present")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

fn bench_evals(c: &mut Criterion) {
    let bytes = [0x5Au8; BYTES_LEN];
    let vars: HashMap<String, f64> = HashMap::new();
    let cb = DefaultPlatformCallbacks;

    // Simple: the classic RPM equation, direct native eval (compile once).
    {
        let ast = native::compile("(A*256+B)/4").unwrap();
        let state = Arc::new(Mutex::new(FunctionState::default()));
        c.bench_function("simple_warm_eval", |b| {
            b.iter(|| {
                let ctx = EvalCtx {
                    variables: &vars,
                    bytes: Some(black_box(&bytes)),
                    pid_values: None,
                    state: Arc::clone(&state),
                    callbacks: &cb,
                };
                black_box(native::eval(&ast, &ctx).unwrap())
            })
        });
    }

    // Public-API warm eval (memoized AST inside ExpressionEvaluator — the
    // per-datapoint hot path the app actually runs).
    {
        let mut ev = ExpressionEvaluator::new().unwrap();
        c.bench_function("simple_warm_eval_public_api", |b| {
            b.iter(|| {
                black_box(
                    ev.evaluate_with_bytes_unified("(A*256+B)/4", &vars, black_box(&bytes))
                        .unwrap(),
                )
            })
        });
    }

    // Heavy corpus: every corpus equation that compiles and evaluates
    // without pid context, round-robin — criterion's distribution over this
    // mixed workload gives the p95 the NA6 criterion asks for. Stateful
    // equations mutate their FunctionState; fresh state per batch keeps
    // growth bounded without measuring allocation storms.
    {
        let asts: Vec<native::Ast> = corpus()
            .iter()
            .filter_map(|eq| native::compile(eq).ok())
            .filter(|ast| {
                let ctx = EvalCtx {
                    variables: &vars,
                    bytes: Some(&bytes),
                    pid_values: None,
                    state: Arc::new(Mutex::new(FunctionState::default())),
                    callbacks: &cb,
                };
                native::eval(ast, &ctx).is_ok()
            })
            .collect();
        assert!(
            asts.len() > 300,
            "usable corpus unexpectedly small: {}",
            asts.len()
        );
        let mut i = 0usize;
        c.bench_function("corpus_warm_eval", |b| {
            b.iter_batched(
                || Arc::new(Mutex::new(FunctionState::default())),
                |state| {
                    let ast = &asts[i % asts.len()];
                    i = i.wrapping_add(1);
                    let ctx = EvalCtx {
                        variables: &vars,
                        bytes: Some(black_box(&bytes)),
                        pid_values: None,
                        state,
                        callbacks: &cb,
                    };
                    black_box(native::eval(ast, &ctx).ok())
                },
                BatchSize::SmallInput,
            )
        });
    }

    // Cold path: compile + eval (what an equation edit costs).
    c.bench_function("simple_cold_compile_eval", |b| {
        b.iter(|| {
            let ast = native::compile(black_box("(A*256+B)/4")).unwrap();
            let ctx = EvalCtx {
                variables: &vars,
                bytes: Some(&bytes),
                pid_values: None,
                state: Arc::new(Mutex::new(FunctionState::default())),
                callbacks: &cb,
            };
            black_box(native::eval(&ast, &ctx).unwrap())
        })
    });
}

criterion_group!(benches, bench_evals);
criterion_main!(benches);
