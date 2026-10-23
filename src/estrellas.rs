//! Estrellas reproducibles, preparadas una vez y dibujadas en el framebuffer CPU.
use crate::framebuffer::Band;
use raylib::prelude::Color;
struct Star {
    x: i32,
    y: i32,
    radius: i32,
    color: Color,
}
pub struct Estrellas {
    stars: Vec<Star>,
}
impl Estrellas {
    pub fn new(width: i32, height: i32) -> Self {
        let mut seed = 0x2026_1023u32;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let mut stars = Vec::new();
        for _ in 0..190 {
            let x = (random() % width as u32) as i32;
            let y = (random() % height as u32) as i32;
            let brightness = (90 + random() % 166) as u8;
            let cool = random() % 2 == 0;
            let color = if cool {
                Color::new(
                    brightness.saturating_sub(25),
                    brightness.saturating_sub(10),
                    brightness,
                    255,
                )
            } else {
                Color::new(
                    brightness,
                    brightness.saturating_sub(8),
                    brightness.saturating_sub(20),
                    255,
                )
            };
            let radius = if random() % 8 == 0 { 2 } else { 1 };
            stars.push(Star {
                x,
                y,
                radius,
                color,
            });
        }
        Self { stars }
    }
    pub fn dibujar(&self, band: &mut Band) {
        for star in &self.stars {
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
}
