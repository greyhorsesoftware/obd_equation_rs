// NA6 per-equation p95 probe: times every corpus equation's warm eval
// individually (10k iters each after warmup) and reports the p50/p95/p99
// across per-equation MEDIANS — the distribution the NA6 "heavy corpus p95"
// criterion talks about. Run manually:
//     cargo test --release --test perf_probe -- --ignored --nocapture

use obd_equation_rs::callbacks::DefaultPlatformCallbacks;
use obd_equation_rs::native::{self, EvalCtx};
use obd_equation_rs::state::FunctionState;
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const BYTES_LEN: usize = 42;

#[test]
#[ignore]
fn corpus_per_equation_p95() {
    let corpus_path = format!("{}/tests/corpus/equations.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&corpus_path).unwrap();
    let bytes = [0x5Au8; BYTES_LEN];
    let vars: HashMap<String, f64> = HashMap::new();
    let cb = DefaultPlatformCallbacks;

    let mut medians_ns: Vec<f64> = Vec::new();
    for eq in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let Ok(ast) = native::compile(eq) else {
            continue;
        };
        let state = Arc::new(Mutex::new(FunctionState::default()));
        let mk_ctx = || EvalCtx {
            variables: &vars,
            bytes: Some(&bytes),
            pid_values: None,
            state: Arc::clone(&state),
            callbacks: &cb,
        };
        if native::eval(&ast, &mk_ctx()).is_err() {
            continue;
        }
        // Warmup then 5 timed rounds of 2000 evals; per-equation value =
        // median of the 5 round means (robust to scheduler noise).
        for _ in 0..2000 {
            let _ = black_box(native::eval(&ast, &mk_ctx()));
        }
        let mut rounds: Vec<f64> = (0..5)
            .map(|_| {
                let t0 = Instant::now();
                for _ in 0..2000 {
                    let _ = black_box(native::eval(&ast, &mk_ctx()));
                }
                t0.elapsed().as_nanos() as f64 / 2000.0
            })
            .collect();
        rounds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        medians_ns.push(rounds[2]);
    }

    medians_ns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| medians_ns[(((medians_ns.len() - 1) as f64) * p) as usize];
    eprintln!(
        "corpus per-equation warm eval over {} equations: p50 {:.0} ns, p95 {:.0} ns, p99 {:.0} ns, max {:.0} ns",
        medians_ns.len(),
        pct(0.50),
        pct(0.95),
        pct(0.99),
        medians_ns.last().unwrap()
    );
}
