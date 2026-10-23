//! Campo estelar CPU: estrellas distantes y motas con paralaje para percibir el vuelo.
use crate::framebuffer::Band;
use crate::pipeline::Uniforms;
use raylib::prelude::{Color, Vector3};
struct Star {
    position: Vector3,
    color: Color,
    radius: i32,
}
pub struct Estrellas {
    distant: Vec<Star>,
    nearby: Vec<Star>,
}
pub struct StarScreen {
    x: i32,
    y: i32,
    color: Color,
    radius: i32,
}
impl Estrellas {
    pub fn new() -> Self {
        let mut seed = 0x2026_1023u32;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        let mut distant = Vec::new();
        let mut nearby = Vec::new();
        for i in 0..3300 {
            let z = random() * 2. - 1.;
            let angle = random() * std::f32::consts::TAU;
            let r = (1. - z * z).max(0.).sqrt();
            let color = Color::new(
                (130. + random() * 125.) as u8,
                (145. + random() * 110.) as u8,
                255,
                255,
            );
            let radius = if random() > 0.95 { 2 } else { 1 };
            if i < 2600 {
                distant.push(Star {
                    position: Vector3::new(r * angle.cos(), z, r * angle.sin()),
                    color,
                    radius,
                });
            } else {
                nearby.push(Star {
                    position: Vector3::new(
                        random() * 2. - 1.,
                        random() * 2. - 1.,
                        random() * 2. - 1.,
                    ),
                    color,
                    radius: 1,
                });
            }
        }
        Self { distant, nearby }
    }
    /// Se proyecta UNA vez por cuadro, no por franja ni por pixel.
    pub fn preparar(&self, u: &Uniforms, eye: Vector3, ship_size: f32) -> Vec<StarScreen> {
        let mut out = Vec::new();
        let mut project = |p: Vector3, star: &Star, brightness: f32| {
            if p.z <= 0.01 || brightness <= 0.01 {
                return;
            }
            let x = u.width * 0.5 + p.x * u.height / p.z;
            let y = u.height * 0.5 - p.y * u.height / p.z;
            if !x.is_finite() || !y.is_finite() || x < 0. || y < 0. || x >= u.width || y >= u.height
            {
                return;
            }
            let c = star.color;
            out.push(StarScreen {
                x: x as i32,
                y: y as i32,
                radius: star.radius,
                color: Color::new(
                    (c.r as f32 * brightness) as u8,
                    (c.g as f32 * brightness) as u8,
                    (c.b as f32 * brightness) as u8,
                    255,
                ),
            });
        };
        for star in &self.distant {
            project(u.camera_direction(star.position), star, 1.);
        }
        // Cubo periodico anclado al MUNDO, no a la pantalla. Suavizar al reciclar bordes.
        let range = ship_size * 18.;
        for star in &self.nearby {
            let wrap = |x: f32| (x + range).rem_euclid(2. * range) - range;
            let offset = Vector3::new(
                wrap(star.position.x * range - eye.x),
                wrap(star.position.y * range - eye.y),
                wrap(star.position.z * range - eye.z),
            );
            let edge = offset.x.abs().max(offset.y.abs()).max(offset.z.abs()) / range;
            let fade = ((1. - edge) * 5.).clamp(0., 1.)
                * (offset.length() / (ship_size * 3.)).clamp(0., 1.);
            project(u.camera_direction(offset), star, fade * 0.8);
        }
        out
    }
}
pub fn dibujar(stars: &[StarScreen], band: &mut Band) {
    for star in stars {
        if star.y + star.radius < band.y0 || star.y - star.radius >= band.y1 {
            continue;
        }
        for dy in -star.radius..=star.radius {
            for dx in -star.radius..=star.radius {
                let r2 = dx * dx + dy * dy;
                if r2 > star.radius * star.radius {
                    continue;
                }
                let factor = if r2 == 0 { 1. } else { 0.25 };
                let c = star.color;
                band.background_pixel(
                    star.x + dx,
                    star.y + dy,
                    Color::new(
                        (c.r as f32 * factor) as u8,
                        (c.g as f32 * factor) as u8,
                        (c.b as f32 * factor) as u8,
                        255,
                    ),
                );
            }
        }
    }
}
