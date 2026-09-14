//! Foreign Function Interface (FFI) bindings for C language
//!
//! This module provides C-compatible bindings for the obd_equation_rs library,
//! allowing it to be used from C programs and other languages that can interface with C.

use crate::{
    error::{EvaluatorError, Result},
    runtime::ExpressionResult,
    ExpressionEvaluator,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::{c_char, c_double, c_int, c_void},
    ptr, slice,
};

// ============================================================================
// Panic containment + last-error reporting for the FFI boundary
// ============================================================================

thread_local! {
    /// Last error message for the current thread, surfaced by `equation_get_last_error`.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Record a message retrievable via `equation_get_last_error`.
fn set_last_error(msg: String) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(msg).ok();
    });
}

/// Extract a human-readable message from a panic payload.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// Run an FFI body with panic containment. On panic, record the message for
/// `equation_get_last_error` and return `on_panic` (the function's error value)
/// instead of unwinding across the C ABI (which is undefined behavior).
fn ffi_guard<R>(on_panic: R, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            set_last_error(format!(
                "panic in equation FFI: {}",
                panic_message(payload.as_ref())
            ));
            on_panic
        }
    }
}

// Opaque pointer types for C API
/// Opaque pointer to ExpressionEvaluator
pub type ExpressionEvaluatorHandle = *mut c_void;

/// Opaque pointer to CustomFunctionRegistry
pub type CustomFunctionRegistryHandle = *mut c_void;

/// Error codes for C API
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum EquationError {
    Ok = 0,
    NullPointer = 1,
    InvalidUtf8 = 2,
    EvaluationError = 3,
    FunctionRegistrationError = 4,
    MemoryError = 5,
    UnknownError = 6,
}

/// C-compatible variable structure
#[repr(C)]
pub struct VariablePair {
    pub key: *const c_char,
    pub value: c_double,
}

/// C-compatible result structure
#[repr(C)]
pub struct EvaluationResult {
    pub value: c_double,
    pub error: EquationError,
}

impl EvaluationResult {
    /// Error result with the given code (value 0.0).
    fn err(error: EquationError) -> Self {
        Self { value: 0.0, error }
    }
}

/// C-compatible string result (for functions that return strings)
#[repr(C)]
pub struct StringResult {
    pub value: *mut c_char, // Must be freed with equation_string_free
    pub error: EquationError,
}

/// C-compatible unified result that can hold numeric, string, or boolean values
#[repr(C)]
pub struct UnifiedResult {
    pub numeric_value: c_double,
    pub string_value: *mut c_char, // NULL if numeric/bool; must be freed with equation_unified_result_free
    pub result_type: u8,           // 0=numeric, 1=string, 2=bool
    pub error: EquationError,
}

impl UnifiedResult {
    /// Error result with the given code (numeric 0.0, no string).
    fn err(error: EquationError) -> Self {
        Self {
            numeric_value: 0.0,
            string_value: ptr::null_mut(),
            result_type: 0,
            error,
        }
    }
}

/// Convert Rust Result to C-compatible result
fn result_to_evaluation_result(result: Result<f64>) -> EvaluationResult {
    match result {
        Ok(value) => EvaluationResult {
            value,
            error: EquationError::Ok,
        },
        Err(_) => EvaluationResult::err(EquationError::EvaluationError),
    }
}

/// Convert C string to Rust string
fn c_str_to_string(c_str: *const c_char) -> Result<String> {
    if c_str.is_null() {
        return Err(EvaluatorError::ParseError(
            "Null string pointer".to_string(),
        ));
    }
    unsafe {
        CStr::from_ptr(c_str)
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| EvaluatorError::ParseError("Invalid UTF-8 string".to_string()))
    }
}

/// Convert C variable array to Rust HashMap.
///
/// Returns the C ABI error code directly so callers surface it accurately:
/// a null variables array with a non-zero count is `NullPointer` (RS7 fix —
/// it was previously flattened to `InvalidUtf8`); a key that fails string
/// conversion stays `InvalidUtf8`.
fn c_variables_to_hashmap(
    vars: *const VariablePair,
    count: c_int,
) -> std::result::Result<HashMap<String, f64>, EquationError> {
    if vars.is_null() && count > 0 {
        return Err(EquationError::NullPointer);
    }

    let mut map = HashMap::new();
    if count > 0 {
        let vars_slice = unsafe { slice::from_raw_parts(vars, count as usize) };
        for var in vars_slice {
            let key = c_str_to_string(var.key).map_err(|_| EquationError::InvalidUtf8)?;
            map.insert(key, var.value);
        }
    }
    Ok(map)
}

/// Convert byte array to Vec<u8> (owned)
fn c_bytes_to_vec(bytes: *const u8, len: c_int) -> Result<Vec<u8>> {
    if bytes.is_null() && len > 0 {
        return Err(EvaluatorError::ParseError(
            "Null bytes array with non-zero length".to_string(),
        ));
    }
    if len < 0 {
        return Err(EvaluatorError::ParseError(
            "Negative byte array length".to_string(),
        ));
    }
    if len == 0 {
        return Ok(Vec::new());
    }
    unsafe { Ok(slice::from_raw_parts(bytes, len as usize).to_vec()) }
}

/// Unmarshalled inputs shared by the evaluate exports:
/// (evaluator, expression, variables, bytes).
type UnmarshalledInputs<'a> = (
    &'a mut ExpressionEvaluator,
    String,
    HashMap<String, f64>,
    Vec<u8>,
);

/// Shared unmarshalling preamble for the evaluate exports: null-check the
/// handle, deref the evaluator, and convert expression / variables / bytes.
/// Callers without a bytes argument pass (null, 0), which yields an empty Vec.
/// On failure, returns the error code the export should surface.
///
/// # Safety
/// Same contract as the evaluate exports: `handle` must come from
/// `equation_evaluator_new()`, pointers must be valid for their counts.
unsafe fn unmarshal_common<'a>(
    handle: ExpressionEvaluatorHandle,
    expression: *const c_char,
    variables: *const VariablePair,
    variable_count: c_int,
    bytes: *const u8,
    byte_count: c_int,
) -> std::result::Result<UnmarshalledInputs<'a>, EquationError> {
    if handle.is_null() {
        return Err(EquationError::NullPointer);
    }
    let evaluator = unsafe { &mut *(handle as *mut ExpressionEvaluator) };
    let expression_str = c_str_to_string(expression).map_err(|_| EquationError::InvalidUtf8)?;
    let variables_map = c_variables_to_hashmap(variables, variable_count)?;
    let bytes_vec = c_bytes_to_vec(bytes, byte_count).map_err(|_| EquationError::InvalidUtf8)?;
    Ok((evaluator, expression_str, variables_map, bytes_vec))
}

// ============================================================================
// C API Functions
// ============================================================================

/// Create a new ExpressionEvaluator instance
///
/// Returns a handle to the evaluator, or null on error.
/// The caller is responsible for freeing the evaluator with equation_evaluator_free().
#[no_mangle]
pub extern "C" fn equation_evaluator_new() -> ExpressionEvaluatorHandle {
    ffi_guard(ptr::null_mut(), || match ExpressionEvaluator::new() {
        Ok(evaluator) => Box::into_raw(Box::new(evaluator)) as ExpressionEvaluatorHandle,
        Err(_) => ptr::null_mut(),
    })
}

/// Free an ExpressionEvaluator instance
///
/// # Safety
/// - handle must be a valid pointer returned from equation_evaluator_new()
/// - handle must not be used after calling this function
#[no_mangle]
pub unsafe extern "C" fn equation_evaluator_free(handle: ExpressionEvaluatorHandle) {
    ffi_guard((), || {
        if !handle.is_null() {
            let _ = unsafe { Box::from_raw(handle as *mut ExpressionEvaluator) };
        }
    })
}

/// Evaluate an expression with variables
///
/// # Safety
/// - handle must be a valid pointer from equation_evaluator_new()
/// - expression must be a valid null-terminated C string
/// - variables can be null if variable_count is 0
/// - variable keys must be valid null-terminated C strings
#[no_mangle]
pub unsafe extern "C" fn equation_evaluator_evaluate(
    handle: ExpressionEvaluatorHandle,
    expression: *const c_char,
    variables: *const VariablePair,
    variable_count: c_int,
) -> EvaluationResult {
    ffi_guard(EvaluationResult::err(EquationError::UnknownError), || {
        let (evaluator, expression_str, variables_map, _bytes) = match unsafe {
            unmarshal_common(
                handle,
                expression,
                variables,
                variable_count,
                ptr::null(),
                0,
            )
        } {
            Ok(parts) => parts,
            Err(e) => return EvaluationResult::err(e),
        };

        result_to_evaluation_result(evaluator.evaluate(&expression_str, &variables_map))
    })
}

/// Evaluate an expression with raw bytes and variables
///
/// # Safety
/// - handle must be a valid pointer from equation_evaluator_new()
/// - expression must be a valid null-terminated C string
/// - bytes can be null if byte_count is 0
/// - variables can be null if variable_count is 0
/// - variable keys must be valid null-terminated C strings
#[no_mangle]
pub unsafe extern "C" fn equation_evaluator_evaluate_with_bytes(
    handle: ExpressionEvaluatorHandle,
    expression: *const c_char,
    variables: *const VariablePair,
    variable_count: c_int,
    bytes: *const u8,
    byte_count: c_int,
) -> EvaluationResult {
    ffi_guard(EvaluationResult::err(EquationError::UnknownError), || {
        let (evaluator, expression_str, variables_map, bytes_vec) = match unsafe {
            unmarshal_common(
                handle,
                expression,
                variables,
                variable_count,
                bytes,
                byte_count,
            )
        } {
            Ok(parts) => parts,
            Err(e) => return EvaluationResult::err(e),
        };

        result_to_evaluation_result(evaluator.evaluate_with_bytes(
            &expression_str,
            &variables_map,
            &bytes_vec,
        ))
    })
}

/// Free a string returned by the FFI API
///
/// # Safety
/// - str must be a pointer returned from a function that allocates strings (currently none, but kept for future use)
#[no_mangle]
pub unsafe extern "C" fn equation_string_free(_str: *mut c_char) {
    ffi_guard((), || {
        if !_str.is_null() {
            let _ = unsafe { CString::from_raw(_str) };
        }
    })
}

/// Convert ExpressionResult to C-compatible UnifiedResult
fn expression_result_to_unified(result: Result<ExpressionResult>) -> UnifiedResult {
    match result {
        Ok(ExpressionResult::Numeric(v)) => UnifiedResult {
            numeric_value: v,
            string_value: ptr::null_mut(),
            result_type: 0,
            error: EquationError::Ok,
        },
        Ok(ExpressionResult::Text(s)) => {
            let c_str = CString::new(s).unwrap_or_default();
            UnifiedResult {
                numeric_value: 0.0,
                string_value: c_str.into_raw(),
                result_type: 1,
                error: EquationError::Ok,
            }
        }
        Ok(ExpressionResult::Bool(b)) => UnifiedResult {
            numeric_value: if b { 1.0 } else { 0.0 },
            string_value: ptr::null_mut(),
            result_type: 2,
            error: EquationError::Ok,
        },
        Err(_) => UnifiedResult::err(EquationError::EvaluationError),
    }
}

/// Evaluate an expression with bytes and return a unified result (numeric, string, or bool)
///
/// # Safety
/// - handle must be a valid pointer from equation_evaluator_new()
/// - expression must be a valid null-terminated C string
/// - variables/bytes/pid_values can be null if their counts are 0
#[no_mangle]
pub unsafe extern "C" fn equation_evaluator_evaluate_unified(
    handle: ExpressionEvaluatorHandle,
    expression: *const c_char,
    variables: *const VariablePair,
    variable_count: c_int,
    bytes: *const u8,
    byte_count: c_int,
    pid_values: *const VariablePair,
    pid_count: c_int,
) -> UnifiedResult {
    ffi_guard(UnifiedResult::err(EquationError::UnknownError), || {
        let (evaluator, expression_str, variables_map, bytes_vec) = match unsafe {
            unmarshal_common(
                handle,
                expression,
                variables,
                variable_count,
                bytes,
                byte_count,
            )
        } {
            Ok(parts) => parts,
            Err(e) => return UnifiedResult::err(e),
        };

        // If PID values provided, use the PID-aware evaluation path
        if pid_count > 0 && !pid_values.is_null() {
            let pid_map = match c_variables_to_hashmap(pid_values, pid_count) {
                Ok(m) => m,
                Err(e) => return UnifiedResult::err(e),
            };
            expression_result_to_unified(evaluator.evaluate_with_bytes_and_pids_unified(
                &expression_str,
                &variables_map,
                &bytes_vec,
                &pid_map,
            ))
        } else {
            expression_result_to_unified(evaluator.evaluate_with_bytes_unified(
                &expression_str,
                &variables_map,
                &bytes_vec,
            ))
        }
    })
}

/// Free the string inside a UnifiedResult
///
/// # Safety
/// - string_value must be a pointer returned from equation_evaluator_evaluate_unified, or null
#[no_mangle]
pub unsafe extern "C" fn equation_unified_result_free_string(string_value: *mut c_char) {
    ffi_guard((), || {
        if !string_value.is_null() {
            let _ = unsafe { CString::from_raw(string_value) };
        }
    })
}

/// Get the last error message (for debugging)
///
/// Returns the last error recorded on the calling thread (e.g. a contained panic
/// message), or a generic string if none. The pointer is valid until the next FFI
/// call on the same thread; callers should copy it immediately.
#[no_mangle]
pub extern "C" fn equation_get_last_error() -> *const c_char {
    LAST_ERROR.with(|e| match &*e.borrow() {
        Some(cstr) => cstr.as_ptr(),
        None => c"No error".as_ptr(),
    })
}

/// Library version — single source of truth is Cargo.toml `version`,
/// embedded at compile time. Returns a static NUL-terminated string
/// (e.g. "1.0.0"); do not free.
#[no_mangle]
pub extern "C" fn equation_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Generate an equation string from PID parameters.
///
/// Returns a StringResult with the equation string or an error.
/// The returned string must be freed with `equation_string_free`.
///
/// # Parameters
/// * `scale` - Multiplier (e.g., 0.25 for RPM)
/// * `offset` - Additive offset (e.g., -40.0 for temperature)
/// * `start_byte` - First byte index (0 = A, 1 = B, etc.)
/// * `end_byte` - Last byte index (inclusive)
/// * `signed` - Whether the value uses two's complement
#[no_mangle]
pub extern "C" fn equation_generate(
    scale: c_double,
    offset: c_double,
    start_byte: c_int,
    end_byte: c_int,
    signed: bool,
) -> StringResult {
    ffi_guard(
        StringResult {
            value: ptr::null_mut(),
            error: EquationError::UnknownError,
        },
        || match crate::generator::generate_equation(scale, offset, start_byte, end_byte, signed) {
            Some(eq) => match CString::new(eq) {
                Ok(cstr) => StringResult {
                    value: cstr.into_raw(),
                    error: EquationError::Ok,
                },
                Err(_) => StringResult {
                    value: ptr::null_mut(),
                    error: EquationError::MemoryError,
                },
            },
            None => StringResult {
                value: ptr::null_mut(),
                error: EquationError::EvaluationError,
            },
        },
    )
}

/// Test-only entry point that panics inside the FFI guard, to prove the guard
/// contains the unwind at the C boundary and records the message.
#[cfg(test)]
extern "C" fn equation_test_panic() -> EquationError {
    ffi_guard(EquationError::UnknownError, || {
        panic!("deliberate test panic");
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_ffi_guard_contains_panic() {
        // A panic on the C-boundary path must return the error value, not unwind.
        let err = equation_test_panic();
        assert_eq!(err as i32, EquationError::UnknownError as i32);

        // ...and the message must be retrievable via the last-error mechanism.
        let msg = unsafe {
            CStr::from_ptr(equation_get_last_error())
                .to_str()
                .unwrap()
                .to_string()
        };
        assert!(msg.contains("deliberate test panic"), "msg: {msg}");
    }

    #[test]
    fn test_basic_ffi_evaluation() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            let expression = CString::new("2 + 3").unwrap();
            let result =
                equation_evaluator_evaluate(evaluator, expression.as_ptr(), ptr::null(), 0);

            assert_eq!(result.error as i32, EquationError::Ok as i32);
            assert_eq!(result.value, 5.0);

            equation_evaluator_free(evaluator);
        }
    }

    #[test]
    fn test_ffi_with_variables() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            let expression = CString::new("x + y").unwrap();
            let var_x = CString::new("x").unwrap();
            let var_y = CString::new("y").unwrap();

            let variables = vec![
                VariablePair {
                    key: var_x.as_ptr(),
                    value: 10.0,
                },
                VariablePair {
                    key: var_y.as_ptr(),
                    value: 20.0,
                },
            ];

            let result = equation_evaluator_evaluate(
                evaluator,
                expression.as_ptr(),
                variables.as_ptr(),
                variables.len() as c_int,
            );

            assert_eq!(result.error as i32, EquationError::Ok as i32);
            assert_eq!(result.value, 30.0);

            equation_evaluator_free(evaluator);
        }
    }
}
