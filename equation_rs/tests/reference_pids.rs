// Reference PID Validation Tests
//
// Validates the equation generator and expression evaluator against known SAE J1979
// OBD-II Mode 01 PIDs. Each test case specifies a PID's scale, offset, byte range,
// signedness, sample raw bytes, and the expected decoded value.
//
// Tiers:
//   1 — Core single-formula PIDs (engine load, coolant temp, RPM, speed, etc.)
//   2 — Multi-datapoint PIDs tested per sub-formula (O2 sensors, catalyst temp)
//   3 — Signed two's complement PIDs (evap vapor pressure)
//   4 — Multi-byte (4-byte) PIDs (odometer, max MAF rate)
//
// Boundary tests cover all-zero and all-0xFF inputs for representative PIDs.
// Structure tests verify generated equation strings contain expected terms.
//
// The `export_results_csv` test writes a CSV to `tests/output/reference_pid_results.csv`
// summarizing every case with its generated equation, expected vs actual value, and
// pass/fail status.

use obd_equation_rs::generator::generate_equation;
use obd_equation_rs::ExpressionEvaluator;
use std::collections::HashMap;

struct ReferencePidCase {
    pid: &'static str,
    name: &'static str,
    reference_eq: &'static str,
    scale: f64,
    offset: f64,
    start_byte: i32,
    end_byte: i32,
    signed: bool,
    test_bytes: &'static [u8],
    expected: f64,
    tolerance: f64,
}

struct UntestablePid {
    pid: &'static str,
    name: &'static str,
    reason: &'static str,
}

const DEFAULT_TOL: f64 = 0.001;

fn tier1_cases() -> Vec<ReferencePidCase> {
    vec![
        ReferencePidCase {
            pid: "0x04",
            name: "Calculated engine load",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[255],
            expected: 100.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x05",
            name: "Coolant temp",
            reference_eq: "A - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[60],
            expected: 20.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x06",
            name: "STFT Bank 1",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[128],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x07",
            name: "LTFT Bank 1",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[0],
            expected: -100.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x08",
            name: "STFT Bank 2",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[255],
            expected: 99.21875,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x09",
            name: "LTFT Bank 2",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[64],
            expected: -50.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x0A",
            name: "Fuel pressure",
            reference_eq: "A * 3",
            scale: 3.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[100],
            expected: 300.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x0B",
            name: "Intake MAP",
            reference_eq: "A",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[101],
            expected: 101.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x0C",
            name: "Engine RPM",
            reference_eq: "(256 * A + B) / 4",
            scale: 0.25,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[31, 64],
            expected: 2000.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x0D",
            name: "Vehicle speed",
            reference_eq: "A",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[120],
            expected: 120.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x0E",
            name: "Timing advance",
            reference_eq: "A / 2 - 64",
            scale: 0.5,
            offset: -64.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[138],
            expected: 5.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x0F",
            name: "Intake air temp",
            reference_eq: "A - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[65],
            expected: 25.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x10",
            name: "MAF air flow",
            reference_eq: "(256 * A + B) / 100",
            scale: 0.01,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[1, 0],
            expected: 2.56,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x11",
            name: "Throttle position",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[127],
            expected: 49.80392156862745,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x1F",
            name: "Run time since start",
            reference_eq: "256 * A + B",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 120],
            expected: 120.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x21",
            name: "Distance w/ MIL",
            reference_eq: "256 * A + B",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[1, 44],
            expected: 300.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x22",
            name: "Fuel rail pressure (rel)",
            reference_eq: "0.079 * (256 * A + B)",
            scale: 0.079,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 100],
            expected: 7.9,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x23",
            name: "Fuel rail gauge pressure",
            reference_eq: "10 * (256 * A + B)",
            scale: 10.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 50],
            expected: 500.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x2C",
            name: "Commanded EGR",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x2D",
            name: "EGR Error",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[128],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x2E",
            name: "Cmd evap purge",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[255],
            expected: 100.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x2F",
            name: "Fuel tank level",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[128],
            expected: 50.19607843137255,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x30",
            name: "Warm-ups since clear",
            reference_eq: "A",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[42],
            expected: 42.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x31",
            name: "Distance since clear",
            reference_eq: "256 * A + B",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[10, 0],
            expected: 2560.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x33",
            name: "Abs barometric pressure",
            reference_eq: "A",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[101],
            expected: 101.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x42",
            name: "Control module voltage",
            reference_eq: "(256 * A + B) / 1000",
            scale: 0.001,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[55, 240],
            expected: 14.32,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x43",
            name: "Absolute load value",
            reference_eq: "(256 * A + B) * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 255],
            expected: 100.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x44",
            name: "Cmd air-fuel ratio (lambda)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[128, 0],
            expected: 1.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x45",
            name: "Relative throttle pos",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[50],
            expected: 19.607843137254903,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x46",
            name: "Ambient air temp",
            reference_eq: "A - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[55],
            expected: 15.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x47",
            name: "Abs throttle pos B",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[200],
            expected: 78.43137254901961,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x48",
            name: "Abs throttle pos C",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[150],
            expected: 58.8235294117647,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x49",
            name: "Accelerator pedal pos D",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[80],
            expected: 31.37254901960784,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x4A",
            name: "Accelerator pedal pos E",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[180],
            expected: 70.58823529411765,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x4B",
            name: "Accelerator pedal pos F",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[60],
            expected: 23.52941176470588,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x4C",
            name: "Cmd throttle actuator",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[100],
            expected: 39.21568627450981,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x4D",
            name: "Time run with MIL on",
            reference_eq: "256 * A + B",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 60],
            expected: 60.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x4E",
            name: "Time since codes cleared",
            reference_eq: "256 * A + B",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[2, 88],
            expected: 600.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x52",
            name: "Ethanol fuel %",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[26],
            expected: 10.196078431372548,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x53",
            name: "Abs evap vapor pressure",
            reference_eq: "(256 * A + B) / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 200],
            expected: 1.0,
            tolerance: DEFAULT_TOL,
        },
        // Secondary O2 sensor trims (0x55-0x58): A = bank 1/2, B = bank 3/4, same formula
        ReferencePidCase {
            pid: "0x55",
            name: "Short term sec O2 trim A (B1)",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[128],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x55",
            name: "Short term sec O2 trim B (B3)",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 64],
            expected: -50.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x56",
            name: "Long term sec O2 trim A (B1)",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[192],
            expected: 50.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x56",
            name: "Long term sec O2 trim B (B3)",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 255],
            expected: 99.21875,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x57",
            name: "Short term sec O2 trim A (B2)",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[0],
            expected: -100.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x57",
            name: "Short term sec O2 trim B (B4)",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 128],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x58",
            name: "Long term sec O2 trim A (B2)",
            reference_eq: "A * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[160],
            expected: 25.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x58",
            name: "Long term sec O2 trim B (B4)",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 96],
            expected: -25.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x59",
            name: "Fuel rail abs pressure",
            reference_eq: "10 * (256 * A + B)",
            scale: 10.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[1, 0],
            expected: 2560.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x5A",
            name: "Rel accelerator pedal pos",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[127],
            expected: 49.80392156862745,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x5B",
            name: "Hybrid battery life",
            reference_eq: "A * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[191],
            expected: 74.90196078431373,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x5C",
            name: "Engine oil temp",
            reference_eq: "A - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[130],
            expected: 90.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x5D",
            name: "Fuel injection timing",
            reference_eq: "(256 * A + B) / 128 - 210",
            scale: 1.0 / 128.0,
            offset: -210.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[105, 0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x5E",
            name: "Engine fuel rate",
            reference_eq: "(256 * A + B) * 0.05",
            scale: 0.05,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 200],
            expected: 10.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x61",
            name: "Driver demand torque %",
            reference_eq: "A - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[125],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x62",
            name: "Actual engine torque %",
            reference_eq: "A - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[200],
            expected: 75.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x63",
            name: "Engine reference torque",
            reference_eq: "256 * A + B",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[1, 100],
            expected: 356.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x8E",
            name: "Engine friction torque %",
            reference_eq: "A - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[100],
            expected: -25.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0xA2",
            name: "Cylinder fuel rate",
            reference_eq: "(256 * A + B) / 32",
            scale: 0.03125,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 160],
            expected: 5.0,
            tolerance: DEFAULT_TOL,
        },
    ]
}

// Tier 2: Multi-datapoint PIDs — each sub-formula tested separately
fn tier2_cases() -> Vec<ReferencePidCase> {
    vec![
        // O2 narrow (0x14): Voltage = A/200
        ReferencePidCase {
            pid: "0x14",
            name: "O2 narrow voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[200],
            expected: 1.0,
            tolerance: DEFAULT_TOL,
        },
        // O2 narrow (0x14): STFT = 100/128*B - 100  (byte index 1)
        ReferencePidCase {
            pid: "0x14",
            name: "O2 narrow STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 128],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        // O2 narrow sensors 2-8 (0x15-0x1B): same formula as 0x14
        ReferencePidCase {
            pid: "0x15",
            name: "O2 S2 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[100],
            expected: 0.5,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x15",
            name: "O2 S2 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 64],
            expected: -50.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x16",
            name: "O2 S3 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[255],
            expected: 1.275,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x16",
            name: "O2 S3 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 0],
            expected: -100.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x17",
            name: "O2 S4 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x17",
            name: "O2 S4 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 255],
            expected: 99.21875,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x18",
            name: "O2 S5 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[150],
            expected: 0.75,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x18",
            name: "O2 S5 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 192],
            expected: 50.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x19",
            name: "O2 S6 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[80],
            expected: 0.4,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x19",
            name: "O2 S6 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 128],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x1A",
            name: "O2 S7 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[50],
            expected: 0.25,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x1A",
            name: "O2 S7 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 96],
            expected: -25.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x1B",
            name: "O2 S8 voltage",
            reference_eq: "A / 200",
            scale: 0.005,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[180],
            expected: 0.9,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x1B",
            name: "O2 S8 STFT",
            reference_eq: "B * 100 / 128 - 100",
            scale: 100.0 / 128.0,
            offset: -100.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 160],
            expected: 25.0,
            tolerance: DEFAULT_TOL,
        },
        // O2 wide lambda+V (0x24): Lambda = 2/65536 * (256*A + B)
        ReferencePidCase {
            pid: "0x24",
            name: "O2 wide lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[128, 0, 0, 0],
            expected: 1.0,
            tolerance: 0.01,
        },
        // O2 wide lambda+V (0x24): Voltage = 8/65536 * (256*C + D)
        ReferencePidCase {
            pid: "0x24",
            name: "O2 wide voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 128, 0],
            expected: 4.0,
            tolerance: 0.01,
        },
        // O2 wide lambda+V sensors 2-8 (0x25-0x2B): same formula as 0x24
        ReferencePidCase {
            pid: "0x25",
            name: "O2 wide S2 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[64, 0, 0, 0],
            expected: 0.5,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x25",
            name: "O2 wide S2 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 64, 0],
            expected: 2.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x26",
            name: "O2 wide S3 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[192, 0, 0, 0],
            expected: 1.5,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x26",
            name: "O2 wide S3 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 192, 0],
            expected: 6.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x27",
            name: "O2 wide S4 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 0, 0, 0],
            expected: 0.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x27",
            name: "O2 wide S4 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 0, 0],
            expected: 0.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x28",
            name: "O2 wide S5 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[255, 255, 0, 0],
            expected: 1.9999694824,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x28",
            name: "O2 wide S5 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 255, 255],
            expected: 7.9999389648,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x29",
            name: "O2 wide S6 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[96, 0, 0, 0],
            expected: 0.75,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x29",
            name: "O2 wide S6 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 96, 0],
            expected: 3.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x2A",
            name: "O2 wide S7 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[160, 0, 0, 0],
            expected: 1.25,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x2A",
            name: "O2 wide S7 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 160, 0],
            expected: 5.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x2B",
            name: "O2 wide S8 lambda",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[32, 0, 0, 0],
            expected: 0.25,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x2B",
            name: "O2 wide S8 voltage",
            reference_eq: "8 / 65536 * (256 * C + D)",
            scale: 8.0 / 65536.0,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 32, 0],
            expected: 1.0,
            tolerance: 0.01,
        },
        // O2 wide lambda+mA (0x34): Lambda = 2/65536 * (256*A + B)
        ReferencePidCase {
            pid: "0x34",
            name: "O2 wide lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[128, 0, 0, 0],
            expected: 1.0,
            tolerance: 0.01,
        },
        // O2 wide lambda+mA (0x34): Current = (256*C+D)/256 - 128
        ReferencePidCase {
            pid: "0x34",
            name: "O2 wide current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 128, 0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        // O2 wide lambda+mA sensors 2-8 (0x35-0x3B): same formula as 0x34
        ReferencePidCase {
            pid: "0x35",
            name: "O2 wide S2 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[64, 0, 0, 0],
            expected: 0.5,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x35",
            name: "O2 wide S2 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 64, 0],
            expected: -64.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x36",
            name: "O2 wide S3 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[192, 0, 0, 0],
            expected: 1.5,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x36",
            name: "O2 wide S3 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 192, 0],
            expected: 64.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x37",
            name: "O2 wide S4 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[128, 0, 0, 0],
            expected: 1.0,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x37",
            name: "O2 wide S4 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 0, 0],
            expected: -128.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x38",
            name: "O2 wide S5 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[96, 0, 0, 0],
            expected: 0.75,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x38",
            name: "O2 wide S5 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 255, 255],
            expected: 127.99609375,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x39",
            name: "O2 wide S6 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[160, 0, 0, 0],
            expected: 1.25,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x39",
            name: "O2 wide S6 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 160, 0],
            expected: 32.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x3A",
            name: "O2 wide S7 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[32, 0, 0, 0],
            expected: 0.25,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x3A",
            name: "O2 wide S7 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 96, 0],
            expected: -32.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x3B",
            name: "O2 wide S8 lambda (mA)",
            reference_eq: "2 / 65536 * (256 * A + B)",
            scale: 2.0 / 65536.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[255, 255, 0, 0],
            expected: 1.9999694824,
            tolerance: 0.01,
        },
        ReferencePidCase {
            pid: "0x3B",
            name: "O2 wide S8 current",
            reference_eq: "(256 * C + D) / 256 - 128",
            scale: 1.0 / 256.0,
            offset: -128.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 128, 0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        // Catalyst temp (0x3C): (256*A + B)/10 - 40
        ReferencePidCase {
            pid: "0x3C",
            name: "Catalyst temp B1S1",
            reference_eq: "(256 * A + B) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[1, 144],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        // Catalyst temp sensors 0x3D-0x3F: same formula as 0x3C
        ReferencePidCase {
            pid: "0x3D",
            name: "Catalyst temp B2S1",
            reference_eq: "(256 * A + B) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[3, 32],
            expected: 40.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x3E",
            name: "Catalyst temp B1S2",
            reference_eq: "(256 * A + B) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[4, 176],
            expected: 80.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x3F",
            name: "Catalyst temp B2S2",
            reference_eq: "(256 * A + B) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 0,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 0],
            expected: -40.0,
            tolerance: DEFAULT_TOL,
        },
        // === Support-byte PIDs: byte A is a support bitmask, data starts at byte B ===

        // 0x64: Engine percent torque data — 5 values, each X-125
        // A=idle, B=point1, C=point2, D=point3, E=point4
        ReferencePidCase {
            pid: "0x64",
            name: "Engine torque idle",
            reference_eq: "A - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[125, 0, 0, 0, 0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x64",
            name: "Engine torque point 1",
            reference_eq: "B - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0, 200, 0, 0, 0],
            expected: 75.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x64",
            name: "Engine torque point 2",
            reference_eq: "C - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 2,
            end_byte: 2,
            signed: false,
            test_bytes: &[0, 0, 255, 0, 0],
            expected: 130.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x64",
            name: "Engine torque point 3",
            reference_eq: "D - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 3,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 0, 0, 0],
            expected: -125.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x64",
            name: "Engine torque point 4",
            reference_eq: "E - 125",
            scale: 1.0,
            offset: -125.0,
            start_byte: 4,
            end_byte: 4,
            signed: false,
            test_bytes: &[0, 0, 0, 0, 180],
            expected: 55.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x66: MAF sensor — A=support, sensor A=(256*B+C)/32, sensor B=(256*D+E)/32
        ReferencePidCase {
            pid: "0x66",
            name: "MAF sensor A",
            reference_eq: "(256 * B + C) / 32",
            scale: 0.03125,
            offset: 0.0,
            start_byte: 1,
            end_byte: 2,
            signed: false,
            test_bytes: &[0x03, 4, 0, 0, 0],
            expected: 32.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x66",
            name: "MAF sensor B",
            reference_eq: "(256 * D + E) / 32",
            scale: 0.03125,
            offset: 0.0,
            start_byte: 3,
            end_byte: 4,
            signed: false,
            test_bytes: &[0x03, 0, 0, 2, 0],
            expected: 16.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x67: Engine coolant temp — A=support, sensor 1=B-40, sensor 2=C-40
        ReferencePidCase {
            pid: "0x67",
            name: "Coolant temp sensor 1",
            reference_eq: "B - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0x03, 90, 0],
            expected: 50.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x67",
            name: "Coolant temp sensor 2",
            reference_eq: "C - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 2,
            end_byte: 2,
            signed: false,
            test_bytes: &[0x03, 0, 120],
            expected: 80.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x68: Intake air temp — A=support, sensor 1=B-40, sensor 2=C-40
        ReferencePidCase {
            pid: "0x68",
            name: "Intake air temp sensor 1",
            reference_eq: "B - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0x03, 65, 0],
            expected: 25.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x68",
            name: "Intake air temp sensor 2",
            reference_eq: "C - 40",
            scale: 1.0,
            offset: -40.0,
            start_byte: 2,
            end_byte: 2,
            signed: false,
            test_bytes: &[0x03, 0, 100],
            expected: 60.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x78: EGT Bank 1 — A=support, sensors use (256*X+Y)/10 - 40
        ReferencePidCase {
            pid: "0x78",
            name: "EGT B1S1",
            reference_eq: "(256 * B + C) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 1,
            end_byte: 2,
            signed: false,
            test_bytes: &[0x0F, 1, 144, 0, 0, 0, 0, 0, 0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x78",
            name: "EGT B1S2",
            reference_eq: "(256 * D + E) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 3,
            end_byte: 4,
            signed: false,
            test_bytes: &[0x0F, 0, 0, 3, 32, 0, 0, 0, 0],
            expected: 40.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x78",
            name: "EGT B1S3",
            reference_eq: "(256 * F + G) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 5,
            end_byte: 6,
            signed: false,
            test_bytes: &[0x0F, 0, 0, 0, 0, 10, 0, 0, 0],
            expected: 216.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x78",
            name: "EGT B1S4",
            reference_eq: "(256 * H + I) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 7,
            end_byte: 8,
            signed: false,
            test_bytes: &[0x0F, 0, 0, 0, 0, 0, 0, 0, 0],
            expected: -40.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x79: EGT Bank 2 — same layout as 0x78
        ReferencePidCase {
            pid: "0x79",
            name: "EGT B2S1",
            reference_eq: "(256 * B + C) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 1,
            end_byte: 2,
            signed: false,
            test_bytes: &[0x0F, 4, 176, 0, 0, 0, 0, 0, 0],
            expected: 80.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x79",
            name: "EGT B2S2",
            reference_eq: "(256 * D + E) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 3,
            end_byte: 4,
            signed: false,
            test_bytes: &[0x0F, 0, 0, 7, 208, 0, 0, 0, 0],
            expected: 160.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x79",
            name: "EGT B2S3",
            reference_eq: "(256 * F + G) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 5,
            end_byte: 6,
            signed: false,
            test_bytes: &[0x0F, 0, 0, 0, 0, 20, 0, 0, 0],
            expected: 472.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x79",
            name: "EGT B2S4",
            reference_eq: "(256 * H + I) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 7,
            end_byte: 8,
            signed: false,
            test_bytes: &[0x0F, 0, 0, 0, 0, 0, 0, 1, 144],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x7C: DPF temperature — A=support, same formula as catalyst/EGT: (256*B+C)/10-40
        ReferencePidCase {
            pid: "0x7C",
            name: "DPF temp sensor 1",
            reference_eq: "(256 * B + C) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 1,
            end_byte: 2,
            signed: false,
            test_bytes: &[0x03, 6, 64, 0, 0, 0, 0, 0, 0],
            expected: 120.0,
            tolerance: DEFAULT_TOL,
        },
        ReferencePidCase {
            pid: "0x7C",
            name: "DPF temp sensor 2",
            reference_eq: "(256 * D + E) / 10 - 40",
            scale: 0.1,
            offset: -40.0,
            start_byte: 3,
            end_byte: 4,
            signed: false,
            test_bytes: &[0x03, 0, 0, 12, 128, 0, 0, 0, 0],
            expected: 280.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x7F: Engine run time — A=support, total=B*2^24+C*2^16+D*2^8+E
        ReferencePidCase {
            pid: "0x7F",
            name: "Engine run time total",
            reference_eq: "B*2^24 + C*2^16 + D*2^8 + E",
            scale: 1.0,
            offset: 0.0,
            start_byte: 1,
            end_byte: 4,
            signed: false,
            test_bytes: &[0x01, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            expected: 65536.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x85: NOx reagent system — F = reagent level (100/255 * F)
        ReferencePidCase {
            pid: "0x85",
            name: "NOx reagent level",
            reference_eq: "F * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 5,
            end_byte: 5,
            signed: false,
            test_bytes: &[0, 0, 0, 0, 0, 128, 0, 0, 0, 0],
            expected: 50.19607843137255,
            tolerance: DEFAULT_TOL,
        },
        // 0x9B: Diesel exhaust fluid sensor — D = DEF concentration (100/255 * D)
        ReferencePidCase {
            pid: "0x9B",
            name: "DEF concentration",
            reference_eq: "D * 100 / 255",
            scale: 100.0 / 255.0,
            offset: 0.0,
            start_byte: 3,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 0, 85],
            expected: 33.33333333333333,
            tolerance: DEFAULT_TOL,
        },
        // 0xA4: Transmission actual gear — ratio = (256*C+D)/1000
        ReferencePidCase {
            pid: "0xA4",
            name: "Transmission gear ratio",
            reference_eq: "(256 * C + D) / 1000",
            scale: 0.001,
            offset: 0.0,
            start_byte: 2,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 3, 232],
            expected: 1.0,
            tolerance: DEFAULT_TOL,
        },
        // 0xA5: Commanded DEF dosing — B/2
        ReferencePidCase {
            pid: "0xA5",
            name: "Commanded DEF dosing",
            reference_eq: "B / 2",
            scale: 0.5,
            offset: 0.0,
            start_byte: 1,
            end_byte: 1,
            signed: false,
            test_bytes: &[0x01, 100, 0, 0],
            expected: 50.0,
            tolerance: DEFAULT_TOL,
        },
    ]
}

// Tier 3: Signed two's complement PIDs
fn tier3_cases() -> Vec<ReferencePidCase> {
    vec![
        // 0x32: Evap vapor pressure = signed(256*A+B) / 4
        // Positive: A=0x00, B=0x64 → 100/4 = 25.0
        ReferencePidCase {
            pid: "0x32",
            name: "Evap vapor pressure (positive)",
            reference_eq: "signed(256 * A + B) / 4",
            scale: 0.25,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: true,
            test_bytes: &[0x00, 0x64],
            expected: 25.0,
            tolerance: DEFAULT_TOL,
        },
        // Negative: A=0xFF, B=0x9C → unsigned=65436, signed=65436-65536=-100, /4 = -25.0
        ReferencePidCase {
            pid: "0x32",
            name: "Evap vapor pressure (negative)",
            reference_eq: "signed(256 * A + B) / 4",
            scale: 0.25,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: true,
            test_bytes: &[0xFF, 0x9C],
            expected: -25.0,
            tolerance: DEFAULT_TOL,
        },
        // 0x54: Evap vapor pressure = signed(256*A+B)
        // Positive: A=0x00, B=0x0A → 10
        ReferencePidCase {
            pid: "0x54",
            name: "Evap vapor pressure abs (positive)",
            reference_eq: "signed(256 * A + B)",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: true,
            test_bytes: &[0x00, 0x0A],
            expected: 10.0,
            tolerance: DEFAULT_TOL,
        },
        // Negative: A=0xFF, B=0xF6 → unsigned=65526, signed=-10
        ReferencePidCase {
            pid: "0x54",
            name: "Evap vapor pressure abs (negative)",
            reference_eq: "signed(256 * A + B)",
            scale: 1.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: true,
            test_bytes: &[0xFF, 0xF6],
            expected: -10.0,
            tolerance: DEFAULT_TOL,
        },
        // Zero
        ReferencePidCase {
            pid: "0x32",
            name: "Evap vapor pressure (zero)",
            reference_eq: "signed(256 * A + B) / 4",
            scale: 0.25,
            offset: 0.0,
            start_byte: 0,
            end_byte: 1,
            signed: true,
            test_bytes: &[0x00, 0x00],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
    ]
}

// Tier 4: Multi-byte (4-byte) PIDs
fn tier4_cases() -> Vec<ReferencePidCase> {
    vec![
        // 0xA6: Odometer = (A*2^24 + B*2^16 + C*2^8 + D) / 10
        ReferencePidCase {
            pid: "0xA6",
            name: "Odometer",
            reference_eq: "(A * 2^24 + B * 2^16 + C * 2^8 + D) / 10",
            scale: 0.1,
            offset: 0.0,
            start_byte: 0,
            end_byte: 3,
            signed: false,
            test_bytes: &[0x00, 0x01, 0x00, 0x00],
            expected: 6553.6,
            tolerance: DEFAULT_TOL,
        },
        // Odometer zero
        ReferencePidCase {
            pid: "0xA6",
            name: "Odometer (zero)",
            reference_eq: "(A * 2^24 + B * 2^16 + C * 2^8 + D) / 10",
            scale: 0.1,
            offset: 0.0,
            start_byte: 0,
            end_byte: 3,
            signed: false,
            test_bytes: &[0, 0, 0, 0],
            expected: 0.0,
            tolerance: DEFAULT_TOL,
        },
        // Odometer max (~429496729.5 km)
        ReferencePidCase {
            pid: "0xA6",
            name: "Odometer (max)",
            reference_eq: "(A * 2^24 + B * 2^16 + C * 2^8 + D) / 10",
            scale: 0.1,
            offset: 0.0,
            start_byte: 0,
            end_byte: 3,
            signed: false,
            test_bytes: &[0xFF, 0xFF, 0xFF, 0xFF],
            expected: 429496729.5,
            tolerance: 0.1,
        },
        // 0x50: Max MAF rate = A * 10 (only byte A)
        ReferencePidCase {
            pid: "0x50",
            name: "Max MAF rate",
            reference_eq: "A * 10",
            scale: 10.0,
            offset: 0.0,
            start_byte: 0,
            end_byte: 0,
            signed: false,
            test_bytes: &[65],
            expected: 650.0,
            tolerance: DEFAULT_TOL,
        },
    ]
}

fn untestable_pids() -> Vec<UntestablePid> {
    vec![
        // PID support bitmaps
        UntestablePid {
            pid: "0x00",
            name: "PIDs supported [01-20]",
            reason: "PID support bitmap",
        },
        UntestablePid {
            pid: "0x20",
            name: "PIDs supported [21-40]",
            reason: "PID support bitmap",
        },
        UntestablePid {
            pid: "0x40",
            name: "PIDs supported [41-60]",
            reason: "PID support bitmap",
        },
        UntestablePid {
            pid: "0x60",
            name: "PIDs supported [61-80]",
            reason: "PID support bitmap",
        },
        UntestablePid {
            pid: "0x80",
            name: "PIDs supported [81-A0]",
            reason: "PID support bitmap",
        },
        UntestablePid {
            pid: "0xA0",
            name: "PIDs supported [A1-C0]",
            reason: "PID support bitmap",
        },
        UntestablePid {
            pid: "0xC0",
            name: "PIDs supported [C1-E0]",
            reason: "PID support bitmap",
        },
        // Bit-encoded / enumerated
        UntestablePid {
            pid: "0x01",
            name: "Monitor status since DTCs cleared",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x02",
            name: "Freeze DTC",
            reason: "DTC encoding",
        },
        UntestablePid {
            pid: "0x03",
            name: "Fuel system status",
            reason: "Enumerated",
        },
        UntestablePid {
            pid: "0x12",
            name: "Commanded secondary air status",
            reason: "Enumerated",
        },
        UntestablePid {
            pid: "0x13",
            name: "O2 sensors present (2 banks)",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x1C",
            name: "OBD standards conformance",
            reason: "Enumerated",
        },
        UntestablePid {
            pid: "0x1D",
            name: "O2 sensors present (4 banks)",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x1E",
            name: "Auxiliary input status",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x41",
            name: "Monitor status this drive cycle",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x4F",
            name: "Max values for ER/O2V/O2C/MAP",
            reason: "Multi-value (4 independent scales)",
        },
        UntestablePid {
            pid: "0x51",
            name: "Fuel type",
            reason: "Enumerated",
        },
        UntestablePid {
            pid: "0x5F",
            name: "Emission requirements",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x65",
            name: "Auxiliary input/output supported",
            reason: "Bit-encoded",
        },
        // Complex / no public formula
        UntestablePid {
            pid: "0x69",
            name: "Actual EGR / Commanded EGR / EGR Error",
            reason: "Complex multi-value, no public formula",
        },
        UntestablePid {
            pid: "0x6A",
            name: "Commanded diesel intake air flow",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x6B",
            name: "EGR temperature",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x6C",
            name: "Commanded throttle actuator / rel throttle pos",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x6D",
            name: "Fuel pressure control system",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x6E",
            name: "Injection pressure control system",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x6F",
            name: "Turbocharger compressor inlet pressure",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x70",
            name: "Boost pressure control",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x71",
            name: "Variable geometry turbo control",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x72",
            name: "Wastegate control",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x73",
            name: "Exhaust pressure",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x74",
            name: "Turbocharger RPM",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x75",
            name: "Turbocharger temperature A",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x76",
            name: "Turbocharger temperature B",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x77",
            name: "Charge air cooler temperature (CACT)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x7A",
            name: "DPF differential pressure",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x7B",
            name: "Diesel particulate filter",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x7D",
            name: "NOx NTE control area status",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x7E",
            name: "PM NTE control area status",
            reason: "Bit-encoded",
        },
        UntestablePid {
            pid: "0x81",
            name: "Engine run time for AECD #1-#5",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x82",
            name: "Engine run time for AECD #6-#10",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x83",
            name: "NOx sensor",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x84",
            name: "Manifold surface temperature",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x86",
            name: "Particulate matter sensor",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x87",
            name: "Intake manifold absolute pressure (extended)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x88",
            name: "SCR induce system",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x89",
            name: "Run time for AECD #11-#15",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x8A",
            name: "Run time for AECD #16-#20",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x8B",
            name: "Diesel aftertreatment",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x8C",
            name: "O2 sensor wide range",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x8D",
            name: "Throttle position G",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x8F",
            name: "PM sensor bank 1 & 2",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x90",
            name: "WWH-OBD vehicle OBD system info",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x91",
            name: "WWH-OBD vehicle OBD system info (2)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x92",
            name: "Fuel system control",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x93",
            name: "WWH-OBD counters support",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x94",
            name: "NOx warning and inducement system",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x98",
            name: "EGT sensor bank 1 (extended)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x99",
            name: "EGT sensor bank 2 (extended)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x9A",
            name: "Hybrid/EV vehicle system data",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x9C",
            name: "O2 sensor data (extended)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x9D",
            name: "Engine fuel rate (extended)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x9E",
            name: "Engine exhaust flow rate",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0x9F",
            name: "Fuel system percentage use",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xA1",
            name: "NOx sensor corrected data",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xA3",
            name: "Evap system vapor pressure (extended)",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xA7",
            name: "NOx sensor concentration S3/S4",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xA8",
            name: "NOx sensor corrected concentration S3/S4",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xA9",
            name: "ABS disable switch state",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xC3",
            name: "Fuel level input A/B",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xC4",
            name: "Exhaust particulate control diagnostic",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xC5",
            name: "Fuel pressure A and B",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xC6",
            name: "Particulate control inducement",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xC7",
            name: "Distance since reflash/module replacement",
            reason: "No public formula",
        },
        UntestablePid {
            pid: "0xC8",
            name: "NOx/PCD warning lamp status",
            reason: "Bit-encoded",
        },
    ]
}

fn run_cases(
    evaluator: &mut ExpressionEvaluator,
    cases: &[ReferencePidCase],
) -> (usize, Vec<String>) {
    let variables: HashMap<String, f64> = HashMap::new();
    let mut pass = 0;
    let mut failures = Vec::new();

    for case in cases {
        let eq = generate_equation(
            case.scale,
            case.offset,
            case.start_byte,
            case.end_byte,
            case.signed,
        )
        .unwrap_or_else(|| {
            panic!(
                "[{}] {} — generate_equation returned None",
                case.pid, case.name
            )
        });

        let result = evaluator
            .evaluate_with_bytes(&eq, &variables, case.test_bytes)
            .unwrap_or_else(|e| {
                panic!(
                    "[{}] {} — evaluate failed: {} (eq: {})",
                    case.pid, case.name, e, eq
                )
            });

        let diff = (result - case.expected).abs();
        if diff < case.tolerance {
            pass += 1;
        } else {
            failures.push(format!(
                "[{}] {} — expected {}, got {} (diff={}, eq: {})",
                case.pid, case.name, case.expected, result, diff, eq
            ));
        }
    }

    (pass, failures)
}

#[test]
fn test_tier1_core_pids() {
    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let cases = tier1_cases();
    let total = cases.len();
    let (pass, failures) = run_cases(&mut evaluator, &cases);

    if !failures.is_empty() {
        panic!(
            "Tier 1: {}/{} passed. Failures:\n{}",
            pass,
            total,
            failures.join("\n")
        );
    }
    assert_eq!(pass, total, "Tier 1: all {} cases should pass", total);
}

#[test]
fn test_tier2_multi_datapoint_pids() {
    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let cases = tier2_cases();
    let total = cases.len();
    let (pass, failures) = run_cases(&mut evaluator, &cases);

    if !failures.is_empty() {
        panic!(
            "Tier 2: {}/{} passed. Failures:\n{}",
            pass,
            total,
            failures.join("\n")
        );
    }
    assert_eq!(pass, total, "Tier 2: all {} cases should pass", total);
}

#[test]
fn test_tier3_signed_pids() {
    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let cases = tier3_cases();
    let total = cases.len();
    let (pass, failures) = run_cases(&mut evaluator, &cases);

    if !failures.is_empty() {
        panic!(
            "Tier 3: {}/{} passed. Failures:\n{}",
            pass,
            total,
            failures.join("\n")
        );
    }
    assert_eq!(pass, total, "Tier 3: all {} cases should pass", total);
}

#[test]
fn test_tier4_multibyte_pids() {
    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let cases = tier4_cases();
    let total = cases.len();
    let (pass, failures) = run_cases(&mut evaluator, &cases);

    if !failures.is_empty() {
        panic!(
            "Tier 4: {}/{} passed. Failures:\n{}",
            pass,
            total,
            failures.join("\n")
        );
    }
    assert_eq!(pass, total, "Tier 4: all {} cases should pass", total);
}

// Boundary tests: all-zero and all-0xFF for a subset of representative PIDs
#[test]
fn test_boundary_all_zeros() {
    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let variables: HashMap<String, f64> = HashMap::new();

    let boundary_cases: Vec<(&str, f64, f64, i32, i32, bool, &[u8], f64)> = vec![
        // (name, scale, offset, start, end, signed, bytes, expected)
        ("Coolant temp zero", 1.0, -40.0, 0, 0, false, &[0u8], -40.0),
        ("RPM zero", 0.25, 0.0, 0, 1, false, &[0, 0], 0.0),
        ("Fuel tank zero", 100.0 / 255.0, 0.0, 0, 0, false, &[0], 0.0),
        ("Signed evap zero", 0.25, 0.0, 0, 1, true, &[0, 0], 0.0),
        ("Odometer zero", 0.1, 0.0, 0, 3, false, &[0, 0, 0, 0], 0.0),
    ];

    for (name, scale, offset, start, end, signed, bytes, expected) in &boundary_cases {
        let eq = generate_equation(*scale, *offset, *start, *end, *signed).unwrap();
        let result = evaluator
            .evaluate_with_bytes(&eq, &variables, bytes)
            .unwrap();
        assert!(
            (result - expected).abs() < DEFAULT_TOL,
            "{}: expected {}, got {} (eq: {})",
            name,
            expected,
            result,
            eq
        );
    }
}

#[test]
fn test_boundary_all_ff() {
    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let variables: HashMap<String, f64> = HashMap::new();

    let boundary_cases: Vec<(&str, f64, f64, i32, i32, bool, &[u8], f64, f64)> = vec![
        // (name, scale, offset, start, end, signed, bytes, expected, tolerance)
        (
            "Coolant temp max",
            1.0,
            -40.0,
            0,
            0,
            false,
            &[0xFF],
            215.0,
            DEFAULT_TOL,
        ),
        (
            "RPM max",
            0.25,
            0.0,
            0,
            1,
            false,
            &[0xFF, 0xFF],
            16383.75,
            DEFAULT_TOL,
        ),
        (
            "Load max",
            100.0 / 255.0,
            0.0,
            0,
            0,
            false,
            &[0xFF],
            100.0,
            DEFAULT_TOL,
        ),
        (
            "Speed max",
            1.0,
            0.0,
            0,
            0,
            false,
            &[0xFF],
            255.0,
            DEFAULT_TOL,
        ),
        (
            "Signed16 0xFFFF",
            0.25,
            0.0,
            0,
            1,
            true,
            &[0xFF, 0xFF],
            -0.25,
            DEFAULT_TOL,
        ),
    ];

    for (name, scale, offset, start, end, signed, bytes, expected, tol) in &boundary_cases {
        let eq = generate_equation(*scale, *offset, *start, *end, *signed).unwrap();
        let result = evaluator
            .evaluate_with_bytes(&eq, &variables, bytes)
            .unwrap();
        assert!(
            (result - expected).abs() < *tol,
            "{}: expected {}, got {} (eq: {})",
            name,
            expected,
            result,
            eq
        );
    }
}

// Roundtrip: verify generated equation strings have expected structure
#[test]
fn test_generator_equation_structure() {
    // RPM: 0.25 * (A*256 + B)
    let eq = generate_equation(0.25, 0.0, 0, 1, false).unwrap();
    assert!(eq.contains("A*256"), "RPM eq should contain A*256: {}", eq);
    assert!(eq.contains("B"), "RPM eq should contain B: {}", eq);

    // Coolant temp: A + -40
    let eq = generate_equation(1.0, -40.0, 0, 0, false).unwrap();
    assert!(eq.contains("A"), "Coolant eq should contain A: {}", eq);
    assert!(eq.contains("-40"), "Coolant eq should contain -40: {}", eq);

    // Signed 16-bit: should use SIGNED16
    let eq = generate_equation(0.25, 0.0, 0, 1, true).unwrap();
    assert!(
        eq.contains("SIGNED16"),
        "Signed 16-bit eq should use SIGNED16: {}",
        eq
    );

    // 4-byte odometer: A*16777216 + B*65536 + C*256 + D
    let eq = generate_equation(0.1, 0.0, 0, 3, false).unwrap();
    assert!(
        eq.contains("16777216"),
        "4-byte eq should contain 2^24: {}",
        eq
    );
    assert!(
        eq.contains("65536"),
        "4-byte eq should contain 2^16: {}",
        eq
    );
    assert!(eq.contains("D"), "4-byte eq should contain D: {}", eq);

    // O2 sensor bytes 2-3: should reference C and D
    let eq = generate_equation(8.0 / 65536.0, 0.0, 2, 3, false).unwrap();
    assert!(
        eq.contains("C*256"),
        "Bytes 2-3 eq should contain C*256: {}",
        eq
    );
    assert!(eq.contains("D"), "Bytes 2-3 eq should contain D: {}", eq);
}

#[test]
fn export_results_csv() {
    use std::io::Write;

    let mut evaluator = ExpressionEvaluator::new().expect("Failed to create evaluator");
    let variables: HashMap<String, f64> = HashMap::new();

    let all_cases: Vec<(&str, Vec<ReferencePidCase>)> = vec![
        ("Tier1-Core", tier1_cases()),
        ("Tier2-MultiDatapoint", tier2_cases()),
        ("Tier3-Signed", tier3_cases()),
        ("Tier4-MultiByte", tier4_cases()),
    ];

    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/output");
    std::fs::create_dir_all(&out_dir).expect("Failed to create output dir");
    let csv_path = out_dir.join("reference_pid_results.csv");
    let mut file = std::fs::File::create(&csv_path).expect("Failed to create CSV");

    writeln!(file, "Tier,PID,Name,Scale,Offset,StartByte,EndByte,Signed,TestBytes,ReferenceEquation,GeneratedEquation,Expected,Actual,Diff,Tolerance,Result")
        .unwrap();

    let mut total = 0;
    let mut passed = 0;

    for (tier, cases) in &all_cases {
        for case in cases {
            total += 1;
            let eq = generate_equation(
                case.scale,
                case.offset,
                case.start_byte,
                case.end_byte,
                case.signed,
            )
            .unwrap_or_else(|| {
                panic!(
                    "[{}] {} — generate_equation returned None",
                    case.pid, case.name
                )
            });

            let result = evaluator
                .evaluate_with_bytes(&eq, &variables, case.test_bytes)
                .unwrap_or_else(|e| {
                    panic!(
                        "[{}] {} — evaluate failed: {} (eq: {})",
                        case.pid, case.name, e, eq
                    )
                });

            let diff = (result - case.expected).abs();
            let status = if diff < case.tolerance {
                passed += 1;
                "PASS"
            } else {
                "FAIL"
            };
            let bytes_str: Vec<String> = case
                .test_bytes
                .iter()
                .map(|b| format!("{:#04X}", b))
                .collect();

            writeln!(
                file,
                "{},{},{},{},{},{},{},{},{},\"{}\",\"{}\",{},{},{},{},{}",
                tier,
                case.pid,
                case.name,
                case.scale,
                case.offset,
                case.start_byte,
                case.end_byte,
                case.signed,
                bytes_str.join(" "),
                case.reference_eq,
                eq,
                case.expected,
                result,
                diff,
                case.tolerance,
                status
            )
            .unwrap();
        }
    }

    // Append untestable PIDs
    let untestable = untestable_pids();
    let skipped = untestable.len();
    for pid in &untestable {
        writeln!(
            file,
            "Untestable,{},{},,,,,,,\"{}\",N/A,,,,,SKIPPED ({})",
            pid.pid, pid.name, pid.reason, pid.reason
        )
        .unwrap();
    }

    println!(
        "\n=== Reference PID Results: {}/{} passed, {} skipped (no formula) ===",
        passed, total, skipped
    );
    println!("CSV written to: {}", csv_path.display());
}
