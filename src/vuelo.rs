//! Estado de vuelo y camara de seguimiento, calculados exclusivamente en CPU.
use crate::pipeline::Mat4;
use raylib::prelude::Vector3;

/// Base ortonormal que reemplaza a yaw/pitch mientras la nave esta posada (right x forward = up).
#[derive(Clone, Copy)]
pub struct Base {
    pub right: Vector3,
    pub up: Vector3,
    pub forward: Vector3,
}

pub struct Nave {
    pub position: Vector3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vector3,
    pub speed_kmh: f32,
    pub base: Option<Base>,
}
impl Nave {
    pub fn new() -> Self {
        Self {
            position: Vector3::zero(),
            yaw: 0.,
            pitch: 0.,
            velocity: Vector3::zero(),
            speed_kmh: 0.,
            base: None,
        }
    }
    pub fn forward(&self) -> Vector3 {
        if let Some(b) = self.base {
            return b.forward;
        }
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vector3::new(sy * cp, sp, -cy * cp)
    }
    pub fn right(&self) -> Vector3 {
        if let Some(b) = self.base {
            return b.right;
        }
        Vector3::new(self.yaw.cos(), 0., self.yaw.sin())
    }
    pub fn up(&self) -> Vector3 {
        if let Some(b) = self.base {
            return b.up;
        }
        self.right().cross(self.forward()).normalized()
    }
    /// Fija la velocidad (km/h) y la velocidad de mundo coherente con ella.
    pub fn set_speed(&mut self, kmh: f32) {
        self.speed_kmh = kmh.max(0.);
        self.velocity = self.forward() * (self.speed_kmh / 3.6 / Self::METERS_PER_UNIT);
    }
    /// Sale del modo posado conservando el rumbo actual como yaw/pitch.
    pub fn liberar_base(&mut self, forward: Vector3) {
        self.yaw = forward.x.atan2(-forward.z).rem_euclid(std::f32::consts::TAU);
        self.pitch = forward.y.clamp(-1., 1.).asin().clamp(-1.35, 1.35);
        self.base = None;
    }
    pub const MAX_SPEED_KMH: f32 = 300_000.;
    pub const LASER_THRESHOLD_KMH: f32 = 10_000.;
    // Escala explicita: el velocimetro corresponde al desplazamiento del mundo.
    pub const METERS_PER_UNIT: f32 = 100.;
    pub const NORMAL_ACCEL: f32 = 4_000.; // km/h ganados por segundo al 100%
    pub const LASER_ACCEL: f32 = 60_000.;
    pub const BRAKING: f32 = 60_000.;

    pub fn laser_mode(&self) -> bool {
        self.speed_kmh > Self::LASER_THRESHOLD_KMH
    }

    /// Integra por tramos, incluidos los cruces de 1000, 28000 y la parada.
    /// Retorna velocidad final (km/h) y distancia realmente recorrida (km).
    pub fn integrate_speed(initial: f32, power: f32, dt: f32) -> (f32, f32) {
        let mut speed = initial.clamp(0., Self::MAX_SPEED_KMH);
        let mut remaining = dt.max(0.);
        let mut distance = 0.;
        let power = power.clamp(0., 1.);
        if power <= 0. {
            let time = remaining.min(speed / Self::BRAKING);
            let end = (speed - Self::BRAKING * time).max(0.);
            return (end, (speed + end) * 0.5 * time / 3600.);
        }
        // A lo sumo: tramo normal, tramo laser y crucero al limite.
        for _ in 0..3 {
            if remaining <= 0. {
                break;
            }
            if speed >= Self::MAX_SPEED_KMH {
                distance += speed * remaining / 3600.;
                break;
            }
            let (boundary, accel) = if speed < Self::LASER_THRESHOLD_KMH {
                (Self::LASER_THRESHOLD_KMH, Self::NORMAL_ACCEL * power)
            } else {
                (Self::MAX_SPEED_KMH, Self::LASER_ACCEL * power)
            };
            let until_boundary = (boundary - speed) / accel;
            let time = remaining.min(until_boundary);
            let end = if time >= until_boundary {
                boundary
            } else {
                speed + accel * time
            };
            distance += (speed + end) * 0.5 * time / 3600.;
            speed = end;
            remaining = (remaining - time).max(0.);
        }
        (speed.min(Self::MAX_SPEED_KMH), distance)
    }

    pub fn step(&mut self, yaw: f32, pitch: f32, advance: bool, power: f32, dt: f32) {
        self.yaw = (self.yaw + yaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + pitch).clamp(-1.35, 1.35);
        let (speed, distance) =
            Self::integrate_speed(self.speed_kmh, if advance { power } else { 0. }, dt);
        self.speed_kmh = speed;
        let forward = self.forward();
        self.velocity = forward * (speed / 3.6 / Self::METERS_PER_UNIT);
        self.position += forward * (distance * 1000. / Self::METERS_PER_UNIT);
    }
    pub fn model_matrix(&self, center: Vector3) -> Mat4 {
        let r = -self.right();
        let u = self.up();
        let f = self.forward();
        // El OBJ original tiene nariz en +Z y boquilla en -Z.
        let rotation = Mat4([
            [r.x, u.x, f.x, 0.],
            [r.y, u.y, f.y, 0.],
            [r.z, u.z, f.z, 0.],
            [0., 0., 0., 1.],
        ]);
        let mut translate_center = Mat4::identity();
        translate_center.0[0][3] = -center.x;
        translate_center.0[1][3] = -center.y;
        translate_center.0[2][3] = -center.z;
        let mut world = Mat4::identity();
        world.0[0][3] = self.position.x;
        world.0[1][3] = self.position.y;
        world.0[2][3] = self.position.z;
        world.mul(rotation).mul(translate_center)
    }
    pub fn centered_point(&self, p: Vector3) -> Vector3 {
        self.position - self.right() * p.x + self.up() * p.y + self.forward() * p.z
    }
    pub fn direction(&self, d: Vector3) -> Vector3 {
        -self.right() * d.x + self.up() * d.y + self.forward() * d.z
    }
}

pub struct Camara {
    pub position: Vector3,
    target: Vector3,
    up: Vector3,
    pub distance: f32,
}
impl Camara {
    // Ajustes expresados en tamanos de nave, para no depender de la escala del OBJ.
    pub const DISTANCE: f32 = 1.45;
    pub const HEIGHT: f32 = 0.55;
    pub const LOOK_AHEAD: f32 = 0.35;
    pub const FOLLOW_SPEED: f32 = 7.0;
    fn desired(nave: &Nave, size: f32, distance: f32) -> (Vector3, Vector3) {
        (
            nave.position - nave.forward() * distance + nave.up() * size * Self::HEIGHT,
            nave.position + nave.forward() * size * Self::LOOK_AHEAD + nave.up() * size * 0.08,
        )
    }
    pub fn new(nave: &Nave, size: f32) -> Self {
        let distance = size * Self::DISTANCE;
        let (position, target) = Self::desired(nave, size, distance);
        Self {
            position,
            target,
            up: nave.up(),
            distance,
        }
    }
    pub fn update(&mut self, nave: &Nave, size: f32, zoom: f32, dt: f32) {
        self.distance = (self.distance - zoom * size * 0.12).clamp(size * 0.9, size * 3.0);
        let (position, target) = Self::desired(nave, size, self.distance);
        let alpha = 1.0 - (-Self::FOLLOW_SPEED * dt.max(0.)).exp();
        self.position += (position - self.position) * alpha;
        self.target += (target - self.target) * alpha;
        let up = self.up + (nave.up() - self.up) * alpha;
        self.up = if up.length() > 1e-6 {
            up.normalized()
        } else {
            nave.up()
        };
    }
    pub fn view_matrix(&self) -> Mat4 {
        let forward = (self.target - self.position).normalized();
        let reference = if forward.dot(self.up).abs() > 0.98 {
            if forward.y.abs() < 0.9 {
                Vector3::new(0., 1., 0.)
            } else {
                Vector3::new(1., 0., 0.)
            }
        } else {
            self.up
        };
        let right = forward.cross(reference).normalized();
        let up = right.cross(forward).normalized();
        let e = self.position;
        Mat4([
            [right.x, right.y, right.z, -right.dot(e)],
            [up.x, up.y, up.z, -up.dot(e)],
            [forward.x, forward.y, forward.z, -forward.dot(e)],
            [0., 0., 0., 1.],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acceleration_braking_and_cap() {
        let mut ship = Nave::new();
        ship.step(0., 0., true, 1., 1.);
        assert!((ship.speed_kmh - Nave::NORMAL_ACCEL).abs() < 0.01);
        assert!(ship.position.z < 0.);
        ship.step(0., 0., true, 1., 100.);
        assert_eq!(ship.speed_kmh, Nave::MAX_SPEED_KMH);
        ship.step(0., 0., false, 1., 1.);
        assert_eq!(ship.speed_kmh, Nave::MAX_SPEED_KMH - Nave::BRAKING);
        ship.step(0., 0., false, 1., 20.);
        assert_eq!(ship.speed_kmh, 0.);
        assert_eq!(ship.velocity.length(), 0.);
    }
    #[test]
    fn threshold_is_strictly_above_laser_limit() {
        let mut ship = Nave::new();
        ship.speed_kmh = Nave::LASER_THRESHOLD_KMH;
        assert!(!ship.laser_mode());
        ship.speed_kmh = Nave::LASER_THRESHOLD_KMH + 0.1;
        assert!(ship.laser_mode());
    }
    #[test]
    fn levels_change_acceleration() {
        let slow = Nave::integrate_speed(0., 0.3, 1.).0;
        let medium = Nave::integrate_speed(0., 0.6, 1.).0;
        let full = Nave::integrate_speed(0., 1., 1.).0;
        assert!(slow < medium && medium < full);
        assert!((slow - Nave::NORMAL_ACCEL * 0.3).abs() < 0.01);
    }
    #[test]
    fn timestep_independence_across_boundaries() {
        let (one, distance) = Nave::integrate_speed(0., 1., 20.);
        let mut speed = 0.;
        let mut total = 0.;
        for _ in 0..2000 {
            let (s, d) = Nave::integrate_speed(speed, 1., 0.01);
            speed = s;
            total += d;
        }
        assert!((one - speed).abs() < 0.1);
        assert!((distance - total).abs() < 0.01);
    }
    #[test]
    fn distance_and_speed_use_same_units() {
        let mut ship = Nave::new();
        ship.speed_kmh = Nave::MAX_SPEED_KMH;
        ship.step(0., 0., true, 1., 1.);
        let expected = Nave::MAX_SPEED_KMH / 3.6 / Nave::METERS_PER_UNIT;
        assert!((ship.position.z + expected).abs() < 0.05);
        assert!((ship.velocity.length() - expected).abs() < 0.05);
    }
    #[test]
    fn controls_and_camera_basis() {
        let mut ship = Nave::new();
        ship.step(0.4, 0.3, false, 1., 0.);
        assert!(ship.forward().x > 0. && ship.forward().y > 0.);
        assert!(ship.forward().dot(ship.right()).abs() < 1e-5);
        let camera = Camara::new(&ship, 10.);
        let p = camera.position;
        let q = camera.view_matrix().transform([p.x, p.y, p.z, 1.]);
        assert!(q[0].abs() < 1e-4 && q[1].abs() < 1e-4 && q[2].abs() < 1e-4);
    }
}
