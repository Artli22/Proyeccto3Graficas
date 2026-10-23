// Estado del laser. Su geometria y renderizado GPU estan en gpu.rs.
pub struct Laser {
    pub potencia: f32,
    tiempo: f32,
}
impl Laser {
    pub fn new() -> Self {
        Self {
            potencia: 0.3,
            tiempo: 0.0,
        }
    }
    pub fn update(&mut self, delta_time: f32, objetivo: f32) {
        self.tiempo += delta_time;
        let k = (delta_time * 10.0).min(1.0);
        self.potencia += (objetivo - self.potencia) * k;
    }
    pub fn brillo(&self) -> f32 {
        (self.potencia * (1.0 + 0.06 * (self.tiempo * 55.0).sin())).clamp(0.0, 1.0)
    }
}