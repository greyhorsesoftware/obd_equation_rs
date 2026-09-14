//! # obd_equation_rs
//!
//! A high-performance equation evaluation library for OBD-II applications.
//!
//! This library evaluates the Torque equation dialect with a native
//! compile-once AST evaluator (src/native/) — no JavaScript engine. Dialect
//! behavior is pinned to the retired QuickJS backend's last build by the
//! recorded-oracle suite in tests/corpus_snapshot.rs.
//!
//! ## Features
//!
//! - **Mathematical Functions**: MIN, MAX, ABS, SQRT, SIN, COS, TAN, LOG, etc.
//! - **Bit Operations**: BIT, BITVALUE, BITSELECT, IFANY
//! - **Data Conversion**: INT16, INT24, INT32, SIGNED, SIGNED16, FLOAT32, FLOAT64
//! - **Lookup Tables**: LOOKUP, CLOSEST with range and string support
//! - **Control Flow**: IF, ZEROREF
//! - **Stateful Functions**: EWMAF, RAVG, AVG, TAVG, TDLY, RDLY, TOT
//! - **Platform Callbacks**: BARO (barometric pressure), PID value resolution
//! - **Cross-PID References**: `val{PID_NAME}` syntax for referencing other PIDs
//!
//! ## Example
//!
//! ```rust
//! use obd_equation_rs::ExpressionEvaluator;
//! use std::collections::HashMap;
//!
//! let mut evaluator = ExpressionEvaluator::new().unwrap();
//! let result = evaluator.evaluate("2 + 3", &HashMap::new()).unwrap();
//! assert_eq!(result, 5.0);
//! ```

pub mod callbacks;
pub mod error;
pub mod evaluator;
pub mod ffi;
pub mod generator;
pub mod native;
pub mod runtime;
pub mod state;
pub mod variables;

// Re-export main types
pub use callbacks::{DefaultPlatformCallbacks, PlatformCallbacks};
pub use error::{EvaluatorError, Result};
pub use evaluator::ExpressionEvaluator;
pub use runtime::ExpressionResult;
