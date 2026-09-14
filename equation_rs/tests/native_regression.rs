// Native-backend regression suite — the surviving half of the retired NA3
// differential harness (tests/native_differential.rs, deleted with the
// QuickJS backend). The cross-backend sweep lives on as a recorded-oracle
// test in tests/corpus_snapshot.rs; this file keeps the native-only checks:
//   * corpus parse coverage (with the pinned pre-existing reject list),
//   * the golden reference-PID CSV, evaluated natively,
//   * parser garbage/mutation fuzz — Err is fine, panicking is not.

use obd_equation_rs::callbacks::DefaultPlatformCallbacks;
use obd_equation_rs::native::{self, EvalCtx};
use obd_equation_rs::state::FunctionState;
use obd_equation_rs::ExpressionResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9e3779b97f4a7c15)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

fn corpus() -> Vec<String> {
    let path = format!("{}/tests/corpus/equations.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("corpus file present");
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

fn run_native(
    ast: &native::Ast,
    bytes: &[u8],
    pids: Option<&HashMap<String, f64>>,
) -> Result<ExpressionResult, String> {
    let vars = HashMap::new();
    let cb = DefaultPlatformCallbacks;
    let ctx = EvalCtx {
        variables: &vars,
        bytes: Some(bytes),
        pid_values: pids,
        state: Arc::new(Mutex::new(FunctionState::default())),
        callbacks: &cb,
    };
    native::eval(ast, &ctx).map_err(|e| e.to_string())
}

/// Every corpus equation must be accepted by the parser — except the pinned
/// list below: raw Torque `[BRACKETID]` forms with hex letters, which the
/// QuickJS backend also rejected (recorded 2026-08-11, its last build). A
/// new reject is a parser regression; a disappearing reject means the
/// dialect grew and the list should shrink deliberately.
#[test]
fn corpus_parse_coverage() {
    const PINNED_REJECTS: [&str; 4] = [
        "[222885]*[222414]/[22000D]",
        "[222885]*[222414]/([22000D]/1.61)",
        "[22436B]*[224424]/1000",
        "[22436B]*[22436C]*100/[224376]",
    ];
    let eqs = corpus();
    let mut unexpected_fail = Vec::new();
    let mut unexpected_pass = Vec::new();
    for eq in &eqs {
        let rejected = native::compile(eq).is_err();
        let pinned = PINNED_REJECTS.contains(&eq.as_str());
        if rejected && !pinned {
            unexpected_fail.push(eq.clone());
        }
        if !rejected && pinned {
            unexpected_pass.push(eq.clone());
        }
    }
    assert!(
        unexpected_fail.is_empty(),
        "parser newly rejects corpus equations: {unexpected_fail:?}"
    );
    assert!(
        unexpected_pass.is_empty(),
        "pinned rejects now parse — dialect grew, shrink PINNED_REJECTS deliberately: {unexpected_pass:?}"
    );
    eprintln!(
        "parse coverage: {}/{} parsed natively; {} pinned rejects",
        eqs.len() - PINNED_REJECTS.len(),
        eqs.len(),
        PINNED_REJECTS.len()
    );
}

// ---------------------------------------------------------------------------
// Golden CSV: every reference-PID equation evaluated natively must reproduce
// the recorded expected value. The committed snapshot lives at
// tests/corpus/reference_pid_results.csv; regenerate it by running
// reference_pids::export_results_csv (writes tests/output/) and copying the
// result over — only on an intentional equation-generator change.
// ---------------------------------------------------------------------------

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(std::mem::take(&mut cur));
            }
            other => cur.push(other),
        }
    }
    out.push(cur);
    out
}

#[test]
fn golden_csv_reproduced_natively() {
    let path = format!(
        "{}/tests/corpus/reference_pid_results.csv",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path)
        .expect("committed golden CSV tests/corpus/reference_pid_results.csv must exist");
    let mut checked = 0;
    for line in text.lines().skip(1) {
        let f = split_csv_line(line);
        if f.len() < 16 || f[15].starts_with("SKIPPED") || f[10] == "N/A" {
            continue;
        }
        let bytes: Vec<u8> = f[8]
            .split_whitespace()
            .map(|t| u8::from_str_radix(t.trim_start_matches("0x"), 16).expect("hex byte"))
            .collect();
        let expected: f64 = f[11].parse().expect("expected value");
        let tolerance: f64 = f[14].parse().unwrap_or(0.001);
        // f[9] (ReferenceEquation) is spec display text (uses `^` as
        // exponent notation) — only the GeneratedEquation is evaluable.
        {
            let eq = &f[10];
            let ast = native::compile(eq).unwrap_or_else(|e| panic!("compile {eq:?}: {e}"));
            let got = match run_native(&ast, &bytes, None) {
                Ok(ExpressionResult::Numeric(v)) => v,
                other => panic!("expected numeric for {eq:?}: {other:?}"),
            };
            assert!(
                (got - expected).abs() <= tolerance,
                "golden CSV mismatch for {} ({eq:?}): expected {expected}, native {got}",
                f[1]
            );
            checked += 1;
        }
    }
    eprintln!("golden CSV: {checked} equation evaluations reproduced natively");
    assert!(checked > 100, "golden CSV coverage too small: {checked}");
}

// ---------------------------------------------------------------------------
// Parser robustness: deterministic pseudo-random inputs must never panic —
// Err is always acceptable, aborting is not.
// ---------------------------------------------------------------------------

#[test]
fn parser_never_panics_on_garbage() {
    let charset: Vec<char> =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 \t()[]{}:;,.?!'\"=<>&|^~+-*/%\\#@_$€𝄞\u{0}\n"
            .chars()
            .collect();
    let mut lcg = Lcg::new(0xF00D);
    let mut tested = 0u32;

    // Pure random strings.
    for _ in 0..20000 {
        let len = (lcg.next() % 64) as usize;
        let s: String = (0..len)
            .map(|_| charset[(lcg.next() as usize) % charset.len()])
            .collect();
        let r = std::panic::catch_unwind(|| {
            let _ = native::compile(&s);
        });
        assert!(r.is_ok(), "parser panicked on input {s:?}");
        tested += 1;
    }

    // Corpus mutations: splice random garbage into real equations.
    let eqs = corpus();
    for round in 0..10000 {
        let eq = &eqs[(lcg.next() as usize) % eqs.len()];
        let mut s: Vec<char> = eq.chars().collect();
        for _ in 0..(1 + lcg.next() % 4) {
            let pos = (lcg.next() as usize) % (s.len() + 1);
            match lcg.next() % 3 {
                0 => {
                    s.insert(pos, charset[(lcg.next() as usize) % charset.len()]);
                }
                1 if !s.is_empty() => {
                    s.remove(pos.min(s.len() - 1));
                }
                _ => {
                    s.truncate(pos);
                }
            }
        }
        let s: String = s.into_iter().collect();
        let r = std::panic::catch_unwind(|| {
            let _ = native::compile(&s);
        });
        assert!(
            r.is_ok(),
            "parser panicked on mutated input {s:?} (round {round})"
        );
        tested += 1;
    }

    // Deep nesting must return Err (depth limit), not blow the stack.
    let deep = "(".repeat(5000) + "A" + &")".repeat(5000);
    assert!(native::compile(&deep).is_err());
    eprintln!("parser robustness: {tested} garbage/mutated inputs, no panics");
}
