//! Integration tests for the FFI interface
//!
//! This file tests the C FFI functions directly from Rust to ensure
//! the foreign function interface works correctly.

use std::ffi::CString;
use std::os::raw::c_int;

// Re-export the FFI types and functions for testing
use obd_equation_rs::ffi::{
    equation_evaluator_evaluate, equation_evaluator_evaluate_with_bytes, equation_evaluator_free,
    equation_evaluator_new, VariablePair,
};

// Error constants (matching C enum values)
const EQUATION_ERROR_OK: u32 = 0;
const EQUATION_ERROR_NULL_POINTER: u32 = 1;
const EQUATION_ERROR_INVALID_UTF8: u32 = 2;
const EQUATION_ERROR_EVALUATION_ERROR: u32 = 3;
const _EQUATION_ERROR_FUNCTION_REGISTRATION_ERROR: u32 = 4;
const _EQUATION_ERROR_MEMORY_ERROR: u32 = 5;
const _EQUATION_ERROR_UNKNOWN_ERROR: u32 = 6;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test basic arithmetic evaluation
    #[test]
    fn test_basic_arithmetic() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            let expression = CString::new("2 + 3").unwrap();
            let result =
                equation_evaluator_evaluate(evaluator, expression.as_ptr(), std::ptr::null(), 0);

            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 5.0);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test evaluation with variables
    #[test]
    fn test_variables() {
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

            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 30.0);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test OBD-II style byte operations
    #[test]
    fn test_byte_operations() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Sample OBD-II data (example: engine RPM calculation)
            // RPM = ((A*256)+B)/4 where A and B are bytes
            let bytes: Vec<u8> = vec![0x1A, 0x0F]; // A=26 (0x1A), B=15 (0x0F)

            let result = equation_evaluator_evaluate_with_bytes(
                evaluator,
                CString::new("((A*256)+B)/4").unwrap().as_ptr(),
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as c_int,
            );

            assert_eq!(result.error as u32, EQUATION_ERROR_OK);

            // Expected: ((26*256)+15)/4 = (6656+15)/4 = 6671/4 = 1667.75
            assert_eq!(result.value, 1667.75);

            // Test bit operations
            let result = equation_evaluator_evaluate_with_bytes(
                evaluator,
                CString::new("BIT(A:0)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as c_int,
            );

            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 0.0);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test mathematical functions
    #[test]
    fn test_math_functions() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Test trigonometric functions
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("SIN(0)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert!((result.value - 0.0).abs() < 0.001);

            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("COS(0)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert!((result.value - 1.0).abs() < 0.001);

            // Test MIN/MAX functions
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("MAX(10:20:5)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 20.0);

            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("MIN(10:20:5)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 5.0);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test lookup functions (supports both : and = syntax)
    #[test]
    fn test_lookup_functions() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Test LOOKUP function with = syntax - exact key match
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("LOOKUP(2:0:1=100:2=200:3=300)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 200.0); // Exact match for key 2

            // Test LOOKUP function with = syntax - no match, return default
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("LOOKUP(99:42:1=100:2=200:3=300)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 42.0); // No match, return default

            // Empty-default LOOKUP form `A::` (no default), e.g. the MG ZS EV
            // HVAC pids `LOOKUP(A::1='Fresh':0='Recirc')`. `::` used to emit
            // `,,` (elided arg → parse error); now `,"",`. Must parse + match.
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("LOOKUP(2::1=100:2=200:3=300)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(
                result.error as u32, EQUATION_ERROR_OK,
                "A:: form must parse"
            );
            assert_eq!(result.value, 200.0);

            // Whitespace-default form `: :` (Cavalier Trans Range/Display).
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("LOOKUP(2: :1=100:2=200:3=300)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(
                result.error as u32, EQUATION_ERROR_OK,
                "`: :` form must parse"
            );
            assert_eq!(result.value, 200.0);

            // Title-cased function names (Torque: Signed, Lookup, Bit).
            for eq in ["Signed(200)", "SIGNED(200)", "signed(200)"] {
                let r = equation_evaluator_evaluate(
                    evaluator,
                    CString::new(eq).unwrap().as_ptr(),
                    std::ptr::null(),
                    0,
                );
                assert_eq!(r.error as u32, EQUATION_ERROR_OK, "{eq} must parse");
                assert_eq!(r.value, -56.0, "{eq} == signed8(200)");
            }
            // Title-cased LOOKUP with nested Title-cased BIT + empty default
            // (numeric mappings so the FFI returns a value). BIT(255,2)=1.
            let r = equation_evaluator_evaluate(
                evaluator,
                CString::new("Lookup(Bit(255:2)::0=100:1=200)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(
                r.error as u32, EQUATION_ERROR_OK,
                "Lookup(Bit(..)::..) must parse"
            );
            assert_eq!(r.value, 200.0, "BIT(255,2)=1 → key 1 = 200");

            // Test CLOSEST function with = syntax - find closest match
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("CLOSEST(150:0:1=100:255=200)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 200.0); // 150 is closer to 255 than to 1

            // Test LOOKUP with colon syntax (still supported)
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("LOOKUP(2:0:1:100:2:200:3:300)")
                    .unwrap()
                    .as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 200.0); // Exact match for key 2

            equation_evaluator_free(evaluator);
        }
    }

    /// Test data conversion functions (common in OBD-II)
    #[test]
    fn test_data_conversions() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Test with sample byte data
            let bytes: Vec<u8> = vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

            // Test INT16 (big-endian 16-bit signed integer)
            let result = equation_evaluator_evaluate_with_bytes(
                evaluator,
                CString::new("INT16(A:B)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as c_int,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);

            // Test INT32
            let result = equation_evaluator_evaluate_with_bytes(
                evaluator,
                CString::new("INT32(A:B:C:D)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as c_int,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);

            // Test SIGNED (8-bit signed)
            let result = equation_evaluator_evaluate_with_bytes(
                evaluator,
                CString::new("SIGNED(A)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as c_int,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test colon-separated parameters (legacy OBD-II syntax)
    #[test]
    fn test_colon_parameters() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Test MIN with colon parameters
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("MIN(10:20:5)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 5.0);

            // Test MAX with colon parameters
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("MAX(1:5:3:9:2)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 9.0);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test error handling
    #[test]
    fn test_error_handling() {
        unsafe {
            // Test null handle
            let result = equation_evaluator_evaluate(
                std::ptr::null_mut(),
                CString::new("1+1").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_NULL_POINTER);

            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Test invalid expression
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("INVALID_SYNTAX+++").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_EVALUATION_ERROR);

            // Test null expression
            let result =
                equation_evaluator_evaluate(evaluator, std::ptr::null(), std::ptr::null(), 0);
            assert_eq!(result.error as u32, EQUATION_ERROR_INVALID_UTF8);

            equation_evaluator_free(evaluator);
        }
    }

    /// Test Cross-PID references via FFI
    ///
    /// NOTE: PID values are now provided via platform callbacks, not as direct parameters.
    /// The FFI layer needs to be extended to support custom platform callbacks.
    /// For now, this test is disabled. See comprehensive_tests.rs for the Rust-level test.
    #[test]
    #[ignore]
    fn test_cross_pid_references() {
        // TODO: Implement FFI support for platform callbacks
        // This would require:
        // 1. C function pointer type for get_pid_value callback
        // 2. equation_evaluator_new_with_callbacks() FFI function
        // 3. Wrapper struct that implements PlatformCallbacks and calls C callbacks
    }

    /// Test INT function via FFI
    #[test]
    fn test_int_function() {
        unsafe {
            let evaluator = equation_evaluator_new();
            assert!(!evaluator.is_null());

            // Test INT function
            let result = equation_evaluator_evaluate(
                evaluator,
                CString::new("INT(3.7)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 3.0);

            // Test INT with byte variables (from Swift tests)
            let bytes: Vec<u8> = vec![100, 0, 0, 0];
            let result = equation_evaluator_evaluate_with_bytes(
                evaluator,
                CString::new("INT(A/30)").unwrap().as_ptr(),
                std::ptr::null(),
                0,
                bytes.as_ptr(),
                bytes.len() as c_int,
            );
            assert_eq!(result.error as u32, EQUATION_ERROR_OK);
            assert_eq!(result.value, 3.0); // 100/30 = 3.333... -> 3

            equation_evaluator_free(evaluator);
        }
    }
}

// D9: VAL{} pid references — Torque writes uppercase VAL{...}; the substitution
// regex must be case-insensitive, and provided pid values must flow through.
#[test]
fn test_val_refs_case_insensitive_with_map() {
    use std::collections::HashMap;
    let mut evaluator = obd_equation_rs::ExpressionEvaluator::new().unwrap();
    let vars = HashMap::new();
    let mut pids = HashMap::new();
    pids.insert("HV_Volts".to_string(), 400.0);
    pids.insert("HV_Current".to_string(), 10.0);
    let r = evaluator
        .evaluate_with_bytes_and_pids_unified(
            "VAL{HV_Volts}*VAL{HV_Current}/1000",
            &vars,
            &[],
            &pids,
        )
        .unwrap();
    assert_eq!(r.as_f64().unwrap(), 4.0);
    // Mixed case + missing ref defaults to 0 (startup gate lives in Swift).
    let r = evaluator
        .evaluate_with_bytes_and_pids_unified("Val{HV_Volts} + val{Missing}", &vars, &[], &pids)
        .unwrap();
    assert_eq!(r.as_f64().unwrap(), 400.0);
}
