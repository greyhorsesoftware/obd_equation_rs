/// Platform-specific callbacks for features that require native access
pub trait PlatformCallbacks {
    /// Get barometric pressure for BARO function
    /// - iOS: Returns real pressure from CMAltimeter (converted hPa → PSI)
    /// - macOS/other: Returns 0.0 or platform-appropriate default
    fn get_barometric_pressure(&self) -> f64;

    /// Get the current value of a PID by name
    /// Used for cross-PID references like val{PID_NAME}
    /// Returns None if the PID is not found or has no current value
    fn get_pid_value(&self, pid_name: &str) -> Option<f64>;
}

/// Default platform callbacks that return safe defaults
pub struct DefaultPlatformCallbacks;

impl PlatformCallbacks for DefaultPlatformCallbacks {
    fn get_barometric_pressure(&self) -> f64 {
        0.0 // Default pressure (no sensor available)
    }

    fn get_pid_value(&self, _pid_name: &str) -> Option<f64> {
        None // Default: no PID values available
    }
}
