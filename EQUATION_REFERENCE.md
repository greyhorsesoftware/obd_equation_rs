# OBD Equation Reference

Complete reference for all equation syntax, functions, and features supported by `obd_equation_rs`.

---

## Table of Contents

- [Variables](#variables)
- [Arithmetic Operators](#arithmetic-operators)
- [Colon Parameter Syntax](#colon-parameter-syntax)
- [Control Flow](#control-flow)
- [Math Functions](#math-functions)
- [Bit Manipulation Functions](#bit-manipulation-functions)
- [Data Conversion Functions](#data-conversion-functions)
- [Lookup Functions](#lookup-functions)
- [Stateful Functions](#stateful-functions)
- [Platform Functions](#platform-functions)
- [Cross-PID References](#cross-pid-references)
- [Equation Generator](#equation-generator)
- [Case Sensitivity](#case-sensitivity)
- [Result Types](#result-types)

---

## Variables

### Byte Variables (A-Z)

When evaluating with raw OBD response bytes, each byte is mapped to a letter variable:

| Byte Index | Variable |
|------------|----------|
| 0          | A (or a) |
| 1          | B (or b) |
| 2          | C (or c) |
| ...        | ...      |
| 25         | Z (or z) |

Both uppercase and lowercase are supported.

```
A              -> value of first byte
B              -> value of second byte
A*256 + B      -> 16-bit value from bytes 0-1
```

### Custom Variables

Named variables can be passed as key-value pairs:

```
x + y          -> sum of variables x and y
scaling * A    -> byte A multiplied by a custom variable
```

---

## Arithmetic Operators

Standard JavaScript arithmetic operators are supported:

| Operator | Description    | Example       |
|----------|----------------|---------------|
| `+`      | Addition       | `A + 40`      |
| `-`      | Subtraction    | `A - 40`      |
| `*`      | Multiplication | `A * 0.25`    |
| `/`      | Division       | `A / 4`       |
| `%`      | Modulo         | `A % 16`      |
| `()`     | Grouping       | `(A + B) * 2` |

Operator precedence follows standard JavaScript rules. Division by zero returns `Infinity`.

---

## Colon Parameter Syntax

Function arguments can use colons `:` as separators instead of commas. This is common in OBD equation definitions.

```
BIT(A:3)            -> equivalent to BIT(A, 3)
MIN(A:B)            -> equivalent to MIN(A, B)
MAX(A:B:C)          -> equivalent to MAX(A, B, C)
BITVALUE(A:0:3)     -> equivalent to BITVALUE(A, 0, 3)
```

Colons inside `LOOKUP` and `CLOSEST` functions separate key-value pair arguments (see [Lookup Functions](#lookup-functions)).

---

## Control Flow

### IF Function

```
IF(condition, thenValue, elseValue)
```

Returns `thenValue` if condition is truthy, otherwise `elseValue`.

```
IF(A > 128, A - 256, A)
IF(BIT(A,0) == 1, 'On', 'Off')
```

### IF...THEN...ELSE Syntax

Natural language syntax, transformed to `IF()` calls before evaluation:

```
IF A > 0 THEN 1 ELSE 0
IF BIT(A:0) == 1 THEN 'Yes' ELSE 'No'
IF A == 1 THEN 'No Fault' ELSE 'Fault'
```

Nested IF is supported:

```
IF B == 0 THEN 'OFF' ELSE IF B == 254 THEN 'ERROR' ELSE B
```

This transforms to: `IF(B == 0, 'OFF', IF(B == 254, 'ERROR', B))`

### Ternary Operator

C-style ternary `condition ? trueExpr : falseExpr` is supported:

```
A > 128 ? A - 256 : A
(A >= 128) ? (A - 256) : A
```

Nested ternaries work (right-associative):

```
A ? B : C ? D : E       -> IF(A, B, IF(C, D, E))
A ? B ? C : D : E       -> IF(A, IF(B, C, D), E)
```

Ternaries inside parenthesized sub-expressions are also handled:

```
(1.0 * ((B >= 128) ? (B - 256) : B) + -40.0)
```

### ZEROREF Function

```
ZEROREF(condition, trueValue, falseValue)
```

Equivalent to `IF` -- returns `trueValue` if condition is truthy, otherwise `falseValue`.

---

## Math Functions

| Function     | Description              | Example          |
|--------------|--------------------------|------------------|
| `MIN(a, b, ...)` | Minimum of arguments | `MIN(A, B)`      |
| `MAX(a, b, ...)` | Maximum of arguments | `MAX(A, B, C)`   |
| `ABS(x)`     | Absolute value           | `ABS(A - 128)`   |
| `SQRT(x)`    | Square root              | `SQRT(A)`        |
| `POW(b, e)`  | Power (b^e)              | `POW(2, 8)`      |
| `SIN(x)`     | Sine (radians)           | `SIN(A)`         |
| `COS(x)`     | Cosine (radians)         | `COS(A)`         |
| `TAN(x)`     | Tangent (radians)        | `TAN(A)`         |
| `LOG(x)`     | Natural logarithm        | `LOG(A)`         |
| `LOG10(x)`   | Base-10 logarithm        | `LOG10(A)`       |
| `EXP(x)`     | e^x                      | `EXP(A)`         |
| `CEIL(x)`    | Round up                 | `CEIL(3.2)` -> 4 |
| `FLOOR(x)`   | Round down               | `FLOOR(3.7)` -> 3|
| `ROUND(x)`   | Round to nearest integer | `ROUND(3.5)` -> 4|
| `INT(x)`     | Truncate to integer (same as FLOOR) | `INT(A/30)` |

---

## Bit Manipulation Functions

### BIT(value, bitIndex)

Extract a single bit. Returns 1 if the bit is set, 0 otherwise.

```
BIT(A, 0)     -> least significant bit of A
BIT(A, 7)     -> most significant bit of A (for 8-bit)
BIT(A:3)      -> bit 3 of A (colon syntax)
```

### BITVALUE(value, startBit, endBit)

Extract a range of bits as an unsigned integer. If `endBit < startBit`, they are swapped automatically.

```
BITVALUE(A, 0, 3)   -> lower nibble of A
BITVALUE(A, 4, 7)   -> upper nibble of A
BITVALUE(A:0:3)     -> same, colon syntax
```

### BITSELECT(value, startBit, endBit, option0, option1, ...)

Extract a bit range and return the option corresponding to the highest set bit index.

```
BITSELECT(A, 0, 2, 'OL', 'CL', 'OL-Fault')
BITSELECT(A:0:2:'OL':'CL':'OL-Fault')
```

Also supports `"start~end"` range string syntax as the second argument:

```
BITSELECT(A, '0~2', 'OL', 'CL', 'OL-Fault')
```

### IFANY(value, startBit, endBit, label0, label1, ...)

Check each bit in the range. Returns a comma-separated string of labels for all set bits.

```
IFANY(A, 0, 2, 'Fault1', 'Fault2', 'Fault3')
IFANY(5:0:2:'A':'B':'C')    -> "A, C" (bits 0 and 2 set in 5=0b101)
```

Returns an empty string if no bits are set.

---

## Data Conversion Functions

### Signed Integer Conversion

| Function       | Description                        | Example               |
|----------------|------------------------------------|-----------------------|
| `SIGNED(x)`    | 8-bit unsigned to signed (-128..127)  | `SIGNED(200)` -> -56 |
| `SIGNED8(x)`   | Alias for SIGNED                   | `SIGNED8(A)`          |
| `SIGNED16(x)`  | 16-bit unsigned to signed (-32768..32767) | `SIGNED16(A*256+B)` |

### Multi-Byte Integer Assembly

| Function            | Description                     | Example                  |
|---------------------|---------------------------------|--------------------------|
| `INT16(A, B)`       | 16-bit big-endian integer       | `INT16(A, B)` = A*256+B |
| `INT24(A, B, C)`    | 24-bit big-endian integer       | `INT24(A, B, C)`         |
| `INT32(A, B, C, D)` | 32-bit big-endian integer       | `INT32(A, B, C, D)`      |

### IEEE 754 Floating Point

| Function                     | Description                    |
|------------------------------|--------------------------------|
| `FLOAT32(A, B, C, D)`       | Decode 32-bit float from 4 bytes (big-endian) |
| `FLOAT64(A, B, C, D, E, F, G, H)` | Decode 64-bit float from 8 bytes (big-endian) |

### ASCII Conversion

```
ascii(65)              -> "A"
ascii(72, 101, 108)    -> "Hel"
```

Converts numeric byte values to their ASCII character representations.

---

## Lookup Functions

### LOOKUP(value, default, key=value, ...)

Table lookup with exact key matching. Returns the value associated with the matching key, or the default.

**Syntax variants:**

```
LOOKUP(A, 0, 1=100, 2=200, 3=300)           -> key=value format
LOOKUP(A, 0, 1, 100, 2, 200, 3, 300)        -> key, value pairs
LOOKUP(A:0:1=100:2=200:3=300)               -> colon-separated
```

**Range syntax with tilde:**

```
LOOKUP(A, 0, 0~3='Low', 4~7='Medium', 8~15='High')
```

The `~` creates an inclusive range: `0~3` matches values 0, 1, 2, and 3.

**Special default values:**

- `'value'` as default returns the input value itself when no key matches

**String values:**

```
LOOKUP(A:0:0='Off':1='On':2='Error')
```

### CLOSEST(value, default, key=value, ...)

Same syntax as LOOKUP, but finds the key with the smallest absolute difference from the input value.

```
CLOSEST(150, 0, 100=1, 200=2, 250=3)   -> 2 (200 is closest to 150)
CLOSEST(A:0:100='Low':200='Med':300='High')
```

---

## Stateful Functions

These functions maintain internal state across multiple evaluations on the same `ExpressionEvaluator` instance. State persists between calls but resets when creating a new evaluator.

### EWMAF(weight, value)

Exponentially Weighted Moving Average Filter.

- `weight`: smoothing factor between 0.0 and 1.0
- Higher weight = more responsive to new values
- Formula: `result = weight * value + (1 - weight) * previous`

```
EWMAF(0.5, A)    -> 50% blend of current and previous
EWMAF(0.1, A)    -> slow-moving average (10% new, 90% old)
```

### TAVG(seconds, value)

Time-weighted exponential moving average.

- `seconds`: time constant (tau) in seconds
- Uses real elapsed time between evaluations
- Formula: `alpha = 1 - e^(-dt/tau)`

```
TAVG(1, A)     -> 1-second time constant
TAVG(10, A)    -> 10-second time constant (slower response)
```

### RAVG(value)

Running average over all values ever passed.

```
RAVG(A)    -> average of all A values seen so far
```

### AVG(bucketSize, value)

Sliding window average of the most recent N values.

```
AVG(10, A)    -> average of last 10 values of A
AVG(50, A)    -> average of last 50 values of A
```

### TDLY(delay, value)

Time-based delay. Holds the initial value for `delay` seconds, then passes through the current value.

```
TDLY(5, A)    -> holds first value for 5 seconds, then shows live A
```

### RDLY(samples, value)

Sample-count delay. Returns the value from N samples ago.

```
RDLY(2, A)    -> returns what A was 2 evaluations ago
RDLY(10, A)   -> 10-sample delay
```

### TOT(window, value)

Time On Target. Accumulates seconds where `value` is non-zero.

```
TOT(0, A)     -> total seconds that A has been non-zero
```

---

## Platform Functions

### BARO()

Returns the current barometric pressure as set by the platform callbacks.

```
BARO()    -> barometric pressure in PSI (0.0 if no sensor)
```

On iOS, this reads from CMAltimeter (hPa converted to PSI). On other platforms, returns the value provided by the platform callback implementation.

---

## Cross-PID References

Reference the current value of another PID using `val{PID_NAME}` syntax:

```
val{Engine RPM}
val{HVB Current Low Range} * val{HVB Voltage} * 0.001
```

- PID names are looked up via the platform callback's `get_pid_value()` method
- Unknown PIDs resolve to `0`
- Can also be resolved from a `HashMap<String, f64>` passed to `evaluate_with_bytes_and_pids_unified()`

---

## Equation Generator

The library includes a generator that produces equation strings from PID parameters:

```rust
generate_equation(scale, offset, start_byte, end_byte, signed) -> Option<String>
```

| Parameter    | Description                                    |
|-------------|------------------------------------------------|
| `scale`     | Multiplier (e.g., 0.25 for RPM)               |
| `offset`    | Additive offset (e.g., -40 for temperature)    |
| `start_byte`| First byte index (0-based: 0=A, 1=B, ...)     |
| `end_byte`  | Last byte index (inclusive)                     |
| `signed`    | Whether value uses two's complement             |

**Generated equation examples:**

| PID           | Parameters                          | Generated Equation              |
|---------------|-------------------------------------|---------------------------------|
| Vehicle Speed | scale=1, offset=0, bytes=0..0       | `A`                             |
| Coolant Temp  | scale=1, offset=-40, bytes=0..0     | `A + -40`                       |
| Engine Load   | scale=0.3922, offset=0, bytes=0..0  | `0.3922 * A`                    |
| Engine RPM    | scale=0.25, offset=0, bytes=0..1    | `0.25 * (A*256 + B)`           |
| Timing Advance| scale=0.5, offset=-64, bytes=0..0, signed | `0.5 * (SIGNED(A)) + -64` |
| O2 Current    | scale=0.00390625, offset=0, bytes=2..3, signed | `0.00390625 * (SIGNED16(C*256 + D))` |

For 3+ byte signed values, the generator produces an IF/THEN/ELSE expression using the appropriate bit width thresholds.

---

## Case Sensitivity

All built-in functions are available in both uppercase and lowercase:

```
BIT(A, 0)       -> same as bit(A, 0)
SIGNED(A)       -> same as signed(A)
FLOAT32(A,B,C,D) -> same as float32(A,B,C,D)
LOOKUP(...)     -> same as lookup(...)
```

Variable names (A-Z) are also case-insensitive when using byte variables.

---

## Result Types

Expressions can return three types of results:

| Type    | Description                          | Example                    |
|---------|--------------------------------------|----------------------------|
| Numeric | Floating-point number (f64)          | `A * 256 + B` -> 6904.0   |
| Text    | String value                         | `'No Fault'`               |
| Bool    | Boolean value                        | `A > 128`                  |

- String results that parse as valid numbers are coerced to Numeric
- Bool results coerce to 1.0 (true) or 0.0 (false) when a numeric result is requested
- Null/Undefined JS results coerce to 0.0

Use `evaluate_unified()` or `evaluate_with_bytes_unified()` to receive the typed result without coercion.
