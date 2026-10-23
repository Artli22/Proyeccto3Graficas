use crate::framebuffer::Band;
use crate::particle_renderer::{dibujar_particula, proyectar_particula, ParticulaPantalla};
use raylib::prelude::{Color, Vector3};

const LONGITUD_HAZ: f32 = 40.0;
const RADIO_NUCLEO: f32 = 0.16;
const RADIO_BRILLO: f32 = 0.85;
const PROFUNDIDAD_MIN: f32 = 0.5;

pub struct Laser {
    pub potencia: f32,
    tiempo: f32,
}

// Haz ya proyectado a pantalla. 1/profundidad varía linealmente en pantalla, lo que da
// profundidad y grosor con perspectiva correcta a lo largo del haz.
struct HazPantalla {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    inv0: f32,
    inv1: f32,
    focal: f32,
    potencia: f32,
}

pub struct LaserPantalla {
    haz: HazPantalla,
    destello: Vec<ParticulaPantalla>,
}

impl Laser {
    pub fn new() -> Self {
        Laser { potencia: 0.3, tiempo: 0.0 }
    }

    pub fn update(&mut self, delta_time: f32, objetivo: f32) {
        self.tiempo += delta_time;
        let k = (delta_time * 10.0).min(1.0);
        self.potencia += (objetivo - self.potencia) * k;
    }

    // `origen` y `dir` están en el espacio del modelo; `rotar` lleva un punto al espacio de cámara.
    pub fn preparar(
        &self,
        origen: Vector3,
        dir: Vector3,
        rotar: impl Fn(Vector3) -> Vector3,
        view_distance: f32,
        screen_width: i32,
        screen_height: i32,
        focal: f32,
    ) -> Option<LaserPantalla> {
        let parpadeo = 1.0 + 0.06 * (self.tiempo * 55.0).sin();
        let potencia = (self.potencia * parpadeo).clamp(0.0, 1.0);

        let a = rotar(origen);
        let b = rotar(origen + dir.normalized() * LONGITUD_HAZ);
        let (da, db) = (view_distance - a.z, view_distance - b.z);
        if da < PROFUNDIDAD_MIN && db < PROFUNDIDAD_MIN {
            return None;
        }

        // Recorta el haz para que no cruce el plano de la cámara
        let punto = |t: f32| a + (b - a) * t;
        let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
        if da < PROFUNDIDAD_MIN {
            t0 = (PROFUNDIDAD_MIN - da) / (db - da);
        }
        if db < PROFUNDIDAD_MIN {
            t1 = (PROFUNDIDAD_MIN - da) / (db - da);
        }
        let (p0, p1) = (punto(t0), punto(t1));
        let (d0, d1) = (view_distance - p0.z, view_distance - p1.z);

        let (cx, cy) = (screen_width as f32 / 2.0, screen_height as f32 / 2.0);
        let haz = HazPantalla {
            x0: cx + p0.x * focal / d0,
            y0: cy - p0.y * focal / d0,
            x1: cx + p1.x * focal / d1,
            y1: cy - p1.y * focal / d1,
            inv0: 1.0 / d0,
            inv1: 1.0 / d1,
            focal,
            potencia,
        };

        let brillo = Color::new(255, 40, 30, (190.0 * potencia) as u8);
        let nucleo = Color::new(255, 225, 215, (235.0 * potencia) as u8);
        let destello: Vec<ParticulaPantalla> = [
            proyectar_particula(a, 0.9 * potencia + 0.2, brillo, view_distance, screen_width, screen_height, focal),
            proyectar_particula(a, 0.3 * potencia + 0.08, nucleo, view_distance, screen_width, screen_height, focal),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(LaserPantalla { haz, destello })
    }
}

pub fn dibujar_laser(band: &mut Band, laser: &LaserPantalla) {
    dibujar_haz(band, &laser.haz);
    for d in &laser.destello {
        dibujar_particula(band, d);
    }
}

fn dibujar_haz(band: &mut Band, h: &HazPantalla) {
    let (dx, dy) = (h.x1 - h.x0, h.y1 - h.y0);
    let largo_sq = dx * dx + dy * dy;
    if largo_sq < 1e-6 {
        return;
    }

    let margen = RADIO_BRILLO * h.focal * h.inv0.max(h.inv1) + 1.0;
    let ancho = band.width;

    for y in band.y0..band.y1 {
        let py = y as f32 + 0.5;

        // Rango de x donde el segmento puede estar a menos de `margen` de esta fila
        let (x_min, x_max) = if dy.abs() < 1e-3 {
            if (py - h.y0).abs() > margen {
                continue;
            }
            (h.x0.min(h.x1), h.x0.max(h.x1))
        } else {
            let ta = ((py - margen - h.y0) / dy).clamp(0.0, 1.0);
            let tb = ((py + margen - h.y0) / dy).clamp(0.0, 1.0);
            let (xa, xb) = (h.x0 + dx * ta, h.x0 + dx * tb);
            if (ta == tb) && (py - margen > h.y0.max(h.y1) || py + margen < h.y0.min(h.y1)) {
                continue;
            }
            (xa.min(xb), xa.max(xb))
        };
        let x_ini = ((x_min - margen) as i32).max(0);
        let x_fin = ((x_max + margen) as i32 + 1).min(ancho - 1);

        for x in x_ini..=x_fin {
            let px = x as f32 + 0.5;
            let t = (((px - h.x0) * dx + (py - h.y0) * dy) / largo_sq).clamp(0.0, 1.0);
            let (qx, qy) = (h.x0 + dx * t, h.y0 + dy * t);
            let dist = ((px - qx) * (px - qx) + (py - qy) * (py - qy)).sqrt();

            let inv = h.inv0 + (h.inv1 - h.inv0) * t;
            let escala = h.focal * inv;
            let r_brillo = RADIO_BRILLO * escala;
            if dist >= r_brillo {
                continue;
            }
            let depth = 1.0 / inv;
            let desvanecer = 1.0 - 0.5 * t;

            let g = 1.0 - dist / r_brillo;
            let alpha_brillo = (190.0 * g * g * h.potencia * desvanecer) as u32;
            if alpha_brillo > 3 {
                band.blend(x, y, depth, [255, 40, 30], alpha_brillo);
            }

            let r_nucleo = RADIO_NUCLEO * escala;
            if dist < r_nucleo.max(1.0) {
                let c = 1.0 - dist / r_nucleo.max(1.0);
                let alpha_nucleo = (235.0 * c.sqrt() * h.potencia * desvanecer) as u32;
                if alpha_nucleo > 3 {
                    band.blend(x, y, depth, [255, 225, 215], alpha_nucleo);
                }
            }
        }
    }
}
