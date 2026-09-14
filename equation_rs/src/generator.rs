//! Equation string generator from PID parameters.
//!
//! Generates equation strings like `(0.25 * (A*256 + B) + 0.0)` from
//! scale, offset, byte range, and signedness parameters.

/// Generate an equation string from PID parameters.
///
/// # Arguments
/// * `scale` - Multiplier (e.g., 0.25 for RPM)
/// * `offset` - Additive offset (e.g., -40 for temperature)
/// * `start_byte` - First byte index (0-based, maps to variable A=0, B=1, etc.)
/// * `end_byte` - Last byte index (inclusive)
/// * `signed` - Whether the value is signed (two's complement)
///
/// # Returns
/// Some(equation_string) or None if parameters are invalid.
pub fn generate_equation(
    scale: f64,
    offset: f64,
    start_byte: i32,
    end_byte: i32,
    signed: bool,
) -> Option<String> {
    // Validate. Byte indices map to A–Z then A1–Z1, A2–Z2, … so there's no 26-byte
    // ceiling; cap generously (real PIDs top out ~byte 41, e.g. 0181).
    if start_byte < 0 || end_byte < start_byte || end_byte > 63 {
        return None;
    }

    let start = start_byte as usize;
    let end = end_byte as usize;
    let byte_count = end - start + 1;

    if byte_count == 0 {
        return None;
    }

    // Build byte-weighted terms: A=byte0, B=byte1, … Z=byte25, A1=byte26, …
    let byte_terms: Vec<String> = (0..byte_count)
        .map(|i| {
            let byte_index = start + i;
            let var_letter = crate::variables::byte_var_name(byte_index);
            let power = byte_count - 1 - i;
            if power > 0 {
                format!("{}*{}", var_letter, 256_u64.pow(power as u32))
            } else {
                var_letter
            }
        })
        .collect();

    let combined_expr = byte_terms.join(" + ");

    let inner = if signed {
        if byte_count == 1 {
            format!("SIGNED({})", combined_expr)
        } else if byte_count == 2 {
            format!("SIGNED16({})", combined_expr)
        } else {
            // Fallback for 3+ bytes: IF/THEN/ELSE
            let bit_width = 8 * byte_count;
            let max_unsigned = 1_u64 << bit_width;
            let signed_threshold = 1_u64 << (bit_width - 1);
            format!(
                "IF {} >= {} THEN {} - {} ELSE {}",
                combined_expr, signed_threshold, combined_expr, max_unsigned, combined_expr
            )
        }
    } else {
        combined_expr
    };

    let has_scale = scale != 1.0;
    let has_offset = offset != 0.0;

    // Only wrap inner in parens if it's a compound expression (multi-byte or signed)
    let needs_parens = byte_count > 1 || signed;
    let scaled_inner = if needs_parens {
        format!("({})", inner)
    } else {
        inner.clone()
    };

    let eq = match (has_scale, has_offset) {
        (false, false) => inner,
        (false, true) => format!("{} + {}", inner, offset),
        (true, false) => format!("{} * {}", scale, scaled_inner),
        (true, true) => format!("{} * {} + {}", scale, scaled_inner, offset),
    };

    Some(eq)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Single byte, unsigned
    #[test]
    fn test_coolant_temp() {
        let eq = generate_equation(1.0, -40.0, 0, 0, false).unwrap();
        assert_eq!(eq, "A + -40");
    }

    #[test]
    fn test_speed() {
        let eq = generate_equation(1.0, 0.0, 0, 0, false).unwrap();
        assert_eq!(eq, "A");
    }

    #[test]
    fn test_engine_load() {
        let eq = generate_equation(0.3922, 0.0, 0, 0, false).unwrap();
        assert_eq!(eq, "0.3922 * A");
    }

    // Single byte, signed
    #[test]
    fn test_timing_advance() {
        let eq = generate_equation(0.5, -64.0, 0, 0, true).unwrap();
        assert_eq!(eq, "0.5 * (SIGNED(A)) + -64");
    }

    // Two bytes, unsigned
    #[test]
    fn test_rpm() {
        let eq = generate_equation(0.25, 0.0, 0, 1, false).unwrap();
        assert_eq!(eq, "0.25 * (A*256 + B)");
    }

    #[test]
    fn test_maf() {
        let eq = generate_equation(0.01, 0.0, 0, 1, false).unwrap();
        assert_eq!(eq, "0.01 * (A*256 + B)");
    }

    // Two bytes, signed
    #[test]
    fn test_o2_wide_current() {
        let eq = generate_equation(0.00390625, 0.0, 2, 3, true).unwrap();
        assert_eq!(eq, "0.00390625 * (SIGNED16(C*256 + D))");
    }

    // Non-zero startByte
    #[test]
    fn test_o2_lambda_bytes_0_1() {
        let eq = generate_equation(0.0000305, 0.0, 0, 1, false).unwrap();
        assert_eq!(eq, "0.0000305 * (A*256 + B)");
    }

    #[test]
    fn test_o2_current_bytes_2_3() {
        let eq = generate_equation(0.00390625, 0.0, 2, 3, true).unwrap();
        assert_eq!(eq, "0.00390625 * (SIGNED16(C*256 + D))");
    }

    // Three bytes
    #[test]
    fn test_three_byte_unsigned() {
        let eq = generate_equation(1.0, 0.0, 0, 2, false).unwrap();
        assert_eq!(eq, "A*65536 + B*256 + C");
    }

    #[test]
    fn test_three_byte_signed() {
        let eq = generate_equation(1.0, 0.0, 0, 2, true).unwrap();
        assert!(eq.contains("IF"));
        assert!(eq.contains("8388608")); // 2^23
        assert!(eq.contains("16777216")); // 2^24
    }

    // Edge cases
    #[test]
    fn test_negative_offset() {
        let eq = generate_equation(1.0, -40.0, 0, 0, false).unwrap();
        assert!(eq.contains("-40"));
    }

    #[test]
    fn test_very_small_scale() {
        let eq = generate_equation(0.0000305, 0.0, 0, 1, false).unwrap();
        assert!(eq.contains("0.0000305"));
    }

    #[test]
    fn test_invalid_end_before_start() {
        assert!(generate_equation(1.0, 0.0, 3, 1, false).is_none());
    }

    #[test]
    fn test_invalid_negative_start() {
        assert!(generate_equation(1.0, 0.0, -1, 0, false).is_none());
    }

    #[test]
    fn test_invalid_byte_too_large() {
        // Byte 64+ is beyond the (generous) generator cap.
        assert!(generate_equation(1.0, 0.0, 64, 64, false).is_none());
    }

    // Bytes past Z use A1, B1, … (byte 26 = A1). Regression for the old `> 25` reject.
    #[test]
    fn test_byte_past_z_single() {
        assert_eq!(generate_equation(1.0, 0.0, 26, 26, false).unwrap(), "A1");
        assert_eq!(
            generate_equation(1.0, -40.0, 39, 39, false).unwrap(),
            "N1 + -40"
        );
    }

    // 0181-style: a 4-byte value at bytes 38–41 → M1..P1 (the case that used to silently break).
    #[test]
    fn test_four_byte_high_position() {
        let eq = generate_equation(1.0, 0.0, 38, 41, false).unwrap();
        assert_eq!(eq, "M1*16777216 + N1*65536 + O1*256 + P1");
    }

    // End-to-end: generate an equation over high bytes AND evaluate it against a long
    // (42-byte) response — proves the generator and the byte-variable resolver agree on
    // names past Z, so 0181's AECD timers decode correctly.
    #[test]
    fn test_generate_and_evaluate_many_bytes() {
        let eq = generate_equation(1.0, 0.0, 38, 41, false).unwrap();

        let mut bytes = vec![0u8; 42]; // 42-byte multi-frame response, like 0181
        bytes[38] = 0x12; // M1 (most significant)
        bytes[39] = 0x34; // N1
        bytes[40] = 0x56; // O1
        bytes[41] = 0x78; // P1
        let expected = ((0x12u64 << 24) | (0x34 << 16) | (0x56 << 8) | 0x78) as f64; // 0x12345678

        let mut evaluator = crate::ExpressionEvaluator::new().unwrap();
        let value = evaluator
            .evaluate_with_bytes(&eq, &std::collections::HashMap::new(), &bytes)
            .unwrap();
        assert_eq!(value, expected);
    }
}
