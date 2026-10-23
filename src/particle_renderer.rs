use crate::framebuffer::Band;
use raylib::prelude::{Color, Vector3};
use std::sync::OnceLock;

pub struct ParticulaPantalla {
    cx: i32,
    cy: i32,
    radio: i32,
    pub depth: f32,
    color: Color,
}

// `rotada` ya está en el espacio de cámara; usa la misma proyección y profundidad que el modelo
// para que el z-buffer oculte correctamente las partículas detrás del cohete.
pub fn proyectar_particula(
    rotada: Vector3,
    radio_mundo: f32,
    color: Color,
    view_distance: f32,
    screen_width: i32,
    screen_height: i32,
    focal: f32,
) -> Option<ParticulaPantalla> {
    let depth = view_distance - rotada.z;
    if depth <= 0.1 || color.a == 0 {
        return None;
    }

    let scale = focal / depth;
    let cx = (screen_width as f32 / 2.0 + rotada.x * scale) as i32;
    let cy = (screen_height as f32 / 2.0 - rotada.y * scale) as i32;
    let radio = ((radio_mundo * scale) as i32).clamp(2, 60);

    if cx + radio < 0 || cx - radio >= screen_width || cy + radio < 0 || cy - radio >= screen_height {
        return None;
    }
    Some(ParticulaPantalla { cx, cy, radio, depth, color })
}

pub fn dibujar_particula(band: &mut Band, p: &ParticulaPantalla) {
    if p.cy + p.radio < band.y0 || p.cy - p.radio >= band.y1 {
        return;
    }

    let tabla = tabla_falloff();
    let radio_sq = p.radio * p.radio;
    let y_ini = (p.cy - p.radio).max(band.y0);
    let y_fin = (p.cy + p.radio).min(band.y1 - 1);
    let x_ini = (p.cx - p.radio).max(0);
    let x_fin = (p.cx + p.radio).min(band.width - 1);
    let rgb = [p.color.r, p.color.g, p.color.b];
    let alfa_max = p.color.a as u32;

    for y in y_ini..=y_fin {
        let dy = y - p.cy;
        for x in x_ini..=x_fin {
            let dx = x - p.cx;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > radio_sq {
                continue;
            }
            let alpha = alfa_max * tabla[(dist_sq * 255 / radio_sq) as usize] as u32 / 255;
            if alpha < 8 {
                continue;
            }
            band.blend(x, y, p.depth, rgb, alpha);
        }
    }
}

// falloff[i] = (1 - t)^0.75 con t = i / 255, equivalente a sqrt(1 - t)^1.5
fn tabla_falloff() -> &'static [u8; 256] {
    static TABLA: OnceLock<[u8; 256]> = OnceLock::new();
    TABLA.get_or_init(|| {
        let mut t = [0u8; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = ((1.0 - i as f32 / 255.0).powf(0.75) * 255.0) as u8;
        }
        t
    })
}
