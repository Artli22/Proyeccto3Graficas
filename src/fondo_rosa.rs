
use crate::fragment::ShaderParams;
use crate::framebuffer::Band;
use crate::pipeline::{self, Mat4, ScreenVertex, Uniforms};
use crate::triangle::triangle;
use cohete::color::Relleno;
use cohete::obj::Model;
use raylib::prelude::{Color, Vector2, Vector3};

pub struct FondoRosa {
    material: Relleno,
    aspect: f32,
}
impl FondoRosa {
    pub const WIDTH_IN_SHIPS: f32 = 75.;
    pub const OFFSET_IN_SHIPS: [f32; 3] = [0., -2., -90.];
    pub fn load(path: &str) -> std::io::Result<Self> {
        let model = Model::load_obj(path)?;
        if model.triangles.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "OBJ del fondo sin triangulos",
            ));
        }
        let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);
        for t in &model.triangles {
            for p in t.vertices {
                min = Vector3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                max = Vector3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
            }
        }
        let span = max.x - min.x;
        let height = max.z - min.z;
        if span <= 0. || height <= 0. || max.y - min.y > span * 0.001 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "El cache de fondo requiere el OBJ plano en XZ",
            ));
        }
        let width = 1152usize;
        let padding = 36usize;
        let scale = (width - 2 * padding) as f32 / span;
        let rows = (height * scale).ceil() as usize + 2 * padding;
        let mut mask = vec![0.; width * rows];
        for tri in &model.triangles {
            let p = tri.vertices.map(|v| {
                Vector2::new(
                    padding as f32 + (v.x - min.x) * scale,
                    padding as f32 + (v.z - min.z) * scale,
                )
            });
            raster_mask(&mut mask, width, rows, p);
        }
        let mask = dilate(&mask, width, rows, 2);
        let tight = blur(&mask, width, rows, 2, 3);
        let wide = blur(&mask, width, rows, 7, 3);
        let pixels = mask
            .iter()
            .enumerate()
            .map(|(i, &core)| {
                let halo = (tight[i] * 1.8 + wide[i] * 3.5).clamp(0., 0.85);
                let alpha = core + halo * (1. - core);
                if alpha < 1. / 255. {
                    Color::BLANK
                } else {
                    let center = (core / alpha).clamp(0., 1.);
                    Color::new(
                        255,
                        (5. + 65. * center) as u8,
                        (155. + 65. * center) as u8,
                        ((alpha * 255.).round() as u8).min(254),
                    )
                }
            })
            .collect();
        println!(
            "Fondo rosa: {} triangulos OBJ preparados en cache CPU {}x{}",
            model.triangles.len(),
            width,
            rows
        );
        Ok(Self {
            material: Relleno::Textura {
                width,
                height: rows,
                pixels,
            },
            aspect: rows as f32 / width as f32,
        })
    }
    pub fn preparar(&self, u: &Uniforms, eye: Vector3, ship_size: f32) -> Vec<[ScreenVertex; 3]> {
        let offset = Self::OFFSET_IN_SHIPS;
        let center = eye + Vector3::new(offset[0], offset[1], offset[2]) * ship_size;
        let width = ship_size * Self::WIDTH_IN_SHIPS;
        let height = width * self.aspect;
        let p = [
            center + Vector3::new(-width * 0.5, height * 0.5, 0.),
            center + Vector3::new(width * 0.5, height * 0.5, 0.),
            center + Vector3::new(width * 0.5, -height * 0.5, 0.),
            center + Vector3::new(-width * 0.5, -height * 0.5, 0.),
        ];
        let uv = [
            Vector2::new(0., 0.),
            Vector2::new(1., 0.),
            Vector2::new(1., 1.),
            Vector2::new(0., 1.),
        ];
        let uniforms = Uniforms {
            model: Mat4::identity(),
            ..*u
        };
        let mut out = Vec::new();
        for ids in [[0, 1, 2], [0, 2, 3]] {
            out.extend(pipeline::prepare(
                ids.map(|i| p[i]),
                ids.map(|i| uv[i]),
                &uniforms,
            ));
        }
        out
    }
    pub fn dibujar(&self, band: &mut Band, mesh: &[[ScreenVertex; 3]], eye: Vector3) {
        let shader = ShaderParams {
            normal: Vector3::new(0., 0., 1.),
            eye,
            engine_light: None,
            unlit: true,
            sol: None,
            emisivo: None,
        };
        for &t in mesh {
            triangle(band, t, &self.material, shader);
        }
    }
}
fn raster_mask(mask: &mut [f32], width: usize, height: usize, mut p: [Vector2; 3]) {
    let edge =
        |a: Vector2, b: Vector2, x: f32, y: f32| (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
    let area = edge(p[0], p[1], p[2].x, p[2].y);
    if area.abs() < 1e-7 {
        return;
    }
    if area < 0. {
        p.swap(1, 2);
    }
    let x0 = p
        .iter()
        .map(|p| p.x)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.) as usize;
    let x1 = p
        .iter()
        .map(|p| p.x)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min((width - 1) as f32) as usize;
    let y0 = p
        .iter()
        .map(|p| p.y)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.) as usize;
    let y1 = p
        .iter()
        .map(|p| p.y)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min((height - 1) as f32) as usize;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let mut coverage = 0.;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let (px, py) = (x as f32 + dx, y as f32 + dy);
                if edge(p[0], p[1], px, py) >= 0.
                    && edge(p[1], p[2], px, py) >= 0.
                    && edge(p[2], p[0], px, py) >= 0.
                {
                    coverage += 0.25;
                }
            }
            let i = y * width + x;
            mask[i] = (mask[i] + coverage).min(1.);
        }
    }
}
fn dilate(source: &[f32], width: usize, height: usize, radius: i32) -> Vec<f32> {
    let mut out = vec![0.; source.len()];
    for y in 0..height {
        for x in 0..width {
            let mut value = source[y * width + x];
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dy * dy > radius * radius {
                        continue;
                    }
                    let xx = x as i32 + dx;
                    let yy = y as i32 + dy;
                    if xx >= 0 && xx < width as i32 && yy >= 0 && yy < height as i32 {
                        value = value.max(source[yy as usize * width + xx as usize]);
                    }
                }
            }
            out[y * width + x] = value;
        }
    }
    out
}
fn blur(source: &[f32], width: usize, height: usize, radius: usize, passes: usize) -> Vec<f32> {
    let mut current = source.to_vec();
    let mut temp = vec![0.; source.len()];
    let mut output = vec![0.; source.len()];
    let divisor = (radius * 2 + 1) as f32;
    for _ in 0..passes {
        for y in 0..height {
            let mut sum = (0..=radius.min(width - 1))
                .map(|x| current[y * width + x])
                .sum::<f32>();
            for x in 0..width {
                temp[y * width + x] = (sum / divisor).max(0.);
                if x + radius + 1 < width {
                    sum += current[y * width + x + radius + 1];
                }
                if x >= radius {
                    sum -= current[y * width + x - radius];
                }
            }
        }
        for x in 0..width {
            let mut sum = (0..=radius.min(height - 1))
                .map(|y| temp[y * width + x])
                .sum::<f32>();
            for y in 0..height {
                output[y * width + x] = (sum / divisor).max(0.);
                if y + radius + 1 < height {
                    sum += temp[(y + radius + 1) * width + x];
                }
                if y >= radius {
                    sum -= temp[(y - radius) * width + x];
                }
            }
        }
        std::mem::swap(&mut current, &mut output);
    }
    current
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bloom_spreads_light_without_filling_background() {
        let mut mask = vec![0.; 25 * 25];
        mask[12 * 25 + 12] = 1.;
        let result = blur(&mask, 25, 25, 2, 2);
        assert!(result[12 * 25 + 12] > 0.);
        assert!(result[12 * 25 + 15] > 0.);
        assert_eq!(result[0], 0.);
    }
    #[test]
    fn source_triangle_creates_emission_mask() {
        let mut mask = vec![0.; 16 * 16];
        raster_mask(
            &mut mask,
            16,
            16,
            [
                Vector2::new(2., 2.),
                Vector2::new(14., 2.),
                Vector2::new(2., 14.),
            ],
        );
        assert!(mask.iter().any(|&a| a > 0.));
        assert_eq!(mask[15 * 16 + 15], 0.);
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    #[test]
    fn thicker_points_stay_local_to_the_design() {
        let mut source = vec![0.; 15 * 15];
        source[7 * 15 + 7] = 1.;
        let result = dilate(&source, 15, 15, 2);
        assert_eq!(result[7 * 15 + 9], 1.);
        assert_eq!(result[0], 0.);
        assert!(result.iter().filter(|&&v| v > 0.).count() > 1);
    }
}
