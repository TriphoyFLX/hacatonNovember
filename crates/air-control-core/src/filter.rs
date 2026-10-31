//! One Euro filter.
//!
//! Based on Casiez, Roussel, Vogel, CHI 2012. Higher cutoff reacts faster;
//! `beta` raises the cutoff when the signal is moving.

const PI: f32 = std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct OneEuroFilter {
    min_cutoff: f32,
    beta: f32,
    d_cutoff: f32,
    x_prev: Option<f32>,
    dx_prev: f32,
    t_prev: Option<f32>,
}

impl OneEuroFilter {
    pub fn new(min_cutoff: f32, beta: f32) -> Self {
        Self {
            min_cutoff: min_cutoff.max(0.01),
            beta: beta.max(0.0),
            d_cutoff: 1.0,
            x_prev: None,
            dx_prev: 0.0,
            t_prev: None,
        }
    }

    #[allow(dead_code)]
    pub fn set_params(&mut self, min_cutoff: f32, beta: f32) {
        self.min_cutoff = min_cutoff.max(0.01);
        self.beta = beta.max(0.0);
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.x_prev = None;
        self.dx_prev = 0.0;
        self.t_prev = None;
    }

    pub fn filter(&mut self, time_s: f32, value: f32) -> f32 {
        self.filter_velocity(time_s, value).0
    }

    /// Filtered value plus estimated velocity (units / second).
    pub fn filter_velocity(&mut self, time_s: f32, value: f32) -> (f32, f32) {
        if !value.is_finite() || !time_s.is_finite() {
            return (self.x_prev.unwrap_or(0.0), self.dx_prev);
        }
        let Some(t_prev) = self.t_prev else {
            self.t_prev = Some(time_s);
            self.x_prev = Some(value);
            self.dx_prev = 0.0;
            return (value, 0.0);
        };
        let dt = time_s - t_prev;
        if dt <= 1e-4 {
            return (self.x_prev.unwrap_or(value), self.dx_prev);
        }
        let x_prev = self.x_prev.unwrap_or(value);
        let dx = (value - x_prev) / dt;
        let alpha_d = alpha(self.d_cutoff, dt);
        let dx_hat = exponential(alpha_d, dx, self.dx_prev);
        let cutoff = self.min_cutoff + self.beta * dx_hat.abs();
        let alpha_x = alpha(cutoff, dt);
        let x_hat = exponential(alpha_x, value, x_prev);
        self.x_prev = Some(x_hat);
        self.dx_prev = dx_hat;
        self.t_prev = Some(time_s);
        (x_hat, dx_hat)
    }

    pub fn velocity(&self) -> f32 {
        self.dx_prev
    }
}

fn alpha(cutoff: f32, dt: f32) -> f32 {
    let tau = 1.0 / (2.0 * PI * cutoff.max(1e-3));
    1.0 / (1.0 + tau / dt)
}

/// FPS-independent EMA weight: `alpha = 1 - exp(-dt / tau)`.
pub fn ema_alpha(dt_s: f32, tau_s: f32) -> f32 {
    let dt = dt_s.max(1e-4);
    let tau = tau_s.max(1e-3);
    1.0 - (-dt / tau).exp()
}

pub fn ema(previous: f32, value: f32, alpha: f32) -> f32 {
    let a = alpha.clamp(0.0, 1.0);
    a * value + (1.0 - a) * previous
}

fn exponential(alpha: f32, value: f32, previous: f32) -> f32 {
    alpha * value + (1.0 - alpha) * previous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_signal_stays_put() {
        let mut filter = OneEuroFilter::new(1.2, 0.02);
        let mut last = 0.0;
        for step in 0..20 {
            last = filter.filter(step as f32 * 0.016, 10.0);
        }
        assert!((last - 10.0).abs() < 0.01);
    }

    #[test]
    fn step_moves_toward_target_without_nan() {
        let mut filter = OneEuroFilter::new(1.0, 0.05);
        let mut value = 0.0;
        for step in 0..40 {
            value = filter.filter(step as f32 * 0.016, 50.0);
            assert!(value.is_finite());
        }
        assert!(value > 20.0);
    }

    #[test]
    fn throughput() {
        let mut filter = OneEuroFilter::new(1.5, 0.04);
        let start = std::time::Instant::now();
        let mut sample = 0.0;
        for step in 0..100_000 {
            sample = filter.filter(step as f32 * 0.001, (step as f32 * 0.01).sin());
        }
        assert!(sample.is_finite());
        assert!(start.elapsed().as_millis() < 500);
    }
}
