
use crate::framebuffer::Band;
use crate::pipeline::{Uniforms, NEAR};
use raylib::prelude::{Color, Vector3};

const ALCANCE: f32 = 4.;
const RADIO_MINIMO_PX: f32 = 6.;

pub struct Resplandor {
    x: f32,
    y: f32,
    disco: f32,
    escala: f32,
}

impl Resplandor {
    pub fn preparar(u: &Uniforms, centro: Vector3, radio: f32) -> Option<Self> {
        let c = u.camera_point(centro);
        if c.z <= NEAR.max(radio) {
            return None;
        }
        let x = u.width * 0.5 + c.x * u.height / c.z;
        let y = u.height * 0.5 - c.y * u.height / c.z;
        let disco = radio * u.height / c.z;
        if !(x.is_finite() && y.is_finite() && disco.is_finite()) {
            return None;
        }
        let escala = disco.max(RADIO_MINIMO_PX);
        let alcance = disco + ALCANCE * escala;
        if x + alcance < 0. || x - alcance > u.width || y + alcance < 0. || y - alcance > u.height {
            return None;
        }
        Some(Self { x, y, disco, escala })
    }
    
    fn opacidad(d: f32) -> f32 {
        let nucleo = 0.30 / (1. + 12. * d * d);
        let corona = 0.12 / (1. + 2. * d * d);
        let borde = (1. - d / ALCANCE).clamp(0., 1.);
        (nucleo + corona) * borde
    }

    fn color(d: f32) -> [f32; 3] {
        let t = (d / 2.).clamp(0., 1.);
        [235., 190. - 100. * t, 110. - 80. * t]
    }

    pub fn dibujar(&self, band: &mut Band) {
        let alcance = self.disco + ALCANCE * self.escala;
        let y_min = ((self.y - alcance).floor() as i32).max(band.y0);
        let y_max = ((self.y + alcance).ceil() as i32).min(band.y1 - 1);
        let x_min = ((self.x - alcance).floor() as i32).max(0);
        let x_max = ((self.x + alcance).ceil() as i32).min(band.width - 1);
        let disco2 = self.disco * self.disco;
        for py in y_min..=y_max {
            let dy = py as f32 + 0.5 - self.y;
            for px in x_min..=x_max {
                let dx = px as f32 + 0.5 - self.x;
                let r2 = dx * dx + dy * dy;
                if r2 < disco2 {
                    continue;
                }
                let d = (r2.sqrt() - self.disco) / self.escala;
                let a = Self::opacidad(d);
                if a < 0.01 {
                    continue;
                }
                let [r, g, b] = Self::color(d);
                band.overlay_pixel(px, py, Color::new(r as u8, g as u8, b as u8, (a * 255.) as u8));
            }
        }
    }
}
