
use crate::pipeline::{Mat4, Uniforms};
use crate::planeta::{fbm_n, malla_esfera, mezcla, proyeccion_esfera, Malla};
use raylib::prelude::{Color, Vector3};

pub const RADIO_SOL: f32 = 700.;

pub struct Sol {
    pub centro: Vector3,
    pub radio: f32,
    /// De mayor a menor detalle.
    mallas: Vec<Malla>,
}

impl Sol {
    pub fn nuevo(radio: f32) -> Self {
        let color = Color::new(255, 200, 60, 255);
        Self {
            centro: Vector3::zero(),
            radio,
            mallas: [5, 3, 2]
                .iter()
                .map(|&n| malla_esfera(radio, n, color))
                .collect(),
        }
    }

    pub fn matriz_modelo(&self) -> Mat4 {
        let mut m = Mat4::identity();
        m.0[0][3] = self.centro.x;
        m.0[1][3] = self.centro.y;
        m.0[2][3] = self.centro.z;
        m
    }

    pub fn proyeccion(&self, u: &Uniforms) -> Option<f32> {
        proyeccion_esfera(u, self.centro, self.radio, self.radio)
    }

    pub fn malla_lod(&self, radio_px: f32) -> &Malla {
        let nivel = if radio_px >= 150. {
            0
        } else if radio_px >= 40. {
            1
        } else {
            2
        };
        &self.mallas[nivel.min(self.mallas.len() - 1)]
    }
}

/// Fragment shader emisivo del sol. `world` es el punto de la superficie, `hacia_camara` el
/// vector unitario hacia el ojo y `tiempo` los segundos simulados.
pub fn color_estrella(world: Vector3, centro: Vector3, hacia_camara: Vector3, tiempo: f32) -> Color {
    let dir = (world - centro).normalized();
    // Dos capas de plasma que derivan a distinta velocidad: manchas grandes y granulacion fina.
    let deriva = Vector3::new(tiempo * 0.04, tiempo * 0.025, -tiempo * 0.03);
    let grande = fbm_n(dir + deriva, 7, 3, 1.5);
    let fino = fbm_n(dir + deriva * 3., 19, 2, 5.);
    let calor = ((grande - 0.3) / 0.4 + (fino - 0.5) * 0.5).clamp(0., 1.);
    let base = mezcla([205., 60., 8.], [255., 238., 130.], calor);
    let mancha = if grande < 0.33 { 0.55 } else { 1. };
    // El borde del disco se ve mas oscuro y mas rojizo que el centro, y el brillo late ligeramente.
    let limbo = dir.dot(hacia_camara).clamp(0., 1.);
    let base = mezcla([185., 45., 5.], base, limbo.sqrt());
    let brillo = (0.45 + 0.4 * limbo.sqrt()) * (1. + 0.03 * (tiempo * 2.).sin()) * mancha;
    let canal = |c: f32| (c * brillo).clamp(0., 255.) as u8;
    Color::new(canal(base[0]), canal(base[1]), canal(base[2]), 255)
}
