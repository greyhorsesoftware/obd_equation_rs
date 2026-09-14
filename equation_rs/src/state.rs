use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Clone)]
pub struct FunctionState {
    pub ewma_values: HashMap<String, f64>,
    pub avg_buckets: HashMap<String, Vec<f64>>,
    pub delay_buffers: HashMap<String, Vec<f64>>,
    pub last_eval_time: HashMap<String, SystemTime>,
    pub delay_start: HashMap<String, (SystemTime, f64)>, // (start_time, initial_value)
    pub tot_state: HashMap<String, (f64, SystemTime)>,   // (accumulated_seconds, last_timestamp)
}

impl Default for FunctionState {
    fn default() -> Self {
        Self {
            ewma_values: HashMap::new(),
            avg_buckets: HashMap::new(),
            delay_buffers: HashMap::new(),
            last_eval_time: HashMap::new(),
            delay_start: HashMap::new(),
            tot_state: HashMap::new(),
        }
    }
}

impl FunctionState {
    /// TAVG: Time-weighted exponential moving average.
    /// alpha = 1 - e^(-dt / tau) where tau is the time constant in seconds.
    pub fn tavg(&mut self, tau: f64, value: f64, key: &str) -> f64 {
        let now = SystemTime::now();
        match self.last_eval_time.get(key).cloned() {
            None => {
                self.ewma_values.insert(key.to_string(), value);
                self.last_eval_time.insert(key.to_string(), now);
                value
            }
            Some(last_time) => {
                let dt = now
                    .duration_since(last_time)
                    .unwrap_or_default()
                    .as_secs_f64();
                let alpha = 1.0 - libm::exp(-dt / tau.max(0.001));
                let prev = *self.ewma_values.get(key).unwrap_or(&value);
                let result = alpha * value + (1.0 - alpha) * prev;
                self.ewma_values.insert(key.to_string(), result);
                self.last_eval_time.insert(key.to_string(), now);
                result
            }
        }
    }

    /// TDLY: Time-based delay. Holds the initial value for `delay` seconds.
    pub fn tdly(&mut self, delay: f64, value: f64, key: &str) -> f64 {
        let now = SystemTime::now();
        let entry = self
            .delay_start
            .entry(key.to_string())
            .or_insert_with(|| (now, value));
        let elapsed = now
            .duration_since(entry.0)
            .unwrap_or_default()
            .as_secs_f64();
        if elapsed < delay {
            entry.1
        } else {
            value
        }
    }

    /// RDLY: Sample-count delay. Returns the value from N samples ago.
    pub fn rdly(&mut self, samples: usize, value: f64, key: &str) -> f64 {
        let buf = self
            .delay_buffers
            .entry(key.to_string())
            .or_insert_with(Vec::new);
        let output = if buf.is_empty() { value } else { buf[0] };
        buf.push(value);
        if buf.len() > samples.max(1) {
            buf.remove(0);
        }
        output
    }

    /// TOT: Time on target. Accumulates seconds where value is non-zero.
    pub fn tot(&mut self, _window: f64, value: f64, key: &str) -> f64 {
        let now = SystemTime::now();
        let entry = self
            .tot_state
            .entry(key.to_string())
            .or_insert_with(|| (0.0, now));
        let dt = now
            .duration_since(entry.1)
            .unwrap_or_default()
            .as_secs_f64();
        if value != 0.0 {
            entry.0 += dt;
        }
        entry.1 = now;
        entry.0
    }
}
