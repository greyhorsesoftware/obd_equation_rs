// NA5 archived differential: the corpus × input-grid sweep pinned against a
// committed fixture (tests/corpus/expected_results.txt) instead of a live
// second backend.
//
// The fixture was RECORDED FROM THE QUICKJS BACKEND (2026-08-11, the last
// build before its retirement) by running:
//     cargo test --no-default-features --test corpus_snapshot \
//         regenerate_corpus_snapshot -- --ignored
// and is verified against the current (native) backend on every test run —
// the exact comparison contract of the retired native_differential harness:
//   * Numeric — bit-exact (±0 conflated, NaN==NaN), else ≤1 ULP, else FAIL.
//   * Text/Bool — exact equality, same variant.
//   * Err — fixture Err must stay Err (message text is not pinned).
//   * RANDOM() equations — Ok + within [0,1) per vector (marker `RANDOM`).
//
// Everything goes through the PUBLIC ExpressionEvaluator API, so this file
// compiles identically whichever backend is behind it. Regenerate the
// fixture (same --ignored test, default features) ONLY on an intentional
// dialect change — never to silence a mismatch.

use obd_equation_rs::{ExpressionEvaluator, ExpressionResult};
use std::collections::HashMap;
use std::sync::Mutex;

const BYTES_LEN: usize = 42; // covers A..Z and A1..P1 (multi-frame PIDs)
const RANDOM_VECTORS: usize = 200;
const THREADS: usize = 8;

const FIXTURE: &str = "tests/corpus/expected_results.txt";

// --- deterministic helpers (verbatim from the retired differential) --------

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
    fn byte(&mut self) -> u8 {
        (self.next() >> 33) as u8
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn input_grid() -> Vec<Vec<u8>> {
    let mut grid: Vec<Vec<u8>> = vec![
        vec![0x00; BYTES_LEN],
        vec![0xFF; BYTES_LEN],
        vec![0x7F; BYTES_LEN],
        vec![0x80; BYTES_LEN],
        (0..BYTES_LEN).map(|i| (i * 7 + 3) as u8).collect(),
        (0..BYTES_LEN)
            .map(|i| if i % 2 == 0 { 0x7F } else { 0x80 })
            .collect(),
    ];
    let mut lcg = Lcg::new(0xD1FF);
    for _ in 0..RANDOM_VECTORS {
        grid.push((0..BYTES_LEN).map(|_| lcg.byte()).collect());
    }
    grid
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

fn extract_val_names(eq: &str) -> Vec<String> {
    let b = eq.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < b.len() {
        if (b[i] | 0x20) == b'v'
            && (b[i + 1] | 0x20) == b'a'
            && (b[i + 2] | 0x20) == b'l'
            && b[i + 3] == b'{'
        {
            if let Some(close) = b[i + 4..].iter().position(|c| *c == b'}') {
                if close > 0 {
                    out.push(eq[i + 4..i + 4 + close].to_string());
                    i += 4 + close + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn pid_value_for(name: &str) -> f64 {
    1.0 + (fnv1a(name) % 40000) as f64 / 100.0
}

fn substitute_brackets(eq: &str) -> (String, bool) {
    let b = eq.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut any = false;
    while i < b.len() {
        if b[i] == b'[' {
            if let Some(close) = b[i + 1..].iter().position(|c| *c == b']') {
                let id = &eq[i + 1..i + 1 + close];
                if !id.is_empty() && id.bytes().all(|c| c.is_ascii_alphanumeric()) {
                    out.push_str(&format!("{:.2}", pid_value_for(id)));
                    i += close + 2;
                    any = true;
                    continue;
                }
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    (out, any)
}

fn scan_idents(eq: &str) -> Vec<String> {
    let b = eq.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(eq[start..i].to_uppercase());
        } else {
            i += 1;
        }
    }
    out
}

fn uses_random(eq: &str) -> bool {
    scan_idents(eq).iter().any(|s| s == "RANDOM")
}

// --- variants (same shape the differential ran) ----------------------------

struct Variant {
    eq_index: usize,
    variant: usize, // 0 = bracket-substituted text, 1 = raw text
    text: String,
    pid_map: Option<HashMap<String, f64>>,
    random: bool,
}

fn variants() -> Vec<Variant> {
    let eqs = corpus();
    assert!(eqs.len() > 300, "corpus unexpectedly small: {}", eqs.len());
    let mut out = Vec::new();
    for (eq_index, eq) in eqs.iter().enumerate() {
        let (subst, had_brackets) = substitute_brackets(eq);
        let val_names = extract_val_names(&subst);
        let pid_map: Option<HashMap<String, f64>> = if val_names.is_empty() {
            None
        } else {
            Some(
                val_names
                    .iter()
                    .map(|n| (n.clone(), pid_value_for(n)))
                    .collect(),
            )
        };
        let random = uses_random(&subst);
        out.push(Variant {
            eq_index,
            variant: 0,
            text: subst,
            pid_map: pid_map.clone(),
            random,
        });
        if had_brackets {
            let random = uses_random(eq);
            out.push(Variant {
                eq_index,
                variant: 1,
                text: eq.clone(),
                pid_map,
                random,
            });
        }
    }
    out
}

// Fresh evaluator per case: stateful functions get fresh state so first-call
// results are deterministic (sequence behavior is pinned separately below).
fn run_case(
    text: &str,
    bytes: &[u8],
    pids: Option<&HashMap<String, f64>>,
) -> Result<ExpressionResult, String> {
    let vars = HashMap::new();
    let mut ev = ExpressionEvaluator::new().map_err(|e| e.to_string())?;
    let r = match pids {
        Some(p) => ev.evaluate_with_bytes_and_pids_unified(text, &vars, bytes, p),
        None => ev.evaluate_with_bytes_unified(text, &vars, bytes),
    };
    r.map_err(|e| e.to_string())
}

// --- fixture encoding -------------------------------------------------------

fn escape_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            ' ' => out.push_str("%20"),
            '\n' => out.push_str("%0A"),
            '\t' => out.push_str("%09"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape_text(s: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '%' && i + 2 < b.len() {
            let hex: String = b[i + 1..i + 3].iter().collect();
            let decoded = match hex.as_str() {
                "25" => Some('%'),
                "20" => Some(' '),
                "0A" => Some('\n'),
                "09" => Some('\t'),
                _ => None,
            };
            if let Some(c) = decoded {
                out.push(c);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn encode(r: &Result<ExpressionResult, String>) -> String {
    match r {
        Err(_) => "E".to_string(),
        Ok(ExpressionResult::Numeric(v)) => format!("N{:016x}", v.to_bits()),
        Ok(ExpressionResult::Text(t)) => format!("T{}", escape_text(t)),
        Ok(ExpressionResult::Bool(b)) => format!("B{}", if *b { 1 } else { 0 }),
    }
}

enum Expected {
    Err,
    Numeric(f64),
    Text(String),
    Bool(bool),
}

fn decode(tok: &str) -> Expected {
    match tok.as_bytes().first() {
        Some(b'E') => Expected::Err,
        Some(b'N') => Expected::Numeric(f64::from_bits(
            u64::from_str_radix(&tok[1..], 16).expect("numeric token"),
        )),
        Some(b'T') => Expected::Text(unescape_text(&tok[1..])),
        Some(b'B') => Expected::Bool(&tok[1..] == "1"),
        _ => panic!("bad fixture token: {tok:?}"),
    }
}

fn ulp_distance(a: f64, b: f64) -> Option<u64> {
    if a.is_sign_negative() != b.is_sign_negative() {
        return None;
    }
    let (x, y) = (a.to_bits() & !(1 << 63), b.to_bits() & !(1 << 63));
    Some(x.abs_diff(y))
}

fn numeric_matches(expected: f64, got: f64) -> bool {
    if expected == got || (expected.is_nan() && got.is_nan()) {
        return true;
    }
    matches!(ulp_distance(expected, got), Some(d) if d <= 1)
}

// --- stateful sequences (deterministic subset of the retired sequence test) —
// TAVG/TOT(1:A+1) are wall-clock-coupled and cannot be snapshotted; their
// invariants are checked natively in stateful_sequences_hold below.

const SEQUENCES: &[(&str, usize, u64)] = &[
    ("EWMAF(0.3:A)", 1500, 1),
    ("EWMAF(0.25, A*256+B)", 1200, 2),
    ("RAVG(A)", 1200, 3),
    ("AVG(7:A)", 1200, 4),
    ("RDLY(5:A)", 1200, 5),
    ("TDLY(3600:A)", 1000, 6),
    ("TDLY(0:A)", 1000, 7),
    ("TOT(1:A-A)", 1000, 8),
];

/// Persistent-evaluator sequence run → (FNV hash over all step bit-patterns,
/// final value bits).
fn run_sequence(eq: &str, steps: usize, seed: u64) -> (u64, u64) {
    let vars = HashMap::new();
    let mut ev = ExpressionEvaluator::new().unwrap();
    let mut lcg = Lcg::new(seed);
    let mut hash = 0xcbf29ce484222325u64;
    let mut last = 0u64;
    for step in 0..steps {
        let mut bytes = [0u8; 4];
        for b in bytes.iter_mut() {
            *b = lcg.byte();
        }
        let v = ev
            .evaluate_with_bytes_unified(eq, &vars, &bytes)
            .unwrap_or_else(|e| panic!("step {step} {eq:?}: {e}"))
            .as_f64()
            .expect("numeric");
        last = v.to_bits();
        for byte in last.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    (hash, last)
}

// --- the tests ---------------------------------------------------------------

/// Regenerates the committed fixture from the CURRENT backend. Run only on an
/// intentional dialect change (or the one-time QuickJS recording, see header).
#[test]
#[ignore]
fn regenerate_corpus_snapshot() {
    let grid = input_grid();
    let vs = variants();
    let mut out = String::new();
    out.push_str(
        "# expected_results.txt — corpus × input-grid oracle. See corpus_snapshot.rs header.\n",
    );
    out.push_str(&format!(
        "# equations under test: grid={} vectors\n",
        grid.len()
    ));
    for v in &vs {
        if v.random {
            out.push_str(&format!("EQ {} {} RANDOM\n", v.eq_index, v.variant));
            continue;
        }
        let mut line = format!("EQ {} {} OK", v.eq_index, v.variant);
        for bytes in &grid {
            line.push(' ');
            line.push_str(&encode(&run_case(&v.text, bytes, v.pid_map.as_ref())));
        }
        line.push('\n');
        out.push_str(&line);
    }
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE);
    std::fs::write(&path, out).expect("write fixture");
    eprintln!("wrote {} ({} variants)", path, vs.len());
}

#[test]
fn corpus_matches_snapshot() {
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), FIXTURE);
    let text = std::fs::read_to_string(&path)
        .expect("committed fixture tests/corpus/expected_results.txt must exist");
    let grid = input_grid();
    let vs = variants();

    // Parse fixture: (eq_index, variant) → RANDOM | tokens.
    let mut fixture: HashMap<(usize, usize), Option<Vec<String>>> = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split(' ');
        assert_eq!(parts.next(), Some("EQ"), "bad fixture line: {line:?}");
        let eq_index: usize = parts.next().unwrap().parse().unwrap();
        let variant: usize = parts.next().unwrap().parse().unwrap();
        match parts.next() {
            Some("RANDOM") => {
                fixture.insert((eq_index, variant), None);
            }
            Some("OK") => {
                let toks: Vec<String> = parts.map(String::from).collect();
                assert_eq!(toks.len(), grid.len(), "token count mismatch: {line:?}");
                fixture.insert((eq_index, variant), Some(toks));
            }
            other => panic!("bad fixture kind {other:?} in line {line:?}"),
        }
    }
    assert_eq!(
        fixture.len(),
        vs.len(),
        "fixture/corpus variant count mismatch — regenerate deliberately"
    );

    let divergences: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cases = std::sync::atomic::AtomicU64::new(0);
    let next = std::sync::atomic::AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= vs.len() {
                    break;
                }
                let v = &vs[i];
                let expected = fixture.get(&(v.eq_index, v.variant)).unwrap_or_else(|| {
                    panic!("fixture missing eq {} variant {}", v.eq_index, v.variant)
                });
                match expected {
                    None => {
                        // RANDOM parity: Ok + within [0,1) on every vector.
                        for bytes in &grid {
                            cases.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            match run_case(&v.text, bytes, v.pid_map.as_ref()) {
                                Ok(ExpressionResult::Numeric(x)) if (0.0..1.0).contains(&x) => {}
                                other => divergences
                                    .lock()
                                    .unwrap()
                                    .push(format!("{:?}: RANDOM parity broken: {other:?}", v.text)),
                            }
                        }
                    }
                    Some(toks) => {
                        for (vi, bytes) in grid.iter().enumerate() {
                            cases.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let got = run_case(&v.text, bytes, v.pid_map.as_ref());
                            let ok = match (decode(&toks[vi]), &got) {
                                (Expected::Err, Err(_)) => true,
                                (Expected::Numeric(e), Ok(ExpressionResult::Numeric(g))) => {
                                    numeric_matches(e, *g)
                                }
                                (Expected::Text(e), Ok(ExpressionResult::Text(g))) => e == *g,
                                (Expected::Bool(e), Ok(ExpressionResult::Bool(g))) => e == *g,
                                _ => false,
                            };
                            if !ok {
                                divergences.lock().unwrap().push(format!(
                                    "{:?} input#{vi}: fixture {} vs current {got:?}",
                                    v.text, toks[vi]
                                ));
                            }
                        }
                    }
                }
            });
        }
    });

    let divergences = divergences.into_inner().unwrap();
    eprintln!(
        "corpus snapshot: {} variants, {} cases, {} divergences",
        vs.len(),
        cases.load(std::sync::atomic::Ordering::Relaxed),
        divergences.len()
    );
    for d in divergences.iter().take(50) {
        eprintln!("DIVERGENCE: {d}");
    }
    assert!(
        divergences.is_empty(),
        "{} divergences vs recorded oracle",
        divergences.len()
    );
}

/// Deterministic stateful sequences pinned by hash; regenerated alongside the
/// corpus fixture (the recorded values live in the same fixture file suffix).
#[test]
#[ignore]
fn regenerate_sequence_snapshot() {
    let mut out = String::new();
    for (eq, steps, seed) in SEQUENCES {
        let (hash, last) = run_sequence(eq, *steps, *seed);
        out.push_str(&format!(
            "SEQ {} {} {} {:016x} {:016x}\n",
            escape_text(eq),
            steps,
            seed,
            hash,
            last
        ));
    }
    let path = format!(
        "{}/tests/corpus/expected_sequences.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&path, out).expect("write sequence fixture");
    eprintln!("wrote sequence fixture");
}

#[test]
fn stateful_sequences_hold() {
    let path = format!(
        "{}/tests/corpus/expected_sequences.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path)
        .expect("committed fixture tests/corpus/expected_sequences.txt must exist");
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(' ').collect();
        assert_eq!(f[0], "SEQ");
        let eq = unescape_text(f[1]);
        let steps: usize = f[2].parse().unwrap();
        let seed: u64 = f[3].parse().unwrap();
        let want_hash = u64::from_str_radix(f[4], 16).unwrap();
        let want_last = u64::from_str_radix(f[5], 16).unwrap();
        let (hash, last) = run_sequence(&eq, steps, seed);
        assert_eq!(
            (hash, last),
            (want_hash, want_last),
            "stateful sequence diverged from recorded oracle: {eq:?}"
        );
    }

    // Wall-clock-coupled functions can't be snapshotted — pin invariants.
    // TAVG of a byte stays within the byte range.
    {
        let vars = HashMap::new();
        let mut ev = ExpressionEvaluator::new().unwrap();
        let mut lcg = Lcg::new(9);
        for step in 0..1000 {
            let mut bytes = [0u8; 4];
            for b in bytes.iter_mut() {
                *b = lcg.byte();
            }
            let v = ev
                .evaluate_with_bytes_unified("TAVG(1000000000:A)", &vars, &bytes)
                .unwrap_or_else(|e| panic!("TAVG step {step}: {e}"))
                .as_f64()
                .expect("numeric");
            assert!(
                (0.0..=255.0).contains(&v),
                "TAVG out of input range at step {step}: {v}"
            );
        }
    }
    // TOT of a positive quantity is monotonically non-decreasing.
    {
        let vars = HashMap::new();
        let mut ev = ExpressionEvaluator::new().unwrap();
        let mut lcg = Lcg::new(10);
        let mut prev = f64::NEG_INFINITY;
        for step in 0..1000 {
            let mut bytes = [0u8; 4];
            for b in bytes.iter_mut() {
                *b = lcg.byte();
            }
            let v = ev
                .evaluate_with_bytes_unified("TOT(1:A+1)", &vars, &bytes)
                .unwrap_or_else(|e| panic!("TOT step {step}: {e}"))
                .as_f64()
                .expect("numeric");
            assert!(
                v >= prev,
                "TOT must be monotonic (step {step}): {v} < {prev}"
            );
            prev = v;
        }
    }
}
