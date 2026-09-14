use obd_equation_rs::{ExpressionEvaluator, PlatformCallbacks};
use std::collections::HashMap;

/// Example platform callbacks for iOS (with CoreMotion simulation)
struct IOSPlatformCallbacks {
    simulated_pressure_hpa: f64,
}

impl IOSPlatformCallbacks {
    fn new(pressure_hpa: f64) -> Self {
        Self {
            simulated_pressure_hpa: pressure_hpa,
        }
    }
}

impl PlatformCallbacks for IOSPlatformCallbacks {
    fn get_barometric_pressure(&self) -> f64 {
        // Convert hPa to PSI as per OBD-II specification
        // 1 hPa = 0.0145037738 PSI
        self.simulated_pressure_hpa * 0.0145037738
    }

    fn get_pid_value(&self, _pid_name: &str) -> Option<f64> {
        // In a real implementation, this would query the OBD-II system
        None
    }
}

/// Example platform callbacks for macOS
struct MacOSPlatformCallbacks;

impl PlatformCallbacks for MacOSPlatformCallbacks {
    fn get_barometric_pressure(&self) -> f64 {
        // macOS doesn't have barometric pressure sensor, return 0
        0.0
    }

    fn get_pid_value(&self, _pid_name: &str) -> Option<f64> {
        // In a real implementation, this would query the OBD-II system
        None
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 obd_equation_rs Callback System Demo");
    println!("=======================================");

    // Example 1: Default platform callbacks
    println!("\n📱 Example 1: Default Platform Callbacks");
    let mut evaluator = ExpressionEvaluator::new()?;
    let result = evaluator.evaluate("BARO()", &HashMap::new())?;
    println!("BARO() with default callbacks: {} PSI", result);

    // Example 2: iOS platform callbacks with simulated pressure
    println!("\n📱 Example 2: iOS Platform Callbacks (1013.25 hPa = sea level)");
    let ios_callbacks = IOSPlatformCallbacks::new(1013.25); // Sea level pressure
    let mut ios_evaluator =
        ExpressionEvaluator::new_with_platform_callbacks(Box::new(ios_callbacks))?;
    let result = ios_evaluator.evaluate("BARO()", &HashMap::new())?;
    println!("BARO() on iOS (sea level): {:.2} PSI", result);

    // Example 3: macOS platform callbacks
    println!("\n💻 Example 3: macOS Platform Callbacks");
    let macos_callbacks = MacOSPlatformCallbacks;
    let mut macos_evaluator =
        ExpressionEvaluator::new_with_platform_callbacks(Box::new(macos_callbacks))?;
    let result = macos_evaluator.evaluate("BARO()", &HashMap::new())?;
    println!("BARO() on macOS: {} PSI (no sensor)", result);

    // Example 4: Using expressions with platform features
    println!("\n🔧 Example 4: Complex Expression with Platform Features");
    let ios_callbacks = IOSPlatformCallbacks::new(990.0); // High altitude pressure
    let mut evaluator = ExpressionEvaluator::new_with_platform_callbacks(Box::new(ios_callbacks))?;

    let mut variables = HashMap::new();
    variables.insert("engine_load".to_string(), 75.0);
    variables.insert("rpm".to_string(), 2500.0);

    // Simulate an expression that uses barometric pressure for altitude correction
    let result = evaluator.evaluate("engine_load * (BARO() / 14.7)", &variables)?;
    println!("Engine load corrected for altitude: {:.2}", result);

    println!("\n🎉 Callback system demonstration complete!");
    println!("🔧 Features demonstrated:");
    println!("   • Platform-specific callbacks (BARO function)");
    println!("   • Default vs custom platform implementations");
    println!("   • Integration with expression evaluation");

    Ok(())
}
