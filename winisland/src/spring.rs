//! A real spring (damped harmonic oscillator), integrated per frame. Animations only run while unsettled.

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub pos: f32,
    pub vel: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub fn new(v: f32) -> Self {
        Spring { pos: v, vel: 0.0, target: v, stiffness: 210.0, damping: 21.0 }
    }
    pub fn with(mut self, stiffness: f32, damping: f32) -> Self {
        self.stiffness = stiffness;
        self.damping = damping;
        self
    }
    pub fn set(&mut self, t: f32) {
        self.target = t;
    }
    pub fn snap(&mut self, t: f32) {
        self.target = t;
        self.pos = t;
        self.vel = 0.0;
    }
    /// Advance by `dt` seconds (sub-stepped for stability). Returns true while still moving.
    pub fn step(&mut self, dt: f32) -> bool {
        if self.settled() {
            self.pos = self.target;
            self.vel = 0.0;
            return false;
        }
        let steps = ((dt / (1.0 / 240.0)).ceil() as i32).clamp(1, 32);
        let h = dt / steps as f32;
        for _ in 0..steps {
            let a = self.stiffness * (self.target - self.pos) - self.damping * self.vel;
            self.vel += a * h;
            self.pos += self.vel * h;
        }
        if self.settled() {
            self.pos = self.target;
            self.vel = 0.0;
            return false;
        }
        true
    }
    pub fn settled(&self) -> bool {
        let scale = self.target.abs().max(1.0);
        (self.target - self.pos).abs() < 0.0015 * scale.min(40.0) && self.vel.abs() < 0.02 * scale.min(40.0)
    }
}

/// Simple time-based easing for fades.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
