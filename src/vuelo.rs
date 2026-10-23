//! Estado de vuelo y camara de seguimiento, calculados exclusivamente en CPU.
use crate::pipeline::Mat4;
use raylib::prelude::Vector3;

pub struct Nave {
    pub position: Vector3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vector3,
}
impl Nave {
    pub fn new() -> Self {
        Self {
            position: Vector3::zero(),
            yaw: 0.,
            pitch: 0.,
            velocity: Vector3::zero(),
        }
    }
    pub fn forward(&self) -> Vector3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vector3::new(sy * cp, sp, -cy * cp)
    }
    pub fn right(&self) -> Vector3 {
        Vector3::new(self.yaw.cos(), 0., self.yaw.sin())
    }
    pub fn up(&self) -> Vector3 {
        self.right().cross(self.forward()).normalized()
    }
    pub fn step(&mut self, yaw: f32, pitch: f32, advance: bool, dt: f32, size: f32) {
        self.yaw = (self.yaw + yaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + pitch).clamp(-1.35, 1.35);
        // Sin inercia al soltar Espacio: detenerse y apagar efectos inmediatamente.
        self.velocity = if advance {
            self.forward() * size * 3.0
        } else {
            Vector3::zero()
        };
        self.position += self.velocity * dt.max(0.);
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
    fn forward_and_stop() {
        let mut ship = Nave::new();
        ship.step(0., 0., true, 1., 10.);
        assert!((ship.position.z + 30.).abs() < 1e-4);
        let before = ship.position;
        ship.step(0., 0., false, 1., 10.);
        assert!((ship.position - before).length() < 1e-4);
        assert_eq!(ship.velocity.length(), 0.);
    }
    #[test]
    fn controls_and_basis() {
        let mut ship = Nave::new();
        ship.step(0.4, 0.3, false, 0., 10.);
        assert!(ship.forward().x > 0. && ship.forward().y > 0.);
        assert!(ship.forward().dot(ship.right()).abs() < 1e-5);
        assert!(ship.forward().dot(ship.up()).abs() < 1e-5);
        assert!((ship.forward().length() - 1.).abs() < 1e-5);
    }
    #[test]
    fn view_maps_camera_to_origin() {
        let ship = Nave::new();
        let camera = Camara::new(&ship, 10.);
        let p = camera.position;
        let q = camera.view_matrix().transform([p.x, p.y, p.z, 1.]);
        assert!(q[0].abs() < 1e-4 && q[1].abs() < 1e-4 && q[2].abs() < 1e-4);
        let nose = ship.centered_point(Vector3::new(0., 0., 1.));
        assert!((nose - ship.forward()).length() < 1e-5);
    }
    #[test]
    fn distance_is_time_based() {
        let mut one = Nave::new();
        let mut many = Nave::new();
        one.step(0., 0., true, 1., 10.);
        for _ in 0..100 {
            many.step(0., 0., true, 0.01, 10.);
        }
        assert!((one.position - many.position).length() < 1e-3);
    }
}
