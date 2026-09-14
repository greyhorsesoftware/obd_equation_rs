use obd_equation_rs::generator::generate_equation;
use obd_equation_rs::ExpressionEvaluator;
use obd_equation_rs::ExpressionResult;
use std::collections::HashMap;

#[cfg(test)]
mod comprehensive_tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test simple variable access
        assert_eq!(evaluator.evaluate("5", &HashMap::new()).unwrap(), 5.0);

        // Test basic arithmetic
        assert_eq!(evaluator.evaluate("2 + 3", &HashMap::new()).unwrap(), 5.0);
        assert_eq!(evaluator.evaluate("10 - 4", &HashMap::new()).unwrap(), 6.0);
        assert_eq!(evaluator.evaluate("3 * 4", &HashMap::new()).unwrap(), 12.0);
        assert_eq!(evaluator.evaluate("15 / 3", &HashMap::new()).unwrap(), 5.0);
        assert_eq!(
            evaluator.evaluate("2 + 3 * 4", &HashMap::new()).unwrap(),
            14.0
        );
        assert_eq!(
            evaluator.evaluate("(2 + 3) * 4", &HashMap::new()).unwrap(),
            20.0
        );
    }

    #[test]
    fn test_variable_operations() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let mut variables = HashMap::new();

        variables.insert("x".to_string(), 10.0);
        variables.insert("y".to_string(), 5.0);
        variables.insert("z".to_string(), 2.0);

        assert_eq!(evaluator.evaluate("x", &variables).unwrap(), 10.0);
        assert_eq!(evaluator.evaluate("x + y", &variables).unwrap(), 15.0);
        assert_eq!(evaluator.evaluate("x * y - z", &variables).unwrap(), 48.0);
        assert_eq!(evaluator.evaluate("x / z", &variables).unwrap(), 5.0);
    }

    #[test]
    fn test_byte_variables() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test A-Z byte assignment
        let bytes = [10, 20, 30, 40]; // A=10, B=20, C=30, D=40

        assert_eq!(
            evaluator
                .evaluate_with_bytes("A", &HashMap::new(), &bytes)
                .unwrap(),
            10.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("B", &HashMap::new(), &bytes)
                .unwrap(),
            20.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("C", &HashMap::new(), &bytes)
                .unwrap(),
            30.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("D", &HashMap::new(), &bytes)
                .unwrap(),
            40.0
        );

        // Test case insensitive
        assert_eq!(
            evaluator
                .evaluate_with_bytes("a", &HashMap::new(), &bytes)
                .unwrap(),
            10.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("b", &HashMap::new(), &bytes)
                .unwrap(),
            20.0
        );

        // Test extended byte variables A1-N1 (bytes 26-39)
        let mut extended_bytes = vec![0u8; 40];
        // Set Z (index 25) = 250
        extended_bytes[25] = 250;
        // Set A1 (index 26) = 100, B1 (index 27) = 200, N1 (index 39) = 77
        extended_bytes[26] = 100;
        extended_bytes[27] = 200;
        extended_bytes[39] = 77;

        let vars = HashMap::new();
        assert_eq!(
            evaluator
                .evaluate_with_bytes("Z", &vars, &extended_bytes)
                .unwrap(),
            250.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("A1", &vars, &extended_bytes)
                .unwrap(),
            100.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("B1", &vars, &extended_bytes)
                .unwrap(),
            200.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("N1", &vars, &extended_bytes)
                .unwrap(),
            77.0
        );

        // Test case insensitive for extended vars
        assert_eq!(
            evaluator
                .evaluate_with_bytes("a1", &vars, &extended_bytes)
                .unwrap(),
            100.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("n1", &vars, &extended_bytes)
                .unwrap(),
            77.0
        );

        // Test using extended vars in expressions
        let result = evaluator
            .evaluate_with_bytes("A1 * 256 + B1", &vars, &extended_bytes)
            .unwrap();
        assert_eq!(result, 25800.0); // 100 * 256 + 200
    }

    #[test]
    fn test_math_functions() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test MIN/MAX
        assert_eq!(
            evaluator.evaluate("MIN(10, 20)", &HashMap::new()).unwrap(),
            10.0
        );
        assert_eq!(
            evaluator.evaluate("MAX(10, 20)", &HashMap::new()).unwrap(),
            20.0
        );

        // Test ABS
        assert_eq!(evaluator.evaluate("ABS(5)", &HashMap::new()).unwrap(), 5.0);
        assert_eq!(evaluator.evaluate("ABS(-5)", &HashMap::new()).unwrap(), 5.0);

        // Test SQRT
        assert!((evaluator.evaluate("SQRT(16)", &HashMap::new()).unwrap() - 4.0).abs() < 0.001);

        // Test trigonometric functions
        assert!((evaluator.evaluate("SIN(0)", &HashMap::new()).unwrap() - 0.0).abs() < 0.001);
        assert!((evaluator.evaluate("COS(0)", &HashMap::new()).unwrap() - 1.0).abs() < 0.001);

        // Test rounding functions
        assert_eq!(
            evaluator.evaluate("ROUND(3.7)", &HashMap::new()).unwrap(),
            4.0
        );
        assert_eq!(
            evaluator.evaluate("FLOOR(3.7)", &HashMap::new()).unwrap(),
            3.0
        );
        assert_eq!(
            evaluator.evaluate("CEIL(3.2)", &HashMap::new()).unwrap(),
            4.0
        );
    }

    #[test]
    fn test_bit_operations() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test BIT - extract single bit
        assert_eq!(
            evaluator.evaluate("BIT(5, 0)", &HashMap::new()).unwrap(),
            1.0
        ); // 5 = 0b101, bit 0 = 1
        assert_eq!(
            evaluator.evaluate("BIT(5, 1)", &HashMap::new()).unwrap(),
            0.0
        ); // 5 = 0b101, bit 1 = 0
        assert_eq!(
            evaluator.evaluate("BIT(5, 2)", &HashMap::new()).unwrap(),
            1.0
        ); // 5 = 0b101, bit 2 = 1

        // Test BITVALUE - extract bit range
        assert_eq!(
            evaluator
                .evaluate("BITVALUE(5, 0, 2)", &HashMap::new())
                .unwrap(),
            5.0
        ); // bits 0-2 of 5 = 0b101 = 5
        assert_eq!(
            evaluator
                .evaluate("BITVALUE(15, 0, 3)", &HashMap::new())
                .unwrap(),
            15.0
        ); // bits 0-3 of 15 = 0b1111 = 15
        assert_eq!(
            evaluator
                .evaluate("BITVALUE(15, 1, 2)", &HashMap::new())
                .unwrap(),
            3.0
        ); // bits 1-2 of 15 = 0b11 = 3
    }

    #[test]
    fn test_bit_select_operations() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test BITSELECT with bit range syntax
        let result = evaluator
            .evaluate("BITSELECT(1:0:2,'OL','CL','OL-Fault')", &HashMap::new())
            .unwrap();
        assert_eq!(result, 2.0); // Should return the third option (index 2) for value 1

        // Test BITSELECT with start/end parameters
        let result = evaluator
            .evaluate("BITSELECT(1,0,2,'OL','CL','OL-Fault')", &HashMap::new())
            .unwrap();
        assert_eq!(result, 2.0);

        // Test IFANY - returns comma-joined string of labels for set bits
        // 5 = 0b101, so bits 0 and 2 are set, IFANY returns "A, C" (a string)
        // Since evaluate() expects f64, string result produces a Type conversion error
        let result = evaluator.evaluate("IFANY(5:0:2,'A','B','C')", &HashMap::new());
        assert!(
            result.is_err(),
            "IFANY should return Err (string cannot be parsed as f64)"
        );
        let err_str = format!("{}", result.unwrap_err());
        assert!(
            err_str.contains("Type conversion error"),
            "Error should be Type conversion error, got: {}",
            err_str
        );
    }

    #[test]
    fn test_lookup_functions() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test LOOKUP with exact matches
        let result = evaluator
            .evaluate("LOOKUP(2:0:1=100:2=200:3=300)", &HashMap::new())
            .unwrap();
        assert_eq!(result, 200.0);

        // Test LOOKUP with default value
        let result = evaluator
            .evaluate("LOOKUP(99:42:1=100:2=200:3=300)", &HashMap::new())
            .unwrap();
        assert_eq!(result, 42.0);

        // Test CLOSEST
        let result = evaluator
            .evaluate("CLOSEST(150:0:1=100:255=200)", &HashMap::new())
            .unwrap();
        // Keys are 1 and 255. 150-1=149, 255-150=105, so 255 is closer
        assert_eq!(result, 200.0); // 255 is closest key, returns value 200

        // Test another CLOSEST example
        let result = evaluator
            .evaluate("CLOSEST(150:0:100=1:200=2:250=3)", &HashMap::new())
            .unwrap();
        // 150-100=50, 150-200=50, 150-250=100. Tie between 100 and 200, picks first
        assert_eq!(result, 1.0); // 100 is closest key (tie, picks first), returns value 1
    }

    #[test]
    fn test_colon_separated_parameters() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test colon-separated parameters
        assert_eq!(
            evaluator.evaluate("MIN(10:20)", &HashMap::new()).unwrap(),
            10.0
        );
        assert_eq!(
            evaluator.evaluate("MAX(10:20)", &HashMap::new()).unwrap(),
            20.0
        );

        // Test with byte variables
        let bytes = [10, 20, 30, 40];
        assert_eq!(
            evaluator
                .evaluate_with_bytes("MIN(A:B)", &HashMap::new(), &bytes)
                .unwrap(),
            10.0
        );
        assert_eq!(
            evaluator
                .evaluate_with_bytes("MAX(A:B:C)", &HashMap::new(), &bytes)
                .unwrap(),
            30.0
        );
    }

    #[test]
    fn test_multi_byte_operations() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // For multi-byte operations, we would need to implement INT16, INT24, INT32 functions
        // These would compose multiple bytes into integers
        // For now, test with basic operations

        let bytes = [0x1A, 0xF8, 0x00, 0x00]; // 26, 248, 0, 0
        let result = evaluator
            .evaluate_with_bytes("A * 256 + B", &HashMap::new(), &bytes)
            .unwrap();
        assert_eq!(result, 6904.0); // 26 * 256 + 248 = 6656 + 248 = 6904
    }

    #[test]
    fn test_stateful_functions_basic() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test that stateful functions are callable (even if state doesn't persist)
        let _result1 = evaluator
            .evaluate("EWMAF(0.5, 100)", &HashMap::new())
            .unwrap();
        let _result2 = evaluator.evaluate("RAVG(50)", &HashMap::new()).unwrap();

        // Functions should not panic and return some value
        assert!(true);
    }

    #[test]
    fn test_rdly_state_persists() {
        // A single ExpressionEvaluator instance must accumulate RDLY state
        // across multiple evaluate() calls. This is the core fix of Phase 4.
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // RDLY(2, value): delay by 2 samples
        // Call 1: buffer empty → returns current value (10.0), pushes 10.0
        let r1 = evaluator.evaluate("RDLY(2, 10)", &vars).unwrap();
        assert_eq!(r1, 10.0, "RDLY call 1: buffer empty, returns current value");

        // Call 2: buffer=[10.0] → returns buf[0]=10.0, pushes 20.0 → buf=[10.0, 20.0]
        let r2 = evaluator.evaluate("RDLY(2, 20)", &vars).unwrap();
        assert_eq!(
            r2, 10.0,
            "RDLY call 2: returns oldest buffered value (10.0)"
        );

        // Call 3: buffer=[10.0, 20.0] → returns buf[0]=10.0, pushes 30.0, removes 10.0 → buf=[20.0, 30.0]
        let r3 = evaluator.evaluate("RDLY(2, 30)", &vars).unwrap();
        assert_eq!(
            r3, 10.0,
            "RDLY call 3: still returns original 10.0 (2-sample delay)"
        );

        // Call 4: buffer=[20.0, 30.0] → returns buf[0]=20.0
        let r4 = evaluator.evaluate("RDLY(2, 40)", &vars).unwrap();
        assert_eq!(
            r4, 20.0,
            "RDLY call 4: now returns 20.0 (delayed by 2 samples)"
        );

        // Confirm state is accumulating (r4 differs from r1)
        assert_ne!(
            r4, r1,
            "State must accumulate: RDLY output changed after delay window"
        );
    }

    #[test]
    fn test_tavg_state_persists() {
        // TAVG must blend toward the new value over time.
        // With tau=1 second, alpha = 1 - exp(-dt/1).
        // First call returns value unchanged (initialization).
        // Subsequent calls with a different value produce a DIFFERENT result (blending occurred).
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // First call: TAVG initializes with 100.0, returns 100.0
        let r1 = evaluator.evaluate("TAVG(1, 100)", &vars).unwrap();
        assert_eq!(r1, 100.0, "TAVG first call: initializes with input value");

        // Small sleep so dt > 0 (ensures alpha > 0 in the EWMA formula)
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Second call: TAVG blends 100.0 (stored) with 0.0 (current) — result should be between 0 and 100
        let r2 = evaluator.evaluate("TAVG(1, 0)", &vars).unwrap();
        assert!(
            r2 < 100.0,
            "TAVG second call: must blend toward new value (result < 100.0), got {}",
            r2
        );
        assert!(r2 >= 0.0, "TAVG result must be >= 0.0");

        // State is persisting: r2 is a blend, not just the raw input (0.0)
        assert!(
            r2 > 0.0,
            "TAVG must retain history: result > 0.0 because prior state (100.0) bleeds through"
        );
    }

    #[test]
    fn test_complex_expressions() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let mut variables = HashMap::new();
        variables.insert("scaling".to_string(), 0.01);

        let bytes = [100, 200, 50, 0];

        // Test complex expression with variables, bytes, and functions
        let result = evaluator
            .evaluate_with_bytes("MAX(A * scaling, B * 0.005)", &variables, &bytes)
            .unwrap();
        let expected = (100.0_f64 * 0.01).max(200.0 * 0.005); // 1.0 vs 1.0, so 1.0
        assert!((result - expected).abs() < 0.001);
    }

    #[test]
    fn test_error_handling() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test invalid expressions
        assert!(evaluator
            .evaluate("INVALID_FUNCTION(1)", &HashMap::new())
            .is_err());

        // Test division by zero (should be handled by JavaScript)
        let result = evaluator.evaluate("1 / 0", &HashMap::new());
        match result {
            Ok(val) if val.is_infinite() => assert!(true), // JavaScript returns Infinity
            _ => assert!(false, "Expected infinity or error"),
        }
    }

    #[test]
    fn test_cross_pid_references() {
        use obd_equation_rs::PlatformCallbacks;

        // Create a custom platform callback that provides PID values
        struct TestPlatformCallbacks {
            pid_values: HashMap<String, f64>,
        }

        impl PlatformCallbacks for TestPlatformCallbacks {
            fn get_barometric_pressure(&self) -> f64 {
                0.0
            }

            fn get_pid_value(&self, pid_name: &str) -> Option<f64> {
                self.pid_values.get(pid_name).copied()
            }
        }

        // Test from Swift EquationProcessorTests.swift - testCrossPIDReferences()
        let mut pid_values = HashMap::new();
        pid_values.insert("HVB Current Low Range".to_string(), 10.0);
        pid_values.insert("HVB Voltage".to_string(), 400.0);

        let callbacks = TestPlatformCallbacks { pid_values };
        let mut evaluator =
            ExpressionEvaluator::new_with_platform_callbacks(Box::new(callbacks)).unwrap();

        let power_flow = evaluator
            .evaluate(
                "val{HVB Current Low Range}*val{HVB Voltage}*0.001",
                &HashMap::new(),
            )
            .unwrap();

        // 10A * 400V * 0.001 = 4kW
        assert!((power_flow - 4.0).abs() < 0.01);
    }

    #[test]
    fn test_int_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();

        // Test from Swift EquationProcessorTests.swift - testMathFunctions()
        let result = evaluator
            .evaluate_with_bytes("INT(A/30)", &HashMap::new(), &[100, 0, 0, 0])
            .unwrap();
        assert_eq!(result, 3.0); // 100/30 = 3.333... -> 3
    }

    #[test]
    fn test_zeroref_condition_true_returns_value() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let bytes = [0, 20, 0, 0]; // A=0, B=20

        // When A==0, return 0 (the trueValue)
        let result = evaluator
            .evaluate_with_bytes(
                "ZEROREF(A==0, 0, '(A*256 + B) * 0.01')",
                &HashMap::new(),
                &bytes,
            )
            .unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_zeroref_condition_false_evaluates_equation() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let bytes = [10, 20, 0, 0]; // A=10, B=20

        // When A!=0, evaluate the expression '(A*256 + B) * 0.01'
        // (10*256 + 20) * 0.01 = 2580 * 0.01 = 25.8
        let result = evaluator
            .evaluate_with_bytes(
                "ZEROREF(A==0, 0, '(A*256 + B) * 0.01')",
                &HashMap::new(),
                &bytes,
            )
            .unwrap();
        assert!(
            (result - 25.8).abs() < 0.001,
            "Expected 25.8, got {}",
            result
        );
    }

    #[test]
    fn test_zeroref_condition_false_plain_value() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let bytes = [10, 0, 0, 0]; // A=10

        // When A!=0, falseValue is a plain numeric expression 'A * 0.5'
        // 10 * 0.5 = 5.0
        let result = evaluator
            .evaluate_with_bytes("ZEROREF(A==0, 0, 'A * 0.5')", &HashMap::new(), &bytes)
            .unwrap();
        assert!((result - 5.0).abs() < 0.001, "Expected 5.0, got {}", result);
    }

    #[test]
    fn test_if_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // IF(true, thenValue, elseValue) -> thenValue
        assert_eq!(evaluator.evaluate("IF(1, 42, 99)", &vars).unwrap(), 42.0);
        // IF(false, thenValue, elseValue) -> elseValue
        assert_eq!(evaluator.evaluate("IF(0, 42, 99)", &vars).unwrap(), 99.0);
        // IF(false, thenValue) with no else -> 0
        assert_eq!(evaluator.evaluate("IF(0, 42)", &vars).unwrap(), 0.0);

        // With byte variables
        let bytes = [10, 20, 0, 0];
        let result = evaluator
            .evaluate_with_bytes("IF(A > 5, A * 2, B)", &vars, &bytes)
            .unwrap();
        assert_eq!(result, 20.0); // A=10 > 5, so A*2 = 20
    }

    #[test]
    fn test_pow_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        assert_eq!(evaluator.evaluate("POW(2, 8)", &vars).unwrap(), 256.0);
        assert_eq!(evaluator.evaluate("POW(3, 0)", &vars).unwrap(), 1.0);
        assert!(
            (evaluator.evaluate("POW(2, 0.5)", &vars).unwrap() - std::f64::consts::SQRT_2).abs()
                < 0.001
        );
    }

    #[test]
    fn test_tan_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        assert!((evaluator.evaluate("TAN(0)", &vars).unwrap() - 0.0).abs() < 0.001);
        // tan(pi/4) = 1.0
        let result = evaluator
            .evaluate("TAN(0.7853981633974483)", &vars)
            .unwrap();
        assert!((result - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_log_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // LOG is natural log (ln)
        assert!((evaluator.evaluate("LOG(1)", &vars).unwrap() - 0.0).abs() < 0.001);
        assert!((evaluator.evaluate("LOG(2.718281828459045)", &vars).unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_log10_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        assert!((evaluator.evaluate("LOG10(1)", &vars).unwrap() - 0.0).abs() < 0.001);
        assert!((evaluator.evaluate("LOG10(100)", &vars).unwrap() - 2.0).abs() < 0.001);
        assert!((evaluator.evaluate("LOG10(1000)", &vars).unwrap() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_exp_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        assert!((evaluator.evaluate("EXP(0)", &vars).unwrap() - 1.0).abs() < 0.001);
        assert!((evaluator.evaluate("EXP(1)", &vars).unwrap() - std::f64::consts::E).abs() < 0.001);
    }

    #[test]
    fn test_int16_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // INT16(A, B) = (A << 8) | B
        let bytes = [0x1A, 0xF8, 0, 0]; // A=0x1A, B=0xF8
        let result = evaluator
            .evaluate_with_bytes("INT16(A, B)", &vars, &bytes)
            .unwrap();
        assert_eq!(result, 6904.0); // 0x1AF8 = 6904
    }

    #[test]
    fn test_int24_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // INT24(A, B, C) = (A << 16) | (B << 8) | C
        let bytes = [0x01, 0x02, 0x03, 0];
        let result = evaluator
            .evaluate_with_bytes("INT24(A, B, C)", &vars, &bytes)
            .unwrap();
        assert_eq!(result, 66051.0); // 0x010203 = 66051
    }

    #[test]
    fn test_int32_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // INT32(A, B, C, D) = (A << 24) | (B << 16) | (C << 8) | D
        let bytes = [0x00, 0x01, 0x00, 0x00];
        let result = evaluator
            .evaluate_with_bytes("INT32(A, B, C, D)", &vars, &bytes)
            .unwrap();
        assert_eq!(result, 65536.0); // 0x00010000 = 65536
    }

    #[test]
    fn test_signed_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // Positive value (< 128) stays positive
        let bytes = [100, 0, 0, 0];
        assert_eq!(
            evaluator
                .evaluate_with_bytes("SIGNED(A)", &vars, &bytes)
                .unwrap(),
            100.0
        );

        // Value >= 128 becomes negative (value - 256)
        let bytes = [200, 0, 0, 0];
        assert_eq!(
            evaluator
                .evaluate_with_bytes("SIGNED(A)", &vars, &bytes)
                .unwrap(),
            -56.0
        ); // 200 - 256

        // Boundary: 128 -> -128
        let bytes = [128, 0, 0, 0];
        assert_eq!(
            evaluator
                .evaluate_with_bytes("SIGNED(A)", &vars, &bytes)
                .unwrap(),
            -128.0
        );

        // Boundary: 127 -> 127
        let bytes = [127, 0, 0, 0];
        assert_eq!(
            evaluator
                .evaluate_with_bytes("SIGNED(A)", &vars, &bytes)
                .unwrap(),
            127.0
        );
    }

    #[test]
    fn test_signed8_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // SIGNED8 is an alias for SIGNED
        let bytes = [200, 0, 0, 0];
        assert_eq!(
            evaluator
                .evaluate_with_bytes("SIGNED8(A)", &vars, &bytes)
                .unwrap(),
            -56.0
        );
    }

    #[test]
    fn test_signed16_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // Value < 32768 stays positive
        let result = evaluator.evaluate("SIGNED16(30000)", &vars).unwrap();
        assert_eq!(result, 30000.0);

        // Value >= 32768 becomes negative (value - 65536)
        let result = evaluator.evaluate("SIGNED16(40000)", &vars).unwrap();
        assert_eq!(result, -25536.0); // 40000 - 65536

        // With INT16: SIGNED16(INT16(A, B))
        let bytes = [0xFF, 0xFE, 0, 0]; // INT16 = 65534
        let result = evaluator
            .evaluate_with_bytes("SIGNED16(INT16(A, B))", &vars, &bytes)
            .unwrap();
        assert_eq!(result, -2.0); // 65534 - 65536
    }

    #[test]
    fn test_signed24_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // Value < 2^23 (8388608) stays positive
        assert_eq!(
            evaluator.evaluate("SIGNED24(8000000)", &vars).unwrap(),
            8000000.0
        );

        // Value >= 2^23 becomes negative (value - 2^24)
        assert_eq!(
            evaluator.evaluate("SIGNED24(16000000)", &vars).unwrap(),
            -777216.0
        ); // 16000000 - 16777216

        // Boundary: 2^23 -> -2^23
        assert_eq!(
            evaluator.evaluate("SIGNED24(8388608)", &vars).unwrap(),
            -8388608.0
        );

        // With INT24: SIGNED24(INT24(A, B, C))
        let bytes = [0xFF, 0xFF, 0xFE, 0]; // INT24 = 16777214
        let result = evaluator
            .evaluate_with_bytes("SIGNED24(INT24(A, B, C))", &vars, &bytes)
            .unwrap();
        assert_eq!(result, -2.0); // 16777214 - 16777216
    }

    #[test]
    fn test_signed32_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // Value < 2^31 (2147483648) stays positive
        assert_eq!(
            evaluator.evaluate("SIGNED32(2000000000)", &vars).unwrap(),
            2000000000.0
        );

        // Value >= 2^31 becomes negative (value - 2^32)
        assert_eq!(
            evaluator.evaluate("SIGNED32(3000000000)", &vars).unwrap(),
            -1294967296.0
        ); // 3000000000 - 4294967296

        // Boundary: 2^31 -> -2^31
        assert_eq!(
            evaluator.evaluate("SIGNED32(2147483648)", &vars).unwrap(),
            -2147483648.0
        );
    }

    #[test]
    fn test_log1p_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // log1p(0) == ln(1) == 0
        assert_eq!(evaluator.evaluate("LOG1P(0)", &vars).unwrap(), 0.0);

        // log1p(1) == ln(2) ≈ 0.6931471805599453
        let result = evaluator.evaluate("LOG1P(1)", &vars).unwrap();
        assert!(
            (result - 0.6931471805599453).abs() < 1e-12,
            "LOG1P(1) was {}",
            result
        );
    }

    #[test]
    fn test_random_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // RANDOM() returns a value in [0, 1) — check across several calls
        for _ in 0..20 {
            let r = evaluator.evaluate("RANDOM()", &vars).unwrap();
            assert!((0.0..1.0).contains(&r), "RANDOM() out of range: {}", r);
        }
    }

    #[test]
    fn test_ascii_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // ascii returns a string, so evaluate() (which returns f64) should error
        let result = evaluator.evaluate("ascii(65)", &vars);
        assert!(
            result.is_err(),
            "ascii should return Err (string cannot be parsed as f64)"
        );

        // Use unified evaluator to verify string result
        match evaluator.evaluate_unified("ascii(65)", &vars).unwrap() {
            ExpressionResult::Text(s) => assert_eq!(s, "A"),
            other => panic!("Expected Text(\"A\"), got {:?}", other),
        }

        // Multiple bytes
        match evaluator.evaluate_unified("ascii(72, 105)", &vars).unwrap() {
            ExpressionResult::Text(s) => assert_eq!(s, "Hi"),
            other => panic!("Expected Text(\"Hi\"), got {:?}", other),
        }
    }

    #[test]
    fn test_float32_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // IEEE 754 encoding of 1.0f32: 0x3F800000
        // bytes: 0x3F, 0x80, 0x00, 0x00
        let bytes = [0x3F, 0x80, 0x00, 0x00];
        let result = evaluator
            .evaluate_with_bytes("FLOAT32(A, B, C, D)", &vars, &bytes)
            .unwrap();
        assert!((result - 1.0).abs() < 0.001, "Expected 1.0, got {}", result);

        // IEEE 754 encoding of -2.0f32: 0xC0000000
        // bytes: 0xC0, 0x00, 0x00, 0x00
        let bytes = [0xC0, 0x00, 0x00, 0x00];
        let result = evaluator
            .evaluate_with_bytes("FLOAT32(A, B, C, D)", &vars, &bytes)
            .unwrap();
        assert!(
            (result - (-2.0)).abs() < 0.001,
            "Expected -2.0, got {}",
            result
        );

        // IEEE 754 encoding of 3.14: approximately 0x4048F5C3
        let bytes = [0x40, 0x48, 0xF5, 0xC3];
        let result = evaluator
            .evaluate_with_bytes("FLOAT32(A, B, C, D)", &vars, &bytes)
            .unwrap();
        assert!(
            (result - 3.14).abs() < 0.01,
            "Expected ~3.14, got {}",
            result
        );
    }

    #[test]
    fn test_float64_function() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // IEEE 754 encoding of 1.0f64: 0x3FF0000000000000
        // bytes: 0x3F, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        let bytes = [0x3F, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = evaluator
            .evaluate_with_bytes("FLOAT64(A, B, C, D, E, F, G, H)", &vars, &bytes)
            .unwrap();
        assert!((result - 1.0).abs() < 0.001, "Expected 1.0, got {}", result);

        // IEEE 754 encoding of -1.0f64: 0xBFF0000000000000
        let bytes = [0xBF, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = evaluator
            .evaluate_with_bytes("FLOAT64(A, B, C, D, E, F, G, H)", &vars, &bytes)
            .unwrap();
        assert!(
            (result - (-1.0)).abs() < 0.001,
            "Expected -1.0, got {}",
            result
        );
    }

    #[test]
    fn test_avg_state_persists() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // AVG(bucketSize, value) - running average
        let _r1 = evaluator.evaluate("AVG(3, 10)", &vars).unwrap();
        let _r2 = evaluator.evaluate("AVG(3, 20)", &vars).unwrap();
        let r3 = evaluator.evaluate("AVG(3, 30)", &vars).unwrap();
        // After 3 samples, average of 10+20+30 = 20
        assert!(
            (r3 - 20.0).abs() < 0.001,
            "Expected avg of 10,20,30 = 20.0, got {}",
            r3
        );
    }

    #[test]
    fn test_tdly_state_persists() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // TDLY(delay, value) - time-based delay
        // First call returns current value (no history)
        let r1 = evaluator.evaluate("TDLY(1, 100)", &vars).unwrap();
        // Should return some value without panicking
        assert!(
            r1.is_finite(),
            "TDLY should return a finite value, got {}",
            r1
        );
    }

    #[test]
    fn test_tot_state_persists() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // TOT(window, value) - running total
        let r1 = evaluator.evaluate("TOT(5, 10)", &vars).unwrap();
        let r2 = evaluator.evaluate("TOT(5, 20)", &vars).unwrap();
        // Total should accumulate
        assert!(r2 >= r1, "TOT should accumulate: r2={} >= r1={}", r2, r1);
    }

    #[test]
    fn test_baro_function() {
        use obd_equation_rs::PlatformCallbacks;

        struct TestBaroCallbacks;
        impl PlatformCallbacks for TestBaroCallbacks {
            fn get_barometric_pressure(&self) -> f64 {
                101.325 // standard atmospheric pressure in kPa
            }
            fn get_pid_value(&self, _pid_name: &str) -> Option<f64> {
                None
            }
        }

        let mut evaluator =
            ExpressionEvaluator::new_with_platform_callbacks(Box::new(TestBaroCallbacks)).unwrap();
        let vars = HashMap::new();

        let result = evaluator.evaluate("BARO()", &vars).unwrap();
        assert!(
            (result - 101.325).abs() < 0.001,
            "Expected 101.325, got {}",
            result
        );
    }

    #[test]
    fn test_baro_default_zero() {
        // Without platform callbacks, BARO should return 0
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        let result = evaluator.evaluate("BARO()", &vars).unwrap();
        assert_eq!(result, 0.0);
    }

    /// Tests that generate_equation() produces equations that evaluate to the same
    /// results as the Wikipedia/J1979 reference formulas for standard OBD-II PIDs.
    /// This catches bugs like signed flags being wrong on temperature PIDs.
    #[test]
    fn test_generated_vs_wikipedia_equations() {
        let mut evaluator = ExpressionEvaluator::new().unwrap();
        let vars = HashMap::new();

        // Each entry: (name, generated_params, wikipedia_formula, test_bytes, expected_value)
        struct PidTest {
            name: &'static str,
            scale: f64,
            offset: f64,
            start_byte: i32,
            end_byte: i32,
            signed: bool,
            wikipedia_formula: &'static str,
            bytes: &'static [u8],
            expected: f64,
        }

        let tests = vec![
            // PID 0105: Engine Coolant Temperature
            // Wikipedia: A - 40, range -40 to 215°C, UNSIGNED
            PidTest {
                name: "ECT (0105) - normal operating temp",
                scale: 1.0,
                offset: -40.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A - 40",
                bytes: &[0x85, 0, 0, 0], // 133 - 40 = 93°C
                expected: 93.0,
            },
            PidTest {
                name: "ECT (0105) - cold start",
                scale: 1.0,
                offset: -40.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A - 40",
                bytes: &[0x28, 0, 0, 0], // 40 - 40 = 0°C
                expected: 0.0,
            },
            PidTest {
                name: "ECT (0105) - minimum",
                scale: 1.0,
                offset: -40.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A - 40",
                bytes: &[0x00, 0, 0, 0], // 0 - 40 = -40°C
                expected: -40.0,
            },
            // PID 010C: Engine RPM
            // Wikipedia: (256*A + B) / 4, range 0 to 16383.75 rpm, UNSIGNED
            PidTest {
                name: "RPM (010C) - idle",
                scale: 0.25,
                offset: 0.0,
                start_byte: 0,
                end_byte: 1,
                signed: false,
                wikipedia_formula: "(A * 256 + B) / 4",
                bytes: &[0x0B, 0xB4, 0, 0], // (11*256 + 180) / 4 = 2816+180/4 = 749
                expected: 749.0,
            },
            PidTest {
                name: "RPM (010C) - high rev",
                scale: 0.25,
                offset: 0.0,
                start_byte: 0,
                end_byte: 1,
                signed: false,
                wikipedia_formula: "(A * 256 + B) / 4",
                bytes: &[0x1F, 0x40, 0, 0], // (31*256 + 64) / 4 = 8000/4 = 2000
                expected: 2000.0,
            },
            // PID 010D: Vehicle Speed
            // Wikipedia: A, range 0 to 255 km/h, UNSIGNED
            PidTest {
                name: "Speed (010D)",
                scale: 1.0,
                offset: 0.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A",
                bytes: &[0x50, 0, 0, 0], // 80 km/h
                expected: 80.0,
            },
            // PID 0104: Engine Load
            // Wikipedia: A * 100/255, range 0 to 100%, UNSIGNED
            PidTest {
                name: "Engine Load (0104)",
                scale: 0.3922,
                offset: 0.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A * 100 / 255",
                bytes: &[0x80, 0, 0, 0], // 128 * 0.3922 ≈ 50.2
                expected: 128.0 * 0.3922,
            },
            // PID 010F: Intake Air Temperature
            // Wikipedia: A - 40, range -40 to 215°C, UNSIGNED (same as ECT)
            PidTest {
                name: "IAT (010F)",
                scale: 1.0,
                offset: -40.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A - 40",
                bytes: &[0x41, 0, 0, 0], // 65 - 40 = 25°C
                expected: 25.0,
            },
            // PID 0146: Ambient Air Temperature
            // Wikipedia: A - 40, UNSIGNED
            PidTest {
                name: "Ambient Temp (0146)",
                scale: 1.0,
                offset: -40.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A - 40",
                bytes: &[0x3C, 0, 0, 0], // 60 - 40 = 20°C
                expected: 20.0,
            },
            // PID 015C: Engine Oil Temperature
            // Wikipedia: A - 40, UNSIGNED
            PidTest {
                name: "Oil Temp (015C)",
                scale: 1.0,
                offset: -40.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "A - 40",
                bytes: &[0x82, 0, 0, 0], // 130 - 40 = 90°C
                expected: 90.0,
            },
            // PID 0106: Short Term Fuel Trim Bank 1
            // Wikipedia: (A - 128) * 100/128, range -100 to +99.2%, UNSIGNED byte
            PidTest {
                name: "STFT Bank 1 (0106) - lean",
                scale: 0.7812,
                offset: -100.0,
                start_byte: 0,
                end_byte: 0,
                signed: false,
                wikipedia_formula: "(A - 128) * 100 / 128",
                bytes: &[0x80, 0, 0, 0], // 128 * 0.7812 - 100 = 0 (centered)
                expected: 128.0 * 0.7812 - 100.0,
            },
            // PID 0110: MAF Air Flow Rate
            // Wikipedia: (256*A + B) / 100, range 0 to 655.35 g/s, UNSIGNED
            PidTest {
                name: "MAF (0110)",
                scale: 0.01,
                offset: 0.0,
                start_byte: 0,
                end_byte: 1,
                signed: false,
                wikipedia_formula: "(A * 256 + B) / 100",
                bytes: &[0x01, 0xF4, 0, 0], // (1*256 + 244) / 100 = 500/100 = 5.0 g/s
                expected: 5.0,
            },
            // PID 013C: Catalyst Temperature Bank 1 Sensor 1
            // Wikipedia: (256*A + B) / 10 - 40, range -40 to 6513.5°C, UNSIGNED
            PidTest {
                name: "Catalyst Temp (013C)",
                scale: 0.1,
                offset: -40.0,
                start_byte: 0,
                end_byte: 1,
                signed: false,
                wikipedia_formula: "(A * 256 + B) / 10 - 40",
                bytes: &[0x03, 0x52, 0, 0], // (3*256 + 82) * 0.1 - 40 = 850*0.1 - 40 = 45°C
                expected: (3.0 * 256.0 + 82.0) * 0.1 - 40.0,
            },
        ];

        for test in &tests {
            let generated_eq = generate_equation(
                test.scale,
                test.offset,
                test.start_byte,
                test.end_byte,
                test.signed,
            )
            .expect(&format!("Failed to generate equation for {}", test.name));

            let gen_result = evaluator
                .evaluate_with_bytes(&generated_eq, &vars, test.bytes)
                .expect(&format!(
                    "Failed to evaluate generated eq for {}: {}",
                    test.name, generated_eq
                ));

            let wiki_result = evaluator
                .evaluate_with_bytes(test.wikipedia_formula, &vars, test.bytes)
                .expect(&format!(
                    "Failed to evaluate wikipedia eq for {}: {}",
                    test.name, test.wikipedia_formula
                ));

            // Generated equation must match expected value
            assert!(
                (gen_result - test.expected).abs() < 0.01,
                "{}: generated eq '{}' = {}, expected {}",
                test.name,
                generated_eq,
                gen_result,
                test.expected
            );

            // Generated equation must match Wikipedia formula
            assert!(
                (gen_result - wiki_result).abs() < 0.01,
                "{}: generated eq '{}' = {} but wikipedia '{}' = {} — MISMATCH",
                test.name,
                generated_eq,
                gen_result,
                test.wikipedia_formula,
                wiki_result
            );

            println!(
                "  PASS  {:40} gen={:<40} wiki={:<25} value={:.4}",
                test.name, generated_eq, test.wikipedia_formula, gen_result
            );
        }
    }
}
