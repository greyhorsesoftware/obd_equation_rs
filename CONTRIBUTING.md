# Contributing to obd_equation_rs

Thanks for looking. Small, focused pull requests are easiest to review.

## Before you open a PR

```
cd equation_rs
cargo test            # unit + corpus + FFI tests
cargo doc --no-deps   # must build without warnings
```

- **The corpus is the oracle.** `tests/corpus_snapshot.rs` pins the evaluator's
  output for every equation in `tests/corpus/equations.txt` across an input
  grid. A behaviour change that alters `expected_results.txt` must say why in
  the PR; regenerate it only deliberately (see the test's header).
- **The dialect is Torque's.** New functions or syntax should match how the
  Torque app evaluates them (`EQUATION_REFERENCE.md` documents what we accept).
  If Torque's behaviour is unknown for a case, say so and pick the least
  surprising reading.
- **Cross-platform results must be bit-identical.** Transcendentals go through
  `libm`, not the OS math library, so the corpus holds on every platform. Keep
  it that way.
- **FFI changes** need matching updates in `cbindgen.toml` and the consumers
  that link the C header. Say in the PR that the header changed.
- No captured vehicle data in tests. Use synthetic bytes.

## Toolchain

`rust-toolchain.toml` pins the compiler because the produced static libraries
are linked together with `obd_session_rs` into one binary; both crates must
share a toolchain. Bump it in both repos at once.

## License

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`),
without any additional terms or conditions.
