# obd_equation_rs

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#-license)
[![Tests](https://img.shields.io/badge/tests-passing-green)](obd_equation_rs/src/lib.rs)

A high-performance evaluator for the Torque equation dialect used in OBD-II diagnostic applications, written in Rust. The crate parses and evaluates equations natively with its own lexer, parser, and AST (`src/native/`) — no JavaScript engine. Expressions compile once and evaluate in tens of nanoseconds per sample.

## 🚀 Key Features

- **High Performance**: compile-once/eval-many native AST — 36 ns median warm eval for a simple equation
- **Memory Safety**: Guaranteed by Rust's borrow checker - no memory corruption bugs
- **Thread Safety**: Concurrent evaluation with proper state isolation
- **Dialect Parity**: behavior pinned by an 82k-case recorded-oracle corpus (`tests/corpus_snapshot.rs`)
- **Platform Integration**: Native iOS/macOS platform callbacks (BARO, etc.)
- **Production Ready**: Comprehensive test suite (corpus snapshot, golden reference-PID CSV, parser fuzz)

## 📦 What's Included

### Core Mathematical Functions
- **Basic Math**: MIN, MAX, ABS, SQRT, SIN, COS, TAN, LOG, etc.
- **Bit Operations**: BIT, BITVALUE, BITSELECT, IFANY
- **Data Conversion**: INT16, INT24, INT32, SIGNED, SIGNED16
- **Lookup Tables**: LOOKUP, CLOSEST with range and string support
- **Control Flow**: IF, ZEROREF
- **Stateful Functions**: EWMAF, RAVG, AVG, TAVG, TDLY, RDLY, TOT

### Advanced Features
- **Colon Parameter Parsing**: `MAX(INT16(A:B)/100:C)`
- **Variable System**: A-Z byte variables, PID cross-references
- **Time-Based State**: Thread-safe state management across evaluations
- **Platform Callbacks**: iOS CoreMotion integration (barometric pressure)

## 🏗️ Building

### Prerequisites

- **Rust**: 1.70 or later ([install here](https://rustup.rs/))
- No external runtime dependencies — pure Rust (`serde`, `serde_json`, `thiserror`)
- **For FFI/Swift integration**: `cbindgen` header generation (see `cbindgen.toml`)

### Build Commands

```bash
# Clone the repository
git clone https://github.com/greyhorsesoftware/obd_equation_rs.git
cd obd_equation_rs

# Quick build for development
cargo build --release

# Run tests
cargo test

# Build documentation
cargo doc --open

# Build universal libraries for iOS/macOS (recommended)
./build_universal.sh --all
```

### Manual Build Process

For manual builds, install targets and run individual commands:

```bash
# Install required targets
rustup target add aarch64-apple-ios x86_64-apple-ios x86_64-apple-darwin aarch64-apple-darwin

# iOS universal (device + simulator)
cargo build --release --target aarch64-apple-ios
cargo build --release --target x86_64-apple-ios
lipo -create target/aarch64-apple-ios/release/libobd_equation_rs.a \
             target/x86_64-apple-ios/release/libobd_equation_rs.a \
             -output target/universal-ios/release/libobd_equation_rs.a

# macOS universal (Intel + Apple Silicon)
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create target/x86_64-apple-darwin/release/libobd_equation_rs.a \
             target/aarch64-apple-darwin/release/libobd_equation_rs.a \
             -output target/universal-macos/release/libobd_equation_rs.a
```

### Xcode Integration

After building, the universal libraries will be available in the `lib/` directory. Integrate them into Xcode:

1. **Link Libraries**: Add `libobd_equation_rs.a` to "Link Binary With Libraries"
2. **Library Search Paths**: `$(PROJECT_DIR)/equation_rs/lib/universal-ios`
3. **Header Search Paths**: Include paths to generated FFI headers

The script creates universal binaries in both:
- `target/universal-*/release/` (standard Rust build output)
- `lib/universal-*/` (convenient access for integration)

Use the `lib/` path for cleaner Xcode integration.

### Output Structure

The build script creates universal libraries in two locations:

```
equation_rs/target/universal-ios/release/libobd_equation_rs.a    # Standard Rust output
equation_rs/target/universal-macos/release/libobd_equation_rs.a  # Standard Rust output
equation_rs/lib/universal-ios/libobd_equation_rs.a              # Convenient access
equation_rs/lib/universal-macos/libobd_equation_rs.a            # Convenient access
```

### Prerequisites

Install required Rust targets for cross-compilation:

```bash
rustup target add aarch64-apple-ios x86_64-apple-ios  # iOS
rustup target add x86_64-apple-darwin aarch64-apple-darwin  # macOS
```

The `build_universal.sh` script automatically validates all prerequisites and provides helpful error messages.
```

### Build Configuration

The library is configured for optimal performance:

```toml
[package]
name = "obd_equation_rs"
version = "1.0.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"

[dev-dependencies]
criterion = "0.5"

[lib]
name = "obd_equation_rs"
crate-type = ["lib", "cdylib", "staticlib"]
```

## 📖 Usage

### Basic Usage

```rust
use obd_equation_rs::ExpressionEvaluator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create evaluator
    let mut evaluator = ExpressionEvaluator::new()?;

    // Simple arithmetic
    let result = evaluator.evaluate("A + 10", &[])?;
    println!("Result: {}", result); // 110.0 (assuming A = 100)

    // With bytes
    let bytes = [100, 50, 0, 0];
    let result = evaluator.evaluate_with_bytes("A * 2", &[], &bytes)?;
    println!("Result: {}", result); // 200.0

    Ok(())
}
```

### Advanced Features

```rust
use obd_equation_rs::{ExpressionEvaluator, PlatformCallbacks};
use std::collections::HashMap;

// Platform callbacks for iOS/macOS features
struct IOSCallbacks;
impl PlatformCallbacks for IOSCallbacks {
    fn get_barometric_pressure(&self) -> f64 {
        // Return barometric pressure from CoreMotion
        14.7 // Example: 14.7 PSI at sea level
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create evaluator with platform callbacks
    let callbacks = Box::new(IOSCallbacks);
    let mut evaluator = ExpressionEvaluator::new_with_platform_callbacks(Some(callbacks))?;

    // Use platform-specific functions
    let pressure = evaluator.evaluate("BARO()", &HashMap::new())?;
    println!("Barometric pressure: {} PSI", pressure);

    // Complex expressions with stateful functions
    let ewma1 = evaluator.evaluate("EWMAF(0.5, 100)", &HashMap::new())?;
    let ewma2 = evaluator.evaluate("EWMAF(0.5, 200)", &HashMap::new())?;
    println!("EWMA: {} -> {}", ewma1, ewma2); // 100.0 -> 150.0

    // Data conversion functions
    let bytes = [1, 44, 0, 0];
    let int16 = evaluator.evaluate_with_bytes("INT16(A,B)", &HashMap::new(), &bytes)?;
    println!("INT16 result: {}", int16); // 300.0

    Ok(())
}
```

## 🧪 Testing

The library includes a comprehensive test suite that validates all functionality:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_data_conversion_functions

# Run benchmarks (criterion — benches/eval_bench.rs: warm/cold eval latency
# for a simple equation and the full corpus)
cargo bench
```

### Test Coverage

- ✅ **82k-case recorded-oracle corpus** + stateful sequences (`tests/corpus_snapshot.rs`)
- ✅ **Golden reference-PID CSV** + pinned parse rejects + parser fuzz (`tests/native_regression.rs`)
- ✅ **Data conversion functions** (INT16, INT24, INT32, SIGNED)
- ✅ **Mathematical operations** (MIN, MAX, trigonometry, etc.)
- ✅ **Bit operations** (BIT, BITVALUE, BITSELECT, IFANY)
- ✅ **Lookup tables** with ranges and strings
- ✅ **Stateful functions** (EWMAF, RAVG, AVG, etc.)
- ✅ **Colon parameter parsing**
- ✅ **Platform callbacks**
- ✅ **Error handling**

## 📚 API Documentation

### Core Types

```rust
pub struct ExpressionEvaluator {
    // Main interface for evaluating expressions
}

pub trait PlatformCallbacks: Send + Sync {
    fn get_barometric_pressure(&self) -> f64;
    // Additional platform-specific methods
}

#[derive(Debug)]
pub enum EvaluatorError {
    JavaScriptError(String), // legacy name, kept for source/FFI compatibility
    ParseError(String),
    VariableError(String),
    TypeError(String),
    StateError(String),
    FunctionError(String),
}
```

### Key Methods

```rust
impl ExpressionEvaluator {
    // Create new evaluator
    pub fn new() -> Result<Self, EvaluatorError>

    // Create with platform callbacks
    pub fn new_with_platform_callbacks(callbacks: Option<Box<dyn PlatformCallbacks>>)
        -> Result<Self, EvaluatorError>

    // Evaluate expression with variables
    pub fn evaluate(&mut self, expression: &str, variables: &HashMap<String, f64>)
        -> Result<f64, EvaluatorError>

    // Evaluate with byte array (A-Z variables)
    pub fn evaluate_with_bytes(&mut self, expression: &str, variables: &HashMap<String, f64>, bytes: &[u8])
        -> Result<f64, EvaluatorError>

    // Drop the compiled-AST memo (stateful accumulators survive);
    // the next evaluation lazily recompiles
    pub fn release_runtime(&mut self)
}
```

## 🔧 Integrating from other languages

The crate builds as a `staticlib` / `cdylib` with a C API; `cbindgen --config cbindgen.toml --crate obd_equation_rs --output obd_equation_rs.h` generates the header (CI attaches it to every build). Every exported function is documented in `src/ffi.rs` (`cargo doc --open`). The RevOBD app (Swift, macOS/iPadOS) and omatach (Rust, Linux) are the two consumers today.

## 📊 Performance Benchmarks

Measured 2026-08-11 on Apple Silicon (M-series) with `cargo bench` (criterion, [benches/eval_bench.rs](equation_rs/benches/eval_bench.rs)):

| Operation | Native AST evaluator |
|-----------|----------------------|
| Simple equation, warm eval | 36 ns median |
| Corpus per-equation, warm | p50 57 ns / p95 1.39 µs |
| Cold compile + eval | 441 ns |

For reference, the retired QuickJS backend measured ~606 µs cold / 2–5 µs warm, with ~207 KB of runtime per evaluator. Dropping it also shrank the static library from 22.4 MB to 16.5 MB.

### System Requirements

- **Memory**: per-evaluator footprint is one compiled AST plus stateful-function accumulators
- **Startup**: no runtime initialization — first evaluation compiles the expression (sub-microsecond)
- **Thread Safety**: Concurrent evaluation supported
- **Platforms**: Linux, macOS, Windows, iOS, Android (via cross-compilation)

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Clone repository
git clone https://github.com/greyhorsesoftware/obd_equation_rs.git
cd obd_equation_rs

# Install development dependencies
cargo install cargo-watch
cargo install cargo-tarpaulin  # For coverage

# Run development workflow
cargo watch -x test
```

### Code Style

This project follows standard Rust conventions:

- `rustfmt` for code formatting
- `clippy` for linting
- Comprehensive test coverage required
- Semantic versioning

## 📄 License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## 🙏 Acknowledgments

- **serde / serde_json**: Serialization framework
- **thiserror**: Ergonomic error derives
- **criterion**: Statistics-driven benchmarking (dev-dependency)

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/greyhorsesoftware/obd_equation_rs/issues)
- **Documentation**: [EQUATION_REFERENCE.md](EQUATION_REFERENCE.md) for the dialect, `cargo doc --open` for the API

---

**Integrating into an app?** This library provides a high-performance, memory-safe native evaluator for the Torque equation dialect. Start with [EQUATION_REFERENCE.md](EQUATION_REFERENCE.md) for what the dialect accepts and `src/ffi.rs` for the C surface.
