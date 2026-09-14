//! Hand-rolled Pratt parser for the Torque equation dialect (NA1).
//!
//! Parses the dialect DIRECTLY — no string rewriting — but with grammar rules
//! chosen so that the accepted language and its meaning match what the
//! existing textual pipeline (`transform.rs` + `parser.rs` preprocessing +
//! QuickJS) produces for every real equation:
//!
//! * JS operator set and precedence; `? :` ternary (right-associative).
//! * `IF cond THEN a ELSE b` natural syntax, recognized ONLY at the start of
//!   the expression and at the start of an ELSE branch, requiring the exact
//!   single-space delimiters the textual transform requires; desugared to an
//!   `IF(...)` CALL with flattened comma-lists — exactly the transform's
//!   output shape (extra list elements become extra, ignored-but-evaluated
//!   call arguments; a missing ELSE appends `0`).
//! * `:` accepted as an argument separator inside any call's parentheses and
//!   as a sequence separator inside grouping parentheses (where the textual
//!   pass rewrites it to the JS comma operator).
//! * The 26 case-normalized function names (the retired QuickJS-path
//!   parser's CANON_FUNCS list) are canonicalized here the same way.
//! * `LOOKUP(`/`CLOSEST(` (any case, `(` adjacent) argument text is
//!   segmented by the byte-exact port of the retired parser's
//!   `process_lookup_args` below —
//!   `key=value` arms become double-quoted string args, integer `a~b=v`
//!   ranges are expanded to discrete arms, empty/whitespace segments become
//!   `""` — and the reassembled argument text is re-parsed. This is the one
//!   deliberately textual step, because the dialect defines LOOKUP arms
//!   textually.
//! * `val{NAME}` lexes to a `ValRef` resolved at eval time (the pipeline
//!   substitutes it textually before parsing; keeping it in the AST is what
//!   makes compile-once possible).
//! * `[a, b]` bracket groups parse to JS-array-literal semantics (Torque
//!   bracket PID references reach QuickJS as arrays today).
//!
//! Known deliberate deviations (impossible to hit from the NA0 corpus, each
//! verified by the differential harness over the corpus):
//! * degenerate mixes of `? :` with `:`-separated arguments in the same
//!   parenthesis level follow grammar structure, not the transform's
//!   scan-order mangling;
//! * `=` assignment, `;` statement lists, member access `a.b`/`a[i]`, and
//!   object literals are parse errors here (QuickJS accepts raw JS);
//! * `:` inside a string literal inside parentheses is string content here
//!   (the quote-blind textual pass would rewrite it to `,`).

use super::ast::{Ast, BinOp, Expr, UnOp};
use std::fmt;

/// Parse error with byte position into the source handed to `compile`.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for ParseError {}

type PResult<T> = std::result::Result<T, ParseError>;

fn perr<T>(pos: usize, msg: impl Into<String>) -> PResult<T> {
    Err(ParseError {
        pos,
        msg: msg.into(),
    })
}

/// The retired QuickJS-path parser's CANON_FUNCS list: call names normalized to
/// canonical UPPERCASE when followed by `(`; these are exactly the functions
/// that are case-insensitive in the dialect.
const CANON_FUNCS: &[&str] = &[
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

const MAX_DEPTH: u32 = 200;

/// Compile an expression to an AST (full dialect).
pub fn compile(source: &str) -> PResult<Ast> {
    let mut p = Parser::new(source, true, false);
    let root = p.parse_program()?;
    Ok(Ast {
        root,
        source: source.to_string(),
    })
}

/// Compile in raw-JS submode: this is what `ZEROREF`'s string argument gets
/// (the pipeline hands it to a bare `eval()` with NO preprocessing), so
/// colon argument separators, natural IF and LOOKUP segmentation are NOT
/// available and `? :` stays a real (lazy) ternary.
pub fn compile_js(source: &str) -> PResult<Ast> {
    let mut p = Parser::new(source, false, true);
    let root = p.parse_program()?;
    Ok(Ast {
        root,
        source: source.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    ValRef(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Question,
    Op(&'static str),
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

struct Lexer<'s> {
    src: &'s [u8],
    pos: usize,
    peeked: Option<Token>,
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$'
}
fn is_ident_cont(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

impl<'s> Lexer<'s> {
    fn new(src: &'s str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
            peeked: None,
        }
    }

    fn peek(&mut self) -> PResult<&Token> {
        if self.peeked.is_none() {
            let t = self.lex()?;
            self.peeked = Some(t);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    fn next(&mut self) -> PResult<Token> {
        match self.peeked.take() {
            Some(t) => Ok(t),
            None => self.lex(),
        }
    }

    /// Reposition the raw cursor (used after LOOKUP argument extraction).
    fn seek(&mut self, pos: usize) {
        self.pos = pos;
        self.peeked = None;
    }

    fn skip_ws(&mut self) {
        while self.pos < self.src.len() && (self.src[self.pos] as char).is_whitespace() {
            self.pos += 1;
        }
    }

    fn lex(&mut self) -> PResult<Token> {
        self.skip_ws();
        let start = self.pos;
        if self.pos >= self.src.len() {
            return Ok(Token {
                tok: Tok::Eof,
                start,
                end: start,
            });
        }
        let c = self.src[self.pos];

        // Numbers: digits, leading dot, hex/bin/oct.
        if c.is_ascii_digit()
            || (c == b'.'
                && self.pos + 1 < self.src.len()
                && self.src[self.pos + 1].is_ascii_digit())
        {
            return self.lex_number(start);
        }

        // Strings.
        if c == b'\'' || c == b'"' {
            return self.lex_string(start, c);
        }

        // Identifiers / val{…}.
        if is_ident_start(c) {
            let mut i = self.pos + 1;
            while i < self.src.len() && is_ident_cont(self.src[i]) {
                i += 1;
            }
            let name = std::str::from_utf8(&self.src[self.pos..i])
                .map_err(|_| ParseError {
                    pos: start,
                    msg: "invalid utf-8 in identifier".into(),
                })?
                .to_string();
            // `val{NAME}` — regex in the pipeline is (?i)val\{([^}]+)\}: the
            // brace must be adjacent and the name non-empty.
            if name.eq_ignore_ascii_case("val") && i < self.src.len() && self.src[i] == b'{' {
                let name_start = i + 1;
                let mut j = name_start;
                while j < self.src.len() && self.src[j] != b'}' {
                    j += 1;
                }
                if j < self.src.len() && j > name_start {
                    let pid = std::str::from_utf8(&self.src[name_start..j])
                        .map_err(|_| ParseError {
                            pos: name_start,
                            msg: "invalid utf-8 in val{} name".into(),
                        })?
                        .to_string();
                    self.pos = j + 1;
                    return Ok(Token {
                        tok: Tok::ValRef(pid),
                        start,
                        end: self.pos,
                    });
                }
                return perr(start, "malformed val{...} reference");
            }
            self.pos = i;
            return Ok(Token {
                tok: Tok::Ident(name),
                start,
                end: i,
            });
        }

        // Punctuation / operators (longest match first).
        let rest = &self.src[self.pos..];
        let two_or_more: &[(&str, Tok)] = &[
            ("===", Tok::Op("===")),
            ("!==", Tok::Op("!==")),
            (">>>", Tok::Op(">>>")),
            ("==", Tok::Op("==")),
            ("!=", Tok::Op("!=")),
            ("<=", Tok::Op("<=")),
            (">=", Tok::Op(">=")),
            ("<<", Tok::Op("<<")),
            (">>", Tok::Op(">>")),
            ("&&", Tok::Op("&&")),
            ("||", Tok::Op("||")),
            ("**", Tok::Op("**")),
        ];
        for (s, t) in two_or_more {
            if rest.starts_with(s.as_bytes()) {
                self.pos += s.len();
                return Ok(Token {
                    tok: t.clone(),
                    start,
                    end: self.pos,
                });
            }
        }
        let single = match c {
            b'(' => Some(Tok::LParen),
            b')' => Some(Tok::RParen),
            b'[' => Some(Tok::LBracket),
            b']' => Some(Tok::RBracket),
            b',' => Some(Tok::Comma),
            b':' => Some(Tok::Colon),
            b'?' => Some(Tok::Question),
            b'+' => Some(Tok::Op("+")),
            b'-' => Some(Tok::Op("-")),
            b'*' => Some(Tok::Op("*")),
            b'/' => Some(Tok::Op("/")),
            b'%' => Some(Tok::Op("%")),
            b'&' => Some(Tok::Op("&")),
            b'|' => Some(Tok::Op("|")),
            b'^' => Some(Tok::Op("^")),
            b'<' => Some(Tok::Op("<")),
            b'>' => Some(Tok::Op(">")),
            b'!' => Some(Tok::Op("!")),
            b'~' => Some(Tok::Op("~")),
            _ => None,
        };
        if let Some(t) = single {
            self.pos += 1;
            return Ok(Token {
                tok: t,
                start,
                end: self.pos,
            });
        }
        perr(start, format!("unexpected character '{}'", c as char))
    }

    fn lex_number(&mut self, start: usize) -> PResult<Token> {
        let s = self.src;
        let mut i = self.pos;
        let mut value: f64;

        if s[i] == b'0' && i + 1 < s.len() && (s[i + 1] == b'x' || s[i + 1] == b'X') {
            i += 2;
            let d0 = i;
            let mut v = 0.0f64;
            while i < s.len() && s[i].is_ascii_hexdigit() {
                v = v * 16.0 + (s[i] as char).to_digit(16).unwrap() as f64;
                i += 1;
            }
            if i == d0 {
                return perr(start, "missing hex digits");
            }
            value = v;
        } else if s[i] == b'0'
            && i + 1 < s.len()
            && (s[i + 1] == b'b' || s[i + 1] == b'B' || s[i + 1] == b'o' || s[i + 1] == b'O')
        {
            let radix = if s[i + 1] == b'b' || s[i + 1] == b'B' {
                2u32
            } else {
                8u32
            };
            i += 2;
            let d0 = i;
            let mut v = 0.0f64;
            while i < s.len() && (s[i] as char).is_digit(radix) {
                v = v * radix as f64 + (s[i] as char).to_digit(radix).unwrap() as f64;
                i += 1;
            }
            if i == d0 {
                return perr(start, "missing digits in radix literal");
            }
            value = v;
        } else {
            // Decimal: digits [. digits?] [eE [+-] digits] — or leading dot.
            while i < s.len() && s[i].is_ascii_digit() {
                i += 1;
            }
            if i < s.len() && s[i] == b'.' {
                i += 1;
                while i < s.len() && s[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
                let mut j = i + 1;
                if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
                    j += 1;
                }
                if j < s.len() && s[j].is_ascii_digit() {
                    while j < s.len() && s[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                } else {
                    // `1e` with no digits: JS numeric literal error.
                    return perr(start, "missing exponent digits");
                }
            }
            let text = std::str::from_utf8(&s[start..i]).unwrap();
            value = match text.parse::<f64>() {
                Ok(v) => v,
                Err(_) => return perr(start, "invalid number literal"),
            };
        }

        // JS: an identifier character may not directly follow a numeric
        // literal (`22436B` is a SyntaxError, which is how bracket PID ids
        // with hex letters fail on the QuickJS backend today).
        if i < s.len() && is_ident_start(s[i]) {
            return perr(i, "identifier starts immediately after numeric literal");
        }
        if value == 0.0 {
            value = 0.0; // normalize -0 impossible here; keep literal +0
        }
        self.pos = i;
        Ok(Token {
            tok: Tok::Num(value),
            start,
            end: i,
        })
    }

    fn lex_string(&mut self, start: usize, quote: u8) -> PResult<Token> {
        let s = self.src;
        let mut i = self.pos + 1;
        let mut out = String::new();
        while i < s.len() {
            let c = s[i];
            if c == quote {
                self.pos = i + 1;
                return Ok(Token {
                    tok: Tok::Str(out),
                    start,
                    end: self.pos,
                });
            }
            if c == b'\\' && i + 1 < s.len() {
                let e = s[i + 1];
                i += 2;
                match e {
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => out.push('\r'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'v' => out.push('\u{b}'),
                    b'0' => out.push('\0'),
                    b'x' => {
                        if i + 1 < s.len()
                            && s[i].is_ascii_hexdigit()
                            && s[i + 1].is_ascii_hexdigit()
                        {
                            let hi = (s[i] as char).to_digit(16).unwrap();
                            let lo = (s[i + 1] as char).to_digit(16).unwrap();
                            out.push(char::from_u32(hi * 16 + lo).unwrap_or('\u{fffd}'));
                            i += 2;
                        } else {
                            return perr(i - 2, "invalid \\x escape");
                        }
                    }
                    b'u' => {
                        if i + 3 < s.len() && s[i..i + 4].iter().all(|b| b.is_ascii_hexdigit()) {
                            let code =
                                u32::from_str_radix(std::str::from_utf8(&s[i..i + 4]).unwrap(), 16)
                                    .unwrap();
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                            i += 4;
                        } else {
                            return perr(i - 2, "invalid \\u escape");
                        }
                    }
                    other => {
                        // JS: unknown escape yields the character itself.
                        out.push(other as char);
                    }
                }
                continue;
            }
            // Multi-byte UTF-8 passthrough.
            let ch_len = utf8_len(c);
            if i + ch_len <= s.len() {
                out.push_str(
                    std::str::from_utf8(&s[i..i + ch_len]).map_err(|_| ParseError {
                        pos: i,
                        msg: "invalid utf-8 in string".into(),
                    })?,
                );
                i += ch_len;
            } else {
                return perr(i, "invalid utf-8 in string");
            }
        }
        perr(start, "unterminated string literal")
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >= 0xF0 {
        4
    } else if b >= 0xE0 {
        3
    } else {
        2
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'s> {
    src: &'s str,
    lx: Lexer<'s>,
    /// LOOKUP/CLOSEST textual argument segmentation enabled? (Disabled while
    /// re-parsing a processed argument list, matching the single-pass
    /// left-to-right behaviour of the textual preprocessor.)
    lookup_special: bool,
    /// Raw-JS submode (ZEROREF eval strings): no colon separators, no
    /// natural IF, lazy ternary.
    js_mode: bool,
    depth: u32,
}

fn str_expr(s: String) -> Expr {
    let has_val_ref = contains_val_marker(&s);
    Expr::Str { s, has_val_ref }
}

pub(crate) fn contains_val_marker(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 5 {
        return false;
    }
    for i in 0..b.len() - 3 {
        if (b[i] == b'v' || b[i] == b'V')
            && (b[i + 1] == b'a' || b[i + 1] == b'A')
            && (b[i + 2] == b'l' || b[i + 2] == b'L')
            && b[i + 3] == b'{'
        {
            return true;
        }
    }
    false
}

impl<'s> Parser<'s> {
    fn new(src: &'s str, lookup_special: bool, js_mode: bool) -> Self {
        Self {
            src,
            lx: Lexer::new(src),
            lookup_special,
            js_mode,
            depth: 0,
        }
    }

    fn enter(&mut self, pos: usize) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return perr(pos, "expression too deeply nested");
        }
        Ok(())
    }
    fn leave(&mut self) {
        self.depth -= 1;
    }

    // -- program ------------------------------------------------------------

    fn parse_program(&mut self) -> PResult<Expr> {
        // Empty program: JS eval("") → undefined → Numeric(0).
        if matches!(self.lx.peek()?.tok, Tok::Eof) {
            return Ok(Expr::Undefined);
        }
        let expr = if !self.js_mode && self.at_natural_if()? {
            self.parse_natural_if()?
        } else {
            let mut items = vec![self.parse_expr()?];
            while matches!(self.lx.peek()?.tok, Tok::Comma) {
                self.lx.next()?;
                items.push(self.parse_expr()?);
            }
            if items.len() == 1 {
                items.pop().unwrap()
            } else {
                Expr::Seq(items)
            }
        };
        let t = self.lx.peek()?;
        if !matches!(t.tok, Tok::Eof) {
            return perr(t.start, "unexpected trailing input");
        }
        Ok(expr)
    }

    // -- natural IF … THEN … ELSE … ------------------------------------------

    /// The textual transform requires `IF` followed by a literal space
    /// (`trimmed.starts_with("IF ")`, and `" THEN "` / `" ELSE "` with
    /// single-space delimiters at paren depth 0). We replicate those exact
    /// conditions on the token stream + raw source bytes.
    fn at_natural_if(&mut self) -> PResult<bool> {
        let t = self.lx.peek()?;
        if let Tok::Ident(name) = &t.tok {
            if name == "IF" {
                let end = t.end;
                return Ok(self.src.as_bytes().get(end) == Some(&b' '));
            }
        }
        Ok(false)
    }

    /// True when the upcoming token is the keyword `kw` delimited by literal
    /// spaces on both sides (the `" THEN "` / `" ELSE "` requirement).
    fn at_keyword(&mut self, kw: &str) -> PResult<bool> {
        let t = self.lx.peek()?;
        if let Tok::Ident(name) = &t.tok {
            if name == kw {
                let b = self.src.as_bytes();
                let before = t.start > 0 && b[t.start - 1] == b' ';
                let after = b.get(t.end) == Some(&b' ');
                return Ok(before && after);
            }
        }
        Ok(false)
    }

    /// Desugars to `IF(cond…, then…, else…)` with flattened comma-lists —
    /// exactly the shape `transform_if_syntax` emits (which is why a missing
    /// ELSE appends `0` and surplus list items become extra call arguments).
    fn parse_natural_if(&mut self) -> PResult<Expr> {
        let if_tok = self.lx.next()?; // consume IF
        let start = if_tok.start;
        self.enter(start)?;
        let mut args: Vec<Expr> = Vec::new();

        // Condition list until THEN.
        loop {
            if self.at_keyword("THEN")? {
                break;
            }
            args.push(self.parse_expr()?);
            if matches!(self.lx.peek()?.tok, Tok::Comma) {
                self.lx.next()?;
                continue;
            }
            break;
        }
        if !self.at_keyword("THEN")? {
            let t = self.lx.peek()?;
            return perr(t.start, "expected THEN in IF expression");
        }
        if args.is_empty() {
            // Textual transform would emit `IF(, …)` — a syntax error.
            let t = self.lx.peek()?;
            return perr(t.start, "empty condition in IF expression");
        }
        self.lx.next()?; // THEN

        // Then list until ELSE / end.
        let then_start = args.len();
        loop {
            if self.at_keyword("ELSE")? || matches!(self.lx.peek()?.tok, Tok::Eof) {
                break;
            }
            args.push(self.parse_expr()?);
            if matches!(self.lx.peek()?.tok, Tok::Comma) {
                self.lx.next()?;
                continue;
            }
            break;
        }
        if args.len() == then_start {
            // Textual transform would emit `IF(cond, , …)` — a syntax error.
            let t = self.lx.peek()?;
            return perr(t.start, "empty THEN branch in IF expression");
        }

        if self.at_keyword("ELSE")? {
            self.lx.next()?; // ELSE
            if self.at_natural_if()? {
                let nested = self.parse_natural_if()?;
                args.push(nested);
            } else {
                loop {
                    args.push(self.parse_expr()?);
                    if matches!(self.lx.peek()?.tok, Tok::Comma) {
                        self.lx.next()?;
                        continue;
                    }
                    break;
                }
            }
        } else {
            // No ELSE: the transform appends a literal 0.
            args.push(Expr::Num(0.0));
        }

        self.leave();
        let end = self.lx.peek()?.start;
        Ok(Expr::Call {
            name: "IF".to_string(),
            args,
            span: start..end,
        })
    }

    // -- expressions ----------------------------------------------------------

    fn parse_expr(&mut self) -> PResult<Expr> {
        let pos = self.lx.peek()?.start;
        self.enter(pos)?;
        let r = self.parse_ternary();
        self.leave();
        r
    }

    fn parse_ternary(&mut self) -> PResult<Expr> {
        let cond = self.parse_binary(0)?;
        if matches!(self.lx.peek()?.tok, Tok::Question) {
            let q = self.lx.next()?;
            let then_e = self.parse_expr()?;
            let t = self.lx.next()?;
            if !matches!(t.tok, Tok::Colon) {
                return perr(t.start, "expected ':' in ternary expression");
            }
            let else_e = self.parse_expr()?;
            // The textual pipeline rewrites `c ? a : b` into the IF(...)
            // CALL, whose arguments evaluate eagerly — replicate that shape
            // in dialect mode. Raw-JS mode (ZEROREF eval strings) keeps the
            // real, lazy ternary.
            return Ok(if self.js_mode {
                Expr::Ternary(Box::new(cond), Box::new(then_e), Box::new(else_e))
            } else {
                let end = self.lx.peek()?.start;
                Expr::Call {
                    name: "IF".to_string(),
                    args: vec![cond, then_e, else_e],
                    span: q.start..end,
                }
            });
        }
        Ok(cond)
    }

    fn parse_binary(&mut self, min_bp: u8) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let (op, l_bp, right_assoc) = match &self.lx.peek()?.tok {
                Tok::Op("||") => (BinOp::Or, 1, false),
                Tok::Op("&&") => (BinOp::And, 2, false),
                Tok::Op("|") => (BinOp::BitOr, 3, false),
                Tok::Op("^") => (BinOp::BitXor, 4, false),
                Tok::Op("&") => (BinOp::BitAnd, 5, false),
                Tok::Op("==") => (BinOp::EqLoose, 6, false),
                Tok::Op("!=") => (BinOp::NeLoose, 6, false),
                Tok::Op("===") => (BinOp::EqStrict, 6, false),
                Tok::Op("!==") => (BinOp::NeStrict, 6, false),
                Tok::Op("<") => (BinOp::Lt, 7, false),
                Tok::Op(">") => (BinOp::Gt, 7, false),
                Tok::Op("<=") => (BinOp::Le, 7, false),
                Tok::Op(">=") => (BinOp::Ge, 7, false),
                Tok::Op("<<") => (BinOp::Shl, 8, false),
                Tok::Op(">>") => (BinOp::Shr, 8, false),
                Tok::Op(">>>") => (BinOp::UShr, 8, false),
                Tok::Op("+") => (BinOp::Add, 9, false),
                Tok::Op("-") => (BinOp::Sub, 9, false),
                Tok::Op("*") => (BinOp::Mul, 10, false),
                Tok::Op("/") => (BinOp::Div, 10, false),
                Tok::Op("%") => (BinOp::Mod, 10, false),
                Tok::Op("**") => (BinOp::Pow, 11, true),
                _ => break,
            };
            if l_bp < min_bp {
                break;
            }
            let op_tok = self.lx.next()?;
            // JS: an unparenthesized unary expression may not be the left
            // operand of ** (`-2**2` is a SyntaxError).
            if op == BinOp::Pow && matches!(lhs, Expr::Unary(..)) {
                return perr(op_tok.start, "unparenthesized unary operand of **");
            }
            let next_min = if right_assoc { l_bp } else { l_bp + 1 };
            let rhs = self.parse_binary(next_min)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let t = self.lx.peek()?;
        let un = match &t.tok {
            Tok::Op("-") => Some(UnOp::Neg),
            Tok::Op("+") => Some(UnOp::Plus),
            Tok::Op("!") => Some(UnOp::Not),
            Tok::Op("~") => Some(UnOp::BitNot),
            _ => None,
        };
        if let Some(op) = un {
            let pos = t.start;
            self.lx.next()?;
            self.enter(pos)?;
            let operand = self.parse_unary()?;
            self.leave();
            return Ok(Expr::Unary(op, Box::new(operand)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let t = self.lx.next()?;
        match t.tok {
            Tok::Num(v) => Ok(Expr::Num(v)),
            Tok::Str(s) => Ok(str_expr(s)),
            Tok::ValRef(name) => Ok(Expr::ValRef {
                name,
                span: t.start..t.end,
            }),
            Tok::Ident(name) => self.parse_ident_or_call(name, t.start, t.end),
            Tok::LParen => self.parse_group(t.start),
            Tok::LBracket => self.parse_bracket(t.start),
            Tok::Eof => perr(t.start, "unexpected end of expression"),
            other => perr(t.start, format!("unexpected token {:?}", other)),
        }
    }

    fn parse_ident_or_call(&mut self, name: String, start: usize, end: usize) -> PResult<Expr> {
        // Keyword literals.
        match name.as_str() {
            "true" => return Ok(Expr::Bool(true)),
            "false" => return Ok(Expr::Bool(false)),
            "undefined" => return Ok(Expr::Undefined),
            _ => {}
        }

        // Call? (whitespace between name and '(' allowed, as in the
        // normalize_function_case regex).
        if matches!(self.lx.peek()?.tok, Tok::LParen) {
            let lparen = self.lx.next()?;
            let upper = name.to_uppercase();
            let canonical = if CANON_FUNCS.contains(&upper.as_str()) {
                upper
            } else {
                name
            };

            // LOOKUP/CLOSEST textual argument segmentation — only when '('
            // is directly adjacent, exactly like the `LOOKUP(` scan in
            // preprocess_colon_parameters.
            if self.lookup_special
                && (canonical == "LOOKUP" || canonical == "CLOSEST")
                && lparen.start == end
            {
                return self.parse_lookup_call(canonical, start, lparen.end);
            }

            let mut args = Vec::new();
            if matches!(self.lx.peek()?.tok, Tok::RParen) {
                self.lx.next()?;
            } else {
                loop {
                    args.push(self.parse_expr()?);
                    let sep = self.lx.next()?;
                    match sep.tok {
                        Tok::Comma => continue,
                        // ':' inside any call parens is an argument
                        // separator (the depth>0 colon→comma rewrite) —
                        // dialect only; raw JS rejects it.
                        Tok::Colon if !self.js_mode => continue,
                        Tok::RParen => break,
                        _ => return perr(sep.start, "expected ',', ':' or ')' in argument list"),
                    }
                }
            }
            let call_end = self.lx.peek()?.start;
            return Ok(Expr::Call {
                name: canonical,
                args,
                span: start..call_end,
            });
        }

        Ok(Expr::Ident {
            name,
            span: start..end,
        })
    }

    /// Extract the raw argument text of a LOOKUP/CLOSEST call, run the
    /// byte-exact port of `process_lookup_args`, and re-parse the processed
    /// argument list (with segmentation disabled, single pass).
    fn parse_lookup_call(
        &mut self,
        name: String,
        call_start: usize,
        args_start: usize,
    ) -> PResult<Expr> {
        let b = self.src.as_bytes();
        // Find matching ')' with a plain paren counter (NOT quote-aware —
        // faithful to the preprocessor's scan).
        let mut depth = 1usize;
        let mut i = args_start;
        while i < b.len() && depth > 0 {
            match b[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            if depth > 0 {
                i += 1;
            }
        }
        if depth != 0 {
            return perr(
                call_start,
                "unbalanced parentheses in LOOKUP/CLOSEST arguments",
            );
        }
        let raw = &self.src[args_start..i];
        let processed = process_lookup_args(raw);

        // Re-parse the processed argument list.
        let mut args = Vec::new();
        {
            let mut sub = Parser::new(&processed, false, self.js_mode);
            if !matches!(
                sub.lx.peek().map_err(|e| reloc(e, call_start))?.tok,
                Tok::Eof
            ) {
                loop {
                    let e = sub.parse_expr().map_err(|e| reloc(e, call_start))?;
                    args.push(e);
                    let sep = sub.lx.next().map_err(|e| reloc(e, call_start))?;
                    match sep.tok {
                        Tok::Comma => continue,
                        Tok::Eof => break,
                        _ => {
                            return perr(
                                call_start,
                                format!(
                                "invalid LOOKUP/CLOSEST argument list (processed: {processed:?})"
                            ),
                            )
                        }
                    }
                }
            }
        }

        self.lx.seek(i + 1); // past ')'
        Ok(Expr::Call {
            name,
            args,
            span: call_start..i + 1,
        })
    }

    fn parse_group(&mut self, start: usize) -> PResult<Expr> {
        self.enter(start)?;
        let mut items = vec![self.parse_expr()?];
        loop {
            let t = self.lx.next()?;
            match t.tok {
                Tok::RParen => break,
                // ',' is the JS comma operator; ':' inside parens is
                // rewritten to ',' by the pipeline (dialect mode only).
                Tok::Comma => items.push(self.parse_expr()?),
                Tok::Colon if !self.js_mode => items.push(self.parse_expr()?),
                _ => return perr(t.start, "expected ')' in parenthesized expression"),
            }
        }
        self.leave();
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Expr::Seq(items)
        })
    }

    fn parse_bracket(&mut self, start: usize) -> PResult<Expr> {
        self.enter(start)?;
        let mut items = Vec::new();
        if matches!(self.lx.peek()?.tok, Tok::RBracket) {
            self.lx.next()?;
        } else {
            loop {
                items.push(self.parse_expr()?);
                let t = self.lx.next()?;
                match t.tok {
                    Tok::RBracket => break,
                    Tok::Comma => continue,
                    _ => return perr(t.start, "expected ']' or ',' in bracket group"),
                }
            }
        }
        self.leave();
        Ok(Expr::Bracket(items))
    }
}

fn reloc(mut e: ParseError, base: usize) -> ParseError {
    // Errors inside re-parsed LOOKUP argument text: point at the call.
    e.pos = base;
    e.msg = format!("in LOOKUP/CLOSEST arguments: {}", e.msg);
    e
}

// ---------------------------------------------------------------------------
// process_lookup_args — byte-exact port of the private helper in
// src/parser.rs (the textual definition of LOOKUP/CLOSEST argument arms).
// Splits on ':' (ignoring nesting), double-quotes key=value segments,
// expands integer `a~b=v` ranges into discrete quoted arms, and emits ""
// for empty / whitespace-only segments.
// ---------------------------------------------------------------------------

fn process_lookup_args(args: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = args.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch == ':' {
            if result.is_empty() || result.ends_with(',') {
                result.push_str("\"\"");
            }
            result.push(',');
            i += 1;
        } else if chars[i..]
            .iter()
            .take_while(|c| **c != ':')
            .all(|c| c.is_whitespace())
            && chars[i..].contains(&':')
        {
            // Whitespace-only segment before the next colon → "".
            result.push_str("\"\"");
            while i < chars.len() && chars[i] != ':' {
                i += 1;
            }
            i += 1; // consume the ':'
            result.push(',');
        } else {
            let mut segment = String::new();
            let mut has_single_equals = false;

            while i < chars.len() && chars[i] != ':' {
                let c = chars[i];
                segment.push(c);
                if c == '=' {
                    let prev_is_comparison = i > 0
                        && (chars[i - 1] == '<' || chars[i - 1] == '>' || chars[i - 1] == '!');
                    let next_is_equals = i + 1 < chars.len() && chars[i + 1] == '=';
                    if !prev_is_comparison && !next_is_equals {
                        has_single_equals = true;
                    }
                }
                i += 1;
            }

            // Tilde range: "lo~hi=value" with integer bounds expands to
            // discrete quoted "N=value" arms.
            let tilde_range_expanded = if let Some(tilde_pos) = segment.find('~') {
                if let Some(eq_pos) = segment.find('=') {
                    if tilde_pos < eq_pos {
                        if let Some((range_part, value_part)) = segment.split_once('=') {
                            let range_parts: Vec<&str> = range_part.split('~').collect();
                            if range_parts.len() == 2 {
                                if let (Ok(lo), Ok(hi)) = (
                                    range_parts[0].trim().parse::<i64>(),
                                    range_parts[1].trim().parse::<i64>(),
                                ) {
                                    let mut first = true;
                                    for n in lo..=hi {
                                        if !first {
                                            result.push(',');
                                        }
                                        result.push('"');
                                        result.push_str(&format!("{}={}", n, value_part.trim()));
                                        result.push('"');
                                        first = false;
                                    }
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if !tilde_range_expanded {
                if has_single_equals {
                    result.push('"');
                    result.push_str(&segment);
                    result.push('"');
                } else {
                    result.push_str(&segment);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(src: &str) -> Ast {
        match compile(src) {
            Ok(a) => a,
            Err(e) => panic!("failed to parse {src:?}: {e}"),
        }
    }

    #[test]
    fn parses_basic_arithmetic() {
        ok("A*256+B");
        ok("(A-48)");
        ok("0.5 * (SIGNED(A)) + -64");
        ok("A*.2");
        ok("4.5");
    }

    #[test]
    fn parses_colon_args_and_natural_if() {
        ok("BIT(A:3)");
        ok("INT16(A:B)*0.01");
        ok("IF BIT(A:4) == 1 THEN 'NoFault' ELSE (B*256)+C");
        ok("IF B == 0 THEN 'OFF' ELSE IF B == 254 THEN 'ERROR' ELSE B");
        ok("IF A > 128 THEN 1");
    }

    #[test]
    fn parses_lookup_forms() {
        ok("LOOKUP(A:'PTO not active':1='PTO active')");
        ok("LOOKUP(BITVALUE(A:0:1):3:0='X':1='Y')");
        ok("LOOKUP(A::0~3=4:4~5=7)");
        ok("LOOKUP(BIT(A, 7), 'Off', '1=On')");
        ok("CLOSEST(A:A:1='low':255='high')");
        ok("LOOKUP((1.15*(VAL{MGZSEV_Cons_60MPH}-3.8)+1)::2.38~10=2.38:0~1=1:1.149~1.15=1)");
    }

    #[test]
    fn parses_misc_dialect() {
        ok("val{Engine RPM}*0.1");
        ok("[222885]*[222414]/1000");
        ok("((((A*256)+B)>>8)&1)");
        ok("A > 0 ? 1 : 0");
        ok("ZEROREF(BIT(A:7)==1, -40, '(A & 0x7F) * 0.5')");
        ok("ascii()");
        ok("~A & 0xFF");
        ok("A >>> 1");
        ok("");
    }

    #[test]
    fn rejects_invalid() {
        assert!(compile("A +").is_err());
        assert!(compile("LOOKUP(A:0").is_err());
        assert!(compile("IF A 1 ELSE 0").is_err());
        assert!(compile("22436B").is_err()); // ident right after number, like JS
        assert!(compile("A ; B").is_err());
        assert!(compile("'unterminated").is_err());
        assert!(compile("A : B").is_err()); // top-level colon
    }

    #[test]
    fn natural_if_requires_exact_spacing() {
        // "IF\tA THEN 1" is not transformed by the pipeline (SyntaxError).
        assert!(compile("IF\tA THEN 1 ELSE 0").is_err());
    }
}
