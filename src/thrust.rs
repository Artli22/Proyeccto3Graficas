use raylib::prelude::*;

pub struct ThrustParticle {
    pub position: Vector3,
    pub velocity: Vector3,
    pub age: f32,
    pub lifetime: f32,
    pub size_factor: f32,
}

pub struct ThrustEngine {
    particles: Vec<ThrustParticle>,
    emit_accumulator: f32,
    rng_state: u32,
    pub intensity: f32,
    pub light_position: Vector3,
}

impl ThrustEngine {
    pub fn new() -> Self {
        ThrustEngine {
            particles: Vec::with_capacity(4000),
            emit_accumulator: 0.0,
            rng_state: 0x9E37_79B9,
            intensity: 0.3,
            light_position: Vector3::zero(),
        }
    }

    fn rand(&mut self) -> f32 {
        // xorshift32
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng_state = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    // `nozzle_pos` está en el espacio del modelo (centrado); `nozzle_dir` es la salida del gas.
    pub fn update(&mut self, delta_time: f32, nozzle_pos: Vector3, nozzle_dir: Vector3, nozzle_radius: f32) {
        let dt = delta_time.min(0.05);
        self.light_position = nozzle_pos + nozzle_dir * 1.0;

        let drag = 0.97_f32.powf(dt * 60.0);
        self.particles.retain_mut(|p| {
            p.age += dt;
            p.position += p.velocity * dt;
            p.velocity *= drag;
            p.age < p.lifetime
        });

        let eje = nozzle_dir.normalized();
        let ref_axis = if eje.y.abs() < 0.9 { Vector3::new(0.0, 1.0, 0.0) } else { Vector3::new(1.0, 0.0, 0.0) };
        let u = eje.cross(ref_axis).normalized();
        let v = eje.cross(u);

        self.emit_accumulator += 3500.0 * self.intensity * dt;
        let nuevas = self.emit_accumulator as usize;
        self.emit_accumulator -= nuevas as f32;

        for _ in 0..nuevas {
            let angulo = self.rand() * std::f32::consts::TAU;
            let r = self.rand().sqrt() * nozzle_radius;
            let lateral = u * angulo.cos() + v * angulo.sin();

            // Nace justo fuera de la cara del motor para no solaparse con el modelo.
            let position = nozzle_pos + eje * 0.15 + lateral * r;
            let velocidad = (9.0 + self.rand() * 10.0) * (0.4 + 0.6 * self.intensity);
            let velocity = eje * velocidad + lateral * (self.rand() * 1.5);

            let lifetime = 1.0 + self.rand() * 0.9;
            let size_factor = 0.8 + self.rand() * 0.5;
            self.particles.push(ThrustParticle { position, velocity, age: 0.0, lifetime, size_factor });
        }
    }

    pub fn get_particles(&self) -> &[ThrustParticle] {
        &self.particles
    }

    // Radio en unidades del mundo: crece con la edad (llama -> nube de humo).
    pub fn get_particle_radius(&self, particle: &ThrustParticle) -> f32 {
        let progress = particle.age / particle.lifetime;
        (0.22 + progress * 0.28) * particle.size_factor
    }

    pub fn get_particle_color(&self, particle: &ThrustParticle) -> Color {
        let progress = particle.age / particle.lifetime;

        let (r, g, b) = if progress < 0.3 {
            let t = progress / 0.3;
            (255, (225.0 * (1.0 - t * 0.35)) as u8, (40.0 * (1.0 - t)) as u8) // Naranja brillante
        } else if progress < 0.7 {
            let t = (progress - 0.3) / 0.4;
            (255, (145.0 * (1.0 - t)) as u8, (50.0 * t) as u8) // Naranja -> rojo
        } else {
            let t = (progress - 0.7) / 0.3;
            (200 - (100.0 * t) as u8, 100 - (80.0 * t) as u8, 80 - (60.0 * t) as u8) // Humo gris
        };

        let alpha = if progress < 0.5 {
            (1.0 - progress * 0.5) * 230.0
        } else {
            (1.0 - progress) * 230.0
        };

        Color::new(r, g, b, alpha.clamp(0.0, 255.0) as u8)
    }
}
