//! AST for the native Torque-dialect evaluator (NA1).
//!
//! The tree is immutable after `compile()` and contains only owned data
//! (`String`, `f64`, `Vec`, `Box`), so `Ast` is naturally `Send + Sync`
//! (asserted in `mod.rs` tests). Spans are byte offsets into the ORIGINAL
//! source string; nodes created by LOOKUP/CLOSEST argument re-segmentation
//! carry the span of the enclosing call (best effort — the textual
//! segmentation step, faithfully ported from the QuickJS pipeline, does not
//! preserve exact positions).

/// Byte-offset span into the original expression source.
pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `-x`
    Neg,
    /// `+x`
    Plus,
    /// `!x`
    Not,
    /// `~x`
    BitNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
    Lt,
    Gt,
    Le,
    Ge,
    /// `==`
    EqLoose,
    /// `!=`
    NeLoose,
    /// `===`
    EqStrict,
    /// `!==`
    NeStrict,
    /// `&&` (returns an operand, not a bool; lazy)
    And,
    /// `||` (returns an operand, not a bool; lazy)
    Or,
}

#[derive(Debug, Clone)]
pub enum Expr {
    /// Numeric literal (decimal, hex `0x`, binary `0b`, octal `0o`).
    Num(f64),
    /// String literal (single- or double-quoted). `has_val_ref` is true when
    /// the content contains a `val{…}` marker: the QuickJS pipeline's PID
    /// replacement is a global, quote-blind regex pass, so `val{X}` inside a
    /// string literal is substituted textually — replicated at eval time.
    Str {
        s: String,
        has_val_ref: bool,
    },
    /// `true` / `false`.
    Bool(bool),
    /// `undefined` literal / empty program.
    Undefined,
    /// Identifier: byte variable, user variable, or `NaN`/`Infinity`.
    Ident {
        name: String,
        span: Span,
    },
    /// `val{PID NAME}` cross-PID reference, resolved at eval time.
    ValRef {
        name: String,
        span: Span,
    },
    /// `[a, b, …]` — JS array literal; evaluates via ToPrimitive to the
    /// comma-joined string of its elements (Torque bracket PID references
    /// like `[222885]` reach QuickJS as arrays today; semantics preserved).
    Bracket(Vec<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `cond ? a : b` (lazy branches).
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Function call. `name` is canonical-case for the 26 case-normalized
    /// function names, as written otherwise. Args are ALWAYS evaluated
    /// eagerly (JS call semantics — including `IF(...)`).
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Comma/colon sequence (`(A, B)`, `(A : B)` in group position, or a
    /// top-level comma expression). Evaluates every element, yields the last.
    Seq(Vec<Expr>),
}

/// A compiled expression. Immutable; safe to share across threads.
#[derive(Debug, Clone)]
pub struct Ast {
    pub(crate) root: Expr,
    /// Original source, kept for error messages.
    pub(crate) source: String,
}

impl Ast {
    pub fn root(&self) -> &Expr {
        &self.root
    }
    pub fn source(&self) -> &str {
        &self.source
    }
}
