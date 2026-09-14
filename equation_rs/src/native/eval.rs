//! Tree-walk evaluator with JavaScript semantics (NA2).
//!
//! The QuickJS backend gives equations full JS semantics; this walker
//! reproduces them: f64 everywhere, ToInt32/ToUint32 coercion for bitwise
//! operators, JS truthiness, loose/strict equality, string concatenation
//! for `+` when either operand is a string, NaN propagation, division by
//! zero → ±Infinity, and the `Number::toString` / `ToNumber(String)` /
//! `parseFloat` / `parseInt` abstract operations used by the function
//! library.

use super::ast::{BinOp, Expr, UnOp};
use super::{funcs, EvalCtx};
use crate::error::{EvaluatorError, Result};
use crate::runtime::ExpressionResult;

/// Runtime value — mirrors the JS primitives an equation can produce.
#[derive(Debug, Clone)]
pub enum V {
    Num(f64),
    Str(String),
    Bool(bool),
    Undef,
}

/// Depth guard for ZEROREF-style nested evaluation.
pub const MAX_EVAL_DEPTH: u32 = 64;

pub fn js_err<T>(msg: impl Into<String>) -> Result<T> {
    Err(EvaluatorError::JavaScriptError(msg.into()))
}

// ---------------------------------------------------------------------------
// Abstract operations
// ---------------------------------------------------------------------------

fn is_js_ws(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

/// ECMA ToNumber(String).
pub fn js_string_to_number(s: &str) -> f64 {
    let t = s.trim_matches(is_js_ws);
    if t.is_empty() {
        return 0.0;
    }
    let b = t.as_bytes();
    if b.len() > 2 && b[0] == b'0' {
        let radix = match b[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            let mut v = 0.0f64;
            for c in t[2..].chars() {
                match c.to_digit(radix) {
                    Some(d) => v = v * radix as f64 + d as f64,
                    None => return f64::NAN,
                }
            }
            return v;
        }
    }
    let (sign, num) = match b[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if num == "Infinity" {
        return sign * f64::INFINITY;
    }
    if valid_str_decimal(num) {
        sign * num.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// StrDecimalLiteral without sign: digits [.digits?] [exp] | .digits [exp]
fn valid_str_decimal(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let mut int_digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        int_digits += 1;
    }
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            frac_digits += 1;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let mut exp_digits = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            exp_digits += 1;
        }
        if exp_digits == 0 {
            return false;
        }
    }
    i == b.len()
}

/// JS `parseFloat`: longest valid decimal-literal prefix, else NaN.
pub fn js_parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_ws);
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if t[i..].starts_with("Infinity") {
        return if i > 0 && b[0] == b'-' {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let digits_start = i;
    let mut int_digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        int_digits += 1;
    }
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        let dot = i;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            frac_digits += 1;
        }
        if int_digits == 0 && frac_digits == 0 {
            // "." alone — not a number
            i = dot;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return f64::NAN;
    }
    // Optional exponent — only included if it has at least one digit.
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let mut exp_digits = 0;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            exp_digits += 1;
        }
        if exp_digits > 0 {
            i = j;
        }
    }
    let _ = digits_start;
    t[..i].parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `parseInt(s)` (no explicit radix: 16 after `0x`/`0X`, else 10).
pub fn js_parse_int(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_ws);
    let b = t.as_bytes();
    let mut i = 0;
    let mut sign = 1.0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        if b[i] == b'-' {
            sign = -1.0;
        }
        i += 1;
    }
    let mut radix = 10u32;
    if i + 1 < b.len() && b[i] == b'0' && (b[i + 1] == b'x' || b[i + 1] == b'X') {
        radix = 16;
        i += 2;
    }
    let mut v = 0.0f64;
    let mut digits = 0;
    while i < b.len() {
        match (b[i] as char).to_digit(radix) {
            Some(d) => {
                v = v * radix as f64 + d as f64;
                digits += 1;
                i += 1;
            }
            None => break,
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    sign * v
}

/// ECMA `Number::toString` (radix 10) — JS's shortest-round-trip decimal
/// with the 21/-6 exponent-form thresholds.
pub fn js_num_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n == 0.0 {
        return "0".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    let neg = n < 0.0;
    let a = n.abs();
    let sci = format!("{a:e}"); // shortest digits, e.g. "1.234e2"
    let (mant, exp) = sci.split_once('e').expect("{:e} format");
    let exp: i32 = exp.parse().expect("exponent");
    let digits_owned: String = mant.chars().filter(|c| *c != '.').collect();
    let mut digits: &str = digits_owned.trim_end_matches('0');
    if digits.is_empty() {
        digits = "0";
    }
    let k = digits.len() as i32;
    let point = exp + 1; // ECMA `n`: value = 0.digits × 10^point

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if k <= point && point <= 21 {
        out.push_str(digits);
        for _ in 0..(point - k) {
            out.push('0');
        }
    } else if 0 < point && point <= 21 {
        out.push_str(&digits[..point as usize]);
        out.push('.');
        out.push_str(&digits[point as usize..]);
    } else if -6 < point && point <= 0 {
        out.push_str("0.");
        for _ in 0..(-point) {
            out.push('0');
        }
        out.push_str(digits);
    } else {
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        let e = point - 1;
        if e >= 0 {
            out.push('+');
        } else {
            out.push('-');
        }
        out.push_str(&e.abs().to_string());
    }
    out
}

/// Abstract ToString (never throws).
pub fn js_to_string(v: &V) -> String {
    match v {
        V::Num(n) => js_num_to_string(*n),
        V::Str(s) => s.clone(),
        V::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        V::Undef => "undefined".into(),
    }
}

/// `x.toString()` method call — throws TypeError on undefined.
pub fn method_to_string(v: &V) -> Result<String> {
    match v {
        V::Undef => Err(EvaluatorError::TypeError(
            "cannot read toString of undefined".to_string(),
        )),
        other => Ok(js_to_string(other)),
    }
}

pub fn to_number(v: &V) -> f64 {
    match v {
        V::Num(n) => *n,
        V::Str(s) => js_string_to_number(s),
        V::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        V::Undef => f64::NAN,
    }
}

pub fn truthy(v: &V) -> bool {
    match v {
        V::Num(n) => *n != 0.0 && !n.is_nan(),
        V::Str(s) => !s.is_empty(),
        V::Bool(b) => *b,
        V::Undef => false,
    }
}

pub fn to_int32(n: f64) -> i32 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    let m = n.trunc() % 4294967296.0;
    let m = if m < 0.0 { m + 4294967296.0 } else { m };
    // m ∈ [0, 2^32)
    m as u32 as i32
}

pub fn to_uint32(n: f64) -> u32 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    let m = n.trunc() % 4294967296.0;
    let m = if m < 0.0 { m + 4294967296.0 } else { m };
    m as u32
}

pub fn to_uint16(n: f64) -> u16 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    let m = n.trunc() % 65536.0;
    let m = if m < 0.0 { m + 65536.0 } else { m };
    m as u16
}

/// JS `Math.pow` — differs from Rust `powf` in the NaN-exponent and
/// |base|==1 with infinite exponent cases.
pub fn js_pow(base: f64, exp: f64) -> f64 {
    if exp.is_nan() {
        return f64::NAN;
    }
    if exp.is_infinite() && base.abs() == 1.0 {
        return f64::NAN;
    }
    // libm::pow for cross-platform bit-identical results (see funcs.rs).
    libm::pow(base, exp)
}

/// JS `Math.round`: nearest integer, ties toward +Infinity, preserving -0.
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    if r == 0.0 && x < 0.0 {
        -0.0
    } else {
        r
    }
}

pub fn strict_eq(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Num(x), V::Num(y)) => x == y, // IEEE: NaN≠NaN, -0==0
        (V::Str(x), V::Str(y)) => x == y,
        (V::Bool(x), V::Bool(y)) => x == y,
        (V::Undef, V::Undef) => true,
        _ => false,
    }
}

pub fn loose_eq(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Num(x), V::Num(y)) => x == y,
        (V::Str(x), V::Str(y)) => x == y,
        (V::Bool(_), _) => loose_eq(&V::Num(to_number(a)), b),
        (_, V::Bool(_)) => loose_eq(a, &V::Num(to_number(b))),
        (V::Num(x), V::Str(s)) => *x == js_string_to_number(s),
        (V::Str(s), V::Num(y)) => js_string_to_number(s) == *y,
        (V::Undef, V::Undef) => true,
        _ => false, // undefined only loosely equals null/undefined
    }
}

// ---------------------------------------------------------------------------
// val{…} textual substitution inside string content (the pipeline's PID
// regex pass is global and quote-blind, so string literals get substituted
// too; replacement text is Rust `f64::to_string()`, missing PIDs become "0").
// ---------------------------------------------------------------------------

pub fn resolve_pid(name: &str, ctx: &EvalCtx) -> f64 {
    if let Some(map) = ctx.pid_values {
        map.get(name).copied().unwrap_or(0.0)
    } else {
        ctx.callbacks.get_pid_value(name).unwrap_or(0.0)
    }
}

pub fn substitute_val_refs(s: &str, ctx: &EvalCtx) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let is_val = i + 4 < b.len()
            && (b[i] == b'v' || b[i] == b'V')
            && (b[i + 1] == b'a' || b[i + 1] == b'A')
            && (b[i + 2] == b'l' || b[i + 2] == b'L')
            && b[i + 3] == b'{';
        if is_val {
            if let Some(close) = b[i + 4..].iter().position(|c| *c == b'}') {
                if close > 0 {
                    let name = &s[i + 4..i + 4 + close];
                    out.push_str(&resolve_pid(name, ctx).to_string());
                    i += 4 + close + 1;
                    continue;
                }
            }
        }
        // Copy one UTF-8 unit.
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

// ---------------------------------------------------------------------------
// Walker
// ---------------------------------------------------------------------------

pub fn eval_expr(e: &Expr, ctx: &EvalCtx, depth: u32) -> Result<V> {
    if depth > MAX_EVAL_DEPTH {
        return js_err("evaluation too deeply nested");
    }
    match e {
        Expr::Num(n) => Ok(V::Num(*n)),
        Expr::Str { s, has_val_ref } => Ok(V::Str(if *has_val_ref {
            substitute_val_refs(s, ctx)
        } else {
            s.clone()
        })),
        Expr::Bool(b) => Ok(V::Bool(*b)),
        Expr::Undefined => Ok(V::Undef),
        Expr::Ident { name, span } => resolve_ident(name, span.start, ctx),
        Expr::ValRef { name, .. } => Ok(V::Num(resolve_pid(name, ctx))),
        Expr::Bracket(items) => {
            // JS array literal → ToPrimitive → Array.prototype.join(","):
            // undefined elements become empty strings.
            let mut parts = Vec::with_capacity(items.len());
            for it in items {
                let v = eval_expr(it, ctx, depth + 1)?;
                parts.push(match v {
                    V::Undef => String::new(),
                    other => js_to_string(&other),
                });
            }
            Ok(V::Str(parts.join(",")))
        }
        Expr::Unary(op, inner) => {
            let v = eval_expr(inner, ctx, depth + 1)?;
            Ok(match op {
                UnOp::Neg => V::Num(-to_number(&v)),
                UnOp::Plus => V::Num(to_number(&v)),
                UnOp::Not => V::Bool(!truthy(&v)),
                UnOp::BitNot => V::Num(!to_int32(to_number(&v)) as f64),
            })
        }
        Expr::Binary(op, l, r) => eval_binary(*op, l, r, ctx, depth),
        Expr::Ternary(c, a, b) => {
            let cv = eval_expr(c, ctx, depth + 1)?;
            if truthy(&cv) {
                eval_expr(a, ctx, depth + 1)
            } else {
                eval_expr(b, ctx, depth + 1)
            }
        }
        Expr::Call { name, args, span } => {
            // JS call semantics: all arguments evaluated eagerly (including
            // for IF(...) — both branches always evaluate).
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(eval_expr(a, ctx, depth + 1)?);
            }
            funcs::call(name, &vals, ctx, depth, span.start)
        }
        Expr::Seq(items) => {
            let mut last = V::Undef;
            for it in items {
                last = eval_expr(it, ctx, depth + 1)?;
            }
            Ok(last)
        }
    }
}

fn resolve_ident(name: &str, pos: usize, ctx: &EvalCtx) -> Result<V> {
    // Non-writable globals win over set_variable attempts.
    match name {
        "NaN" => return Ok(V::Num(f64::NAN)),
        "Infinity" => return Ok(V::Num(f64::INFINITY)),
        _ => {}
    }
    // Byte variables (A–Z / a–z, then A1…Z1, A2… — see variables.rs); the
    // resolver inserts them after user variables, so bytes win.
    if let Some(bytes) = ctx.bytes {
        if let Some(idx) = byte_var_index(name) {
            if idx < bytes.len() {
                return Ok(V::Num(bytes[idx] as f64));
            }
        }
    }
    if let Some(v) = ctx.variables.get(name) {
        return Ok(V::Num(*v));
    }
    if funcs::is_known_function(name) {
        // Bare function reference: QuickJS yields a function object, which
        // the unified-result conversion rejects with a TypeError.
        return Err(EvaluatorError::TypeError(format!(
            "bare function reference '{name}' at byte {pos}"
        )));
    }
    js_err(format!("'{name}' is not defined (byte {pos})"))
}

/// Inverse of `variables::byte_var_name`: `A`→0 … `Z`→25, `A1`→26, `P1`→41 …
/// Lowercase accepted (the resolver registers both cases). Group suffix must
/// be ≥ 1 (the resolver never emits `A0`).
fn byte_var_index(name: &str) -> Option<usize> {
    let b = name.as_bytes();
    let first = *b.first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let letter = (first.to_ascii_uppercase() - b'A') as usize;
    if b.len() == 1 {
        return Some(letter);
    }
    let rest = &name[1..];
    if !rest.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let group: usize = rest.parse().ok()?;
    if group == 0 {
        return None;
    }
    Some(group * 26 + letter)
}

fn eval_binary(op: BinOp, l: &Expr, r: &Expr, ctx: &EvalCtx, depth: u32) -> Result<V> {
    // Lazy operators first.
    match op {
        BinOp::And => {
            let lv = eval_expr(l, ctx, depth + 1)?;
            return if !truthy(&lv) {
                Ok(lv)
            } else {
                eval_expr(r, ctx, depth + 1)
            };
        }
        BinOp::Or => {
            let lv = eval_expr(l, ctx, depth + 1)?;
            return if truthy(&lv) {
                Ok(lv)
            } else {
                eval_expr(r, ctx, depth + 1)
            };
        }
        _ => {}
    }
    let a = eval_expr(l, ctx, depth + 1)?;
    let b = eval_expr(r, ctx, depth + 1)?;
    Ok(match op {
        BinOp::Add => {
            if matches!(a, V::Str(_)) || matches!(b, V::Str(_)) {
                V::Str(format!("{}{}", js_to_string(&a), js_to_string(&b)))
            } else {
                V::Num(to_number(&a) + to_number(&b))
            }
        }
        BinOp::Sub => V::Num(to_number(&a) - to_number(&b)),
        BinOp::Mul => V::Num(to_number(&a) * to_number(&b)),
        BinOp::Div => V::Num(to_number(&a) / to_number(&b)),
        BinOp::Mod => V::Num(to_number(&a) % to_number(&b)),
        BinOp::Pow => V::Num(js_pow(to_number(&a), to_number(&b))),
        BinOp::BitAnd => V::Num((to_int32(to_number(&a)) & to_int32(to_number(&b))) as f64),
        BinOp::BitOr => V::Num((to_int32(to_number(&a)) | to_int32(to_number(&b))) as f64),
        BinOp::BitXor => V::Num((to_int32(to_number(&a)) ^ to_int32(to_number(&b))) as f64),
        BinOp::Shl => {
            V::Num(to_int32(to_number(&a)).wrapping_shl(to_uint32(to_number(&b)) & 31) as f64)
        }
        BinOp::Shr => V::Num((to_int32(to_number(&a)) >> (to_uint32(to_number(&b)) & 31)) as f64),
        BinOp::UShr => V::Num((to_uint32(to_number(&a)) >> (to_uint32(to_number(&b)) & 31)) as f64),
        BinOp::Lt => js_relational(&a, &b, |o| o == std::cmp::Ordering::Less),
        BinOp::Gt => js_relational(&a, &b, |o| o == std::cmp::Ordering::Greater),
        BinOp::Le => js_relational(&a, &b, |o| o != std::cmp::Ordering::Greater),
        BinOp::Ge => js_relational(&a, &b, |o| o != std::cmp::Ordering::Less),
        BinOp::EqLoose => V::Bool(loose_eq(&a, &b)),
        BinOp::NeLoose => V::Bool(!loose_eq(&a, &b)),
        BinOp::EqStrict => V::Bool(strict_eq(&a, &b)),
        BinOp::NeStrict => V::Bool(!strict_eq(&a, &b)),
        BinOp::And | BinOp::Or => unreachable!("handled above"),
    })
}

fn js_relational(a: &V, b: &V, f: impl Fn(std::cmp::Ordering) -> bool) -> V {
    if let (V::Str(x), V::Str(y)) = (a, b) {
        return V::Bool(f(x.as_str().cmp(y.as_str())));
    }
    let (x, y) = (to_number(a), to_number(b));
    match x.partial_cmp(&y) {
        Some(o) => V::Bool(f(o)),
        None => V::Bool(false), // NaN involved
    }
}

/// Map a final value onto the crate's unified result exactly like the
/// retired QuickJS backend's evaluate_expression_unified did (string results that
/// parse as f64 via Rust `str::parse` become Numeric; null/undefined → 0).
pub fn to_expression_result(v: V) -> ExpressionResult {
    match v {
        V::Num(n) => ExpressionResult::Numeric(n),
        V::Bool(b) => ExpressionResult::Bool(b),
        V::Str(s) => match s.parse::<f64>() {
            Ok(n) => ExpressionResult::Numeric(n),
            Err(_) => ExpressionResult::Text(s),
        },
        V::Undef => ExpressionResult::Numeric(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_to_string_matches_js_forms() {
        assert_eq!(js_num_to_string(0.0), "0");
        assert_eq!(js_num_to_string(-0.0), "0");
        assert_eq!(js_num_to_string(5.0), "5");
        assert_eq!(js_num_to_string(-5.5), "-5.5");
        assert_eq!(js_num_to_string(100.0), "100");
        assert_eq!(js_num_to_string(0.5), "0.5");
        assert_eq!(js_num_to_string(0.0000305), "0.0000305");
        assert_eq!(js_num_to_string(1e21), "1e+21");
        assert_eq!(js_num_to_string(1e-7), "1e-7");
        assert_eq!(js_num_to_string(1.5e-7), "1.5e-7");
        assert_eq!(js_num_to_string(123.456), "123.456");
        assert_eq!(js_num_to_string(f64::NAN), "NaN");
        assert_eq!(js_num_to_string(f64::INFINITY), "Infinity");
        assert_eq!(js_num_to_string(1e20), "100000000000000000000");
    }

    #[test]
    fn to_int32_wraps() {
        assert_eq!(to_int32(4294967295.0), -1);
        assert_eq!(to_int32(2147483648.0), -2147483648);
        assert_eq!(to_int32(-1.0), -1);
        assert_eq!(to_int32(f64::NAN), 0);
        assert_eq!(to_int32(1e300), 0); // 10^300 has 2^300 as a factor → ≡ 0 mod 2^32
    }

    #[test]
    fn string_to_number_quirks() {
        assert_eq!(js_string_to_number(""), 0.0);
        assert_eq!(js_string_to_number("  "), 0.0);
        assert_eq!(js_string_to_number("0x10"), 16.0);
        assert!(js_string_to_number("10px").is_nan());
        assert_eq!(js_string_to_number(" 12.5 "), 12.5);
        assert_eq!(js_string_to_number("Infinity"), f64::INFINITY);
        assert!(js_string_to_number("inf").is_nan());
    }

    #[test]
    fn parse_float_prefix() {
        assert_eq!(js_parse_float("12abc"), 12.0);
        assert!(js_parse_float("abc").is_nan());
        assert_eq!(js_parse_float("0x10"), 0.0); // parseFloat stops at 'x'
        assert_eq!(js_parse_float("-2.5e2px"), -250.0);
        assert_eq!(js_parse_float("1e"), 1.0); // exponent without digits excluded
    }

    #[test]
    fn byte_var_indexing() {
        assert_eq!(byte_var_index("A"), Some(0));
        assert_eq!(byte_var_index("z"), Some(25));
        assert_eq!(byte_var_index("A1"), Some(26));
        assert_eq!(byte_var_index("P1"), Some(41));
        assert_eq!(byte_var_index("a2"), Some(52));
        assert_eq!(byte_var_index("A0"), None);
        assert_eq!(byte_var_index("AB"), None);
    }
}
