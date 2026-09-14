//! Native function library (NA2) — semantics transcribed line-by-line from
//! the bundled JS sources (`src/functions/*.js`) and the stateful closures
//! in `evaluator.rs::register_stateful_handlers`. Quirks are intentional and
//! load-bearing:
//!
//! * `BITSELECT`/`IFANY` options are `arguments.slice(2)`, so in the
//!   `(value, start, end, …)` form the END BIT is `options[0]`.
//! * `SIGNED*` return the ORIGINAL argument (possibly a string) when below
//!   the sign threshold (`value = value || 0; if (value >= T) return value-W;
//!   return value;`).
//! * `LOOKUP` values that are quoted return strings; unquoted values run
//!   through `isNaN(val) ? val : parseFloat(val)` — so an empty value yields
//!   NaN and `"0x10"` yields 0 (parseFloat stops at `x`).
//! * All stateful functions share the single `"default"` state keys, exactly
//!   like the QuickJS closures (two TAVG calls in one expression share
//!   state; EWMAF and TAVG share `ewma_values["default"]`).
//! * `IF(cond, a, b)` is a plain function: BOTH branches are always
//!   evaluated (argument evaluation is eager), unlike `? :`.

use super::eval::{
    eval_expr, js_err, js_parse_float, js_parse_int, js_pow, js_round, js_string_to_number,
    js_to_string, method_to_string, strict_eq, to_int32, to_number, to_uint16, to_uint32, truthy,
    V,
};
use super::EvalCtx;
use crate::error::{EvaluatorError, Result};
use std::sync::atomic::{AtomicU64, Ordering};

/// Exact-case-only function names (defined once in the JS runtime, no
/// aliases): everything not in `parse::CANON_FUNCS`.
const EXACT_FUNCS: &[&str] = &[
    "MIN", "MAX", "ABS", "SQRT", "POW", "SIN", "COS", "TAN", "LOG", "LOG10", "EXP", "CEIL",
    "FLOOR", "ROUND", "INT", "IF", "ZEROREF", "ascii",
];

const CANON_UPPER: &[&str] = &[
    "AVG",
    "BARO",
    "BIT",
    "BITSELECT",
    "BITVALUE",
    "CLOSEST",
    "EWMAF",
    "FLOAT32",
    "FLOAT64",
    "IFANY",
    "INT16",
    "INT24",
    "INT32",
    "LOG1P",
    "RANDOM",
    "RAVG",
    "RDLY",
    "SIGNED",
    "SIGNED8",
    "SIGNED16",
    "SIGNED24",
    "SIGNED32",
    "TAVG",
    "TDLY",
    "TOT",
    "LOOKUP",
];

/// Is `name` a global the JS runtime defines as a function? (Used for the
/// bare-identifier case: canonical names exist in UPPERCASE and lowercase
/// forms — the lowercase alias vars — plus the exact-case set.)
pub fn is_known_function(name: &str) -> bool {
    if EXACT_FUNCS.contains(&name) {
        return true;
    }
    CANON_UPPER.contains(&name)
        || (name.chars().all(|c| !c.is_ascii_uppercase())
            && CANON_UPPER.contains(&name.to_ascii_uppercase().as_str()))
}

fn arg(args: &[V], i: usize) -> V {
    args.get(i).cloned().unwrap_or(V::Undef)
}

fn num(args: &[V], i: usize) -> f64 {
    to_number(&arg(args, i))
}

/// `x || 0` — JS falsy coalescing used by INT16/24/32, FLOAT32/64, SIGNED*.
fn or_zero(v: V) -> V {
    if truthy(&v) {
        v
    } else {
        V::Num(0.0)
    }
}

/// Deterministic LCG for RANDOM() — Math.random() is nondeterministic
/// anyway, so backend parity is [0,1) membership, not value identity.
static RNG_STATE: AtomicU64 = AtomicU64::new(0x853c49e6748fea9b);

fn js_random() -> f64 {
    let mut s = RNG_STATE.load(Ordering::Relaxed);
    loop {
        let next = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        match RNG_STATE.compare_exchange_weak(s, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return (next >> 11) as f64 / (1u64 << 53) as f64,
            Err(cur) => s = cur,
        }
    }
}

pub fn call(name: &str, args: &[V], ctx: &EvalCtx, depth: u32, pos: usize) -> Result<V> {
    match name {
        // -- math (math.js) --------------------------------------------------
        "MIN" => Ok(V::Num(js_min_max(args, true))),
        "MAX" => Ok(V::Num(js_min_max(args, false))),
        "ABS" => Ok(V::Num(num(args, 0).abs())),
        "SQRT" => Ok(V::Num(num(args, 0).sqrt())),
        "POW" => Ok(V::Num(js_pow(num(args, 0), num(args, 1)))),
        // Transcendentals via libm for cross-platform bit-identical results.
        "SIN" => Ok(V::Num(libm::sin(num(args, 0)))),
        "COS" => Ok(V::Num(libm::cos(num(args, 0)))),
        "TAN" => Ok(V::Num(libm::tan(num(args, 0)))),
        "LOG" => Ok(V::Num(libm::log(num(args, 0)))),
        "LOG10" => Ok(V::Num(libm::log10(num(args, 0)))),
        "LOG1P" => Ok(V::Num(libm::log1p(num(args, 0)))),
        "EXP" => Ok(V::Num(libm::exp(num(args, 0)))),
        "CEIL" => Ok(V::Num(num(args, 0).ceil())),
        "FLOOR" | "INT" => Ok(V::Num(num(args, 0).floor())),
        "ROUND" => Ok(V::Num(js_round(num(args, 0)))),
        "RANDOM" => Ok(V::Num(js_random())),

        // -- bit ops (bit_ops.js) --------------------------------------------
        "BIT" => Ok(V::Num(bit(num(args, 0), num(args, 1)))),
        "BITVALUE" => Ok(V::Num(bitvalue(num(args, 0), num(args, 1), num(args, 2)))),
        "BITSELECT" => bitselect(args),
        "IFANY" => ifany(args),

        // -- conversions (conversions.js) --------------------------------------
        "INT16" => {
            if args.len() >= 2 {
                let a = to_int32(to_number(&or_zero(arg(args, 0))));
                let b = to_int32(to_number(&or_zero(arg(args, 1))));
                Ok(V::Num(((a.wrapping_shl(8)) | b) as f64))
            } else {
                Ok(V::Num(0.0))
            }
        }
        "INT24" => {
            if args.len() >= 3 {
                let a = to_int32(to_number(&or_zero(arg(args, 0))));
                let b = to_int32(to_number(&or_zero(arg(args, 1))));
                let c = to_int32(to_number(&or_zero(arg(args, 2))));
                Ok(V::Num((a.wrapping_shl(16) | b.wrapping_shl(8) | c) as f64))
            } else {
                Ok(V::Num(0.0))
            }
        }
        "INT32" => {
            if args.len() >= 4 {
                let a = to_int32(to_number(&or_zero(arg(args, 0))));
                let b = to_int32(to_number(&or_zero(arg(args, 1))));
                let c = to_int32(to_number(&or_zero(arg(args, 2))));
                let d = to_int32(to_number(&or_zero(arg(args, 3))));
                Ok(V::Num(
                    (a.wrapping_shl(24) | b.wrapping_shl(16) | c.wrapping_shl(8) | d) as f64,
                ))
            } else {
                Ok(V::Num(0.0))
            }
        }
        "SIGNED" | "SIGNED8" => Ok(signed_n(args, 128.0, 256.0)),
        "SIGNED16" => Ok(signed_n(args, 32768.0, 65536.0)),
        "SIGNED24" => Ok(signed_n(args, 8388608.0, 16777216.0)),
        "SIGNED32" => Ok(signed_n(args, 2147483648.0, 4294967296.0)),
        "FLOAT32" => Ok(V::Num(float32(args))),
        "FLOAT64" => Ok(V::Num(float64(args))),
        "ascii" => Ok(ascii(args)),

        // -- lookup (lookup.js) -------------------------------------------------
        "LOOKUP" => lookup(args),
        "CLOSEST" => closest(args),

        // -- control flow (control_flow.js) ---------------------------------
        "IF" => {
            let cond = arg(args, 0);
            if truthy(&cond) {
                Ok(arg(args, 1))
            } else {
                let else_v = arg(args, 2);
                Ok(if matches!(else_v, V::Undef) {
                    V::Num(0.0)
                } else {
                    else_v
                })
            }
        }
        "ZEROREF" => {
            let cond = arg(args, 0);
            if truthy(&cond) {
                return Ok(arg(args, 1));
            }
            match arg(args, 2) {
                V::Str(s) => {
                    // JS: eval(falseValue) — a RAW eval, no dialect
                    // preprocessing, hence compile_js.
                    let ast = super::parse::compile_js(&s).map_err(|e| {
                        EvaluatorError::JavaScriptError(format!("ZEROREF eval: {e}"))
                    })?;
                    eval_expr(ast.root(), ctx, depth + 1)
                }
                other => Ok(other),
            }
        }

        // -- platform / stateful (stateful.js + evaluator.rs closures) -------
        "BARO" => Ok(V::Num(ctx.callbacks.get_barometric_pressure())),
        "EWMAF" => {
            let weight = stateful_num(args, 0, 0.5)?;
            let value = stateful_num(args, 1, 0.0)?;
            let mut st = lock_state(ctx)?;
            let entry = st.ewma_values.entry("default".to_string()).or_insert(value);
            // NOT .clamp(): the QuickJS closure uses .min(1.0).max(0.0),
            // which maps a NaN weight to 1.0 — clamp would keep NaN.
            #[allow(clippy::manual_clamp)]
            let cw = weight.min(1.0).max(0.0);
            let result = cw * value + (1.0 - cw) * *entry;
            *entry = result;
            Ok(V::Num(result))
        }
        "TAVG" => {
            let tau = stateful_num(args, 0, 1.0)?;
            let value = stateful_num(args, 1, 0.0)?;
            Ok(V::Num(lock_state(ctx)?.tavg(tau, value, "default")))
        }
        "RAVG" => {
            let value = stateful_num(args, 0, 0.0)?;
            let mut st = lock_state(ctx)?;
            let bucket = st
                .avg_buckets
                .entry("ravg_default".to_string())
                .or_default();
            bucket.push(value);
            let sum: f64 = bucket.iter().sum();
            Ok(V::Num(sum / bucket.len() as f64))
        }
        "AVG" => {
            let bucket_size = stateful_usize(args, 0, 10)?;
            let value = stateful_num(args, 1, 0.0)?;
            let mut st = lock_state(ctx)?;
            let bucket = st.avg_buckets.entry("avg_default".to_string()).or_default();
            bucket.push(value);
            if bucket.len() > bucket_size {
                bucket.remove(0);
            }
            let sum: f64 = bucket.iter().sum();
            Ok(V::Num(sum / bucket.len() as f64))
        }
        "TDLY" => {
            let delay = stateful_num(args, 0, 0.0)?;
            let value = stateful_num(args, 1, 0.0)?;
            Ok(V::Num(lock_state(ctx)?.tdly(delay, value, "default")))
        }
        "RDLY" => {
            let samples = stateful_usize(args, 0, 1)?;
            let value = stateful_num(args, 1, 0.0)?;
            Ok(V::Num(lock_state(ctx)?.rdly(samples, value, "default")))
        }
        "TOT" => {
            let window = stateful_num(args, 0, 0.0)?;
            let value = stateful_num(args, 1, 0.0)?;
            Ok(V::Num(lock_state(ctx)?.tot(window, value, "default")))
        }

        other => js_err(format!("'{other}' is not defined (byte {pos})")),
    }
}

fn lock_state<'a>(
    ctx: &'a EvalCtx,
) -> Result<std::sync::MutexGuard<'a, crate::state::FunctionState>> {
    ctx.state
        .lock()
        .map_err(|_| EvaluatorError::StateError("function state lock poisoned".to_string()))
}

/// JS stateful stubs call `nativeX(param.toString(), value.toString())` and
/// the Rust closures parse with `str::parse::<f64>().unwrap_or(default)` —
/// replicated exactly (including the TypeError for `.toString()` on a
/// missing argument).
fn stateful_num(args: &[V], i: usize, default: f64) -> Result<f64> {
    let s = method_to_string(&arg(args, i))?;
    Ok(s.parse::<f64>().unwrap_or(default))
}

fn stateful_usize(args: &[V], i: usize, default: usize) -> Result<usize> {
    let s = method_to_string(&arg(args, i))?;
    Ok(s.parse::<f64>().map(|f| f as usize).unwrap_or(default))
}

/// Math.min / Math.max over ToNumber'd arguments (NaN wins; -0 ordered
/// below +0; empty argument list → ±Infinity).
fn js_min_max(args: &[V], is_min: bool) -> f64 {
    let mut acc = if is_min {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };
    for a in args {
        let x = to_number(a);
        if x.is_nan() {
            return f64::NAN;
        }
        if is_min {
            if x < acc || (x == acc && x.is_sign_negative() && !acc.is_sign_negative()) {
                acc = x;
            }
        } else if x > acc || (x == acc && !x.is_sign_negative() && acc.is_sign_negative()) {
            acc = x;
        }
    }
    acc
}

/// BIT(value, bitIndex) → ((value >> bitIndex) & 0x01) === 1 ? 1 : 0
fn bit(value: f64, bit_index: f64) -> f64 {
    let shifted = to_int32(value) >> (to_uint32(bit_index) & 31);
    if (shifted & 1) == 1 {
        1.0
    } else {
        0.0
    }
}

/// BITVALUE(value, startBit, endBit) with each JS operator's coercion.
fn bitvalue(value: f64, mut start: f64, mut end: f64) -> f64 {
    if end < start {
        std::mem::swap(&mut start, &mut end);
    }
    let num_bits = end - start + 1.0; // float arithmetic, like JS
    let m1 = 1i32.wrapping_shl(to_uint32(num_bits) & 31) as f64; // (1 << numBits)
    let mask = m1 - 1.0; // float subtraction
    let shifted = (to_int32(value) >> (to_uint32(start) & 31)) as f64;
    (to_int32(shifted) & to_int32(mask)) as f64
}

/// Shared start/end extraction for BITSELECT/IFANY: `'a~b'` string ranges go
/// through parseInt; anything else is (arg1, arg2) numerically.
fn bit_range(args: &[V]) -> (f64, f64) {
    let arg1 = arg(args, 1);
    if let V::Str(s) = &arg1 {
        if s.contains('~') {
            let mut parts = s.split('~');
            let lo = js_parse_int(parts.next().unwrap_or(""));
            let hi = match parts.next() {
                Some(p) => js_parse_int(p),
                None => f64::NAN, // parseInt(undefined)
            };
            return (lo, hi);
        }
    }
    (to_number(&arg1), num(args, 2))
}

/// Options array quirk: `Array.prototype.slice.call(arguments, 2)` — in the
/// (value, start, end, …) form options[0] is the END BIT argument.
fn options_slice(args: &[V]) -> &[V] {
    if args.len() > 2 {
        &args[2..]
    } else {
        &[]
    }
}

fn option_at(options: &[V], idx: f64) -> V {
    if idx.fract() == 0.0 && idx >= 0.0 && (idx as usize) < options.len() {
        options[idx as usize].clone()
    } else {
        V::Undef // non-integer array index → undefined property
    }
}

/// Strip surrounding single quotes (with the JS substring(1, 0) swap quirk
/// for a lone "'").
fn strip_quotes(v: V) -> V {
    if let V::Str(s) = &v {
        if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
            return V::Str(s[1..s.len() - 1].to_string());
        }
        if s == "'" {
            return V::Str("'".to_string()); // substring(1,0) swaps to substring(0,1)
        }
    }
    v
}

fn bitselect(args: &[V]) -> Result<V> {
    let value = num(args, 0);
    let (mut start, mut end) = bit_range(args);
    if end < start {
        std::mem::swap(&mut start, &mut end);
    }
    let options = options_slice(args);
    let bit_value = bitvalue(value, start, end);

    let mut bit_index = end - start;
    // Guard identical in effect to JS (loop simply never runs on NaN).
    while bit_index >= 0.0 {
        let mask = 1i32.wrapping_shl(to_uint32(bit_index) & 31);
        if (to_int32(bit_value) & mask) != 0 && bit_index < options.len() as f64 {
            let option = option_at(options, bit_index);
            return Ok(strip_quotes(option));
        }
        bit_index -= 1.0;
    }
    Ok(V::Str(String::new()))
}

fn ifany(args: &[V]) -> Result<V> {
    let value = num(args, 0);
    let (start, end) = bit_range(args);
    let options = options_slice(args);
    let mut results: Vec<String> = Vec::new();

    let mut bit_index = 0.0f64;
    while bit_index <= end - start {
        if bit(value, start + bit_index) == 1.0 && bit_index < options.len() as f64 {
            let option = strip_quotes(option_at(options, bit_index));
            // Array.prototype.join: undefined → empty string.
            results.push(match option {
                V::Undef => String::new(),
                other => js_to_string(&other),
            });
        }
        bit_index += 1.0;
    }
    Ok(V::Str(results.join(", ")))
}

fn signed_n(args: &[V], threshold: f64, wrap: f64) -> V {
    let v0 = or_zero(arg(args, 0));
    let n = to_number(&v0);
    if n >= threshold {
        V::Num(n - wrap)
    } else {
        v0 // original value returned unchanged (string stays a string)
    }
}

fn float32(args: &[V]) -> f64 {
    let a = to_int32(to_number(&or_zero(arg(args, 0)))) & 0xFF;
    let b = to_int32(to_number(&or_zero(arg(args, 1)))) & 0xFF;
    let c = to_int32(to_number(&or_zero(arg(args, 2)))) & 0xFF;
    let d = to_int32(to_number(&or_zero(arg(args, 3)))) & 0xFF;
    let bits = (a as f64) * 16777216.0 + ((b << 16) as f64) + ((c << 8) as f64) + d as f64;
    f32::from_bits(to_uint32(bits)) as f64
}

fn float64(args: &[V]) -> f64 {
    let byte = |i: usize| to_int32(to_number(&or_zero(arg(args, i)))) & 0xFF;
    let hi = (byte(0) as f64) * 16777216.0
        + ((byte(1) << 16) as f64)
        + ((byte(2) << 8) as f64)
        + byte(3) as f64;
    let lo = (byte(4) as f64) * 16777216.0
        + ((byte(5) << 16) as f64)
        + ((byte(6) << 8) as f64)
        + byte(7) as f64;
    f64::from_bits(((to_uint32(hi) as u64) << 32) | to_uint32(lo) as u64)
}

fn ascii(args: &[V]) -> V {
    let mut out = String::new();
    for a in args {
        if matches!(a, V::Undef) {
            continue; // JS skips undefined/null
        }
        let code = to_uint16(to_number(a));
        out.push(char::from_u32(code as u32).unwrap_or('\u{fffd}'));
    }
    V::Str(out)
}

// ---------------------------------------------------------------------------
// LOOKUP / CLOSEST
// ---------------------------------------------------------------------------

struct Pair {
    key: String,
    val: String,
}

/// Argument-to-pair parsing shared by LOOKUP and CLOSEST: `key=value`
/// strings split on the FIRST '=' (extra '=' parts dropped, like JS array
/// destructuring of split), otherwise consecutive (key, value) argument
/// pairs; a trailing odd argument is dropped.
fn build_pairs(args: &[V]) -> Result<Vec<Pair>> {
    let mut pairs = Vec::new();
    let mut i = 2;
    while i < args.len() {
        let s = method_to_string(&args[i])?;
        if s.contains('=') {
            let mut it = s.splitn(3, '=');
            let key = it.next().unwrap_or("").trim().to_string();
            let val = it.next().unwrap_or("").trim().to_string();
            pairs.push(Pair { key, val });
        } else if i + 1 < args.len() {
            let val = method_to_string(&args[i + 1])?;
            pairs.push(Pair { key: s, val });
            i += 1;
        }
        i += 1;
    }
    Ok(pairs)
}

/// `val.startsWith("'") && val.endsWith("'") ? strip : isNaN(val) ? val :
/// parseFloat(val)` — including the ""→NaN and "0x10"→0 quirks.
fn process_val(val: &str) -> V {
    if val.len() >= 2 && val.starts_with('\'') && val.ends_with('\'') {
        return V::Str(val[1..val.len() - 1].to_string());
    }
    if val == "'" {
        return V::Str("'".to_string()); // substring(1, 0) swap
    }
    if js_string_to_number(val).is_nan() {
        V::Str(val.to_string())
    } else {
        V::Num(js_parse_float(val))
    }
}

fn lookup(args: &[V]) -> Result<V> {
    if args.len() < 3 {
        return Ok(V::Num(0.0));
    }
    let value = js_parse_float(&js_to_string(&args[0]));
    let pairs = build_pairs(args)?;

    for Pair { key, val } in &pairs {
        if key.contains('~') {
            let mut parts = key.split('~');
            let lo = js_parse_float(parts.next().unwrap_or(""));
            let hi = match parts.next() {
                Some(p) => js_parse_float(p),
                None => f64::NAN,
            };
            if !lo.is_nan() && !hi.is_nan() && value >= lo && value <= hi {
                return Ok(process_val(val));
            }
            continue;
        }
        // Exact match: parseFloat(key) === value, or string-identical to
        // arguments[0].toString() (which throws on undefined, like JS).
        if js_parse_float(key) == value || *key == method_to_string(&args[0])? {
            return Ok(process_val(val));
        }
    }

    // Default handling.
    let default = &args[1];
    if matches!(default, V::Str(s) if s == "value") || strict_eq(default, &args[0]) {
        return Ok(V::Num(value));
    }
    if matches!(default, V::Str(s) if s.is_empty()) {
        return Ok(V::Num(0.0));
    }
    if to_number(default).is_nan() {
        Ok(default.clone())
    } else {
        Ok(V::Num(js_parse_float(&js_to_string(default))))
    }
}

fn closest(args: &[V]) -> Result<V> {
    if args.len() < 3 {
        return Ok(V::Num(0.0));
    }
    let value = js_parse_float(&js_to_string(&args[0]));
    let pairs = build_pairs(args)?;

    let default = &args[1];
    let mut closest_diff = f64::MAX;
    let mut closest_value = default.clone();

    for Pair { key, val } in &pairs {
        let key_value = js_parse_float(key);
        if key_value.is_nan() {
            continue;
        }
        let diff = (value - key_value).abs();
        if diff < closest_diff {
            closest_diff = diff;
            closest_value = process_val(val);
        }
    }

    if strict_eq(&closest_value, default) {
        if matches!(default, V::Str(s) if s == "value") || strict_eq(default, &args[0]) {
            return Ok(V::Num(value));
        }
        if matches!(default, V::Str(s) if s.is_empty()) {
            return Ok(V::Num(0.0));
        }
        if to_number(default).is_nan() {
            return Ok(default.clone());
        }
        return Ok(V::Num(js_parse_float(&js_to_string(default))));
    }
    Ok(closest_value)
}
