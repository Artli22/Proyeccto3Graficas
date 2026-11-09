use raylib::prelude::{Color, Vector2};

pub const COLOR_POR_DEFECTO: Color = Color::new(0, 0, 0, 255);

#[derive(Clone)]
pub enum Relleno {
    Solido(Color),
    /// Color solido que ademas emite un resplandor propio (RGB) que se suma a la iluminacion.
    Brillante(Color, [f32; 3]),
    Textura {
        width: usize,
        height: usize,
        pixels: Vec<Color>,
    },
}

impl Relleno {
    pub fn color_en(&self, uv: Vector2) -> Color {
        match self {
            Relleno::Solido(color) | Relleno::Brillante(color, _) => *color,
            Relleno::Textura {
                width,
                height,
                pixels,
            } => {
                if *width == 0 || *height == 0 {
                    return Color::BLANK;
                }
                let x = uv.x.clamp(0., 1.) * (*width - 1) as f32;
                let y = uv.y.clamp(0., 1.) * (*height - 1) as f32;
                let x0 = x.floor() as usize;
                let y0 = y.floor() as usize;
                let x1 = (x0 + 1).min(*width - 1);
                let y1 = (y0 + 1).min(*height - 1);
                let tx = x - x0 as f32;
                let ty = y - y0 as f32;
                let mut rgb = [0.; 3];
                let mut alpha = 0.;
                for (px, py, weight) in [
                    (x0, y0, (1. - tx) * (1. - ty)),
                    (x1, y0, tx * (1. - ty)),
                    (x0, y1, (1. - tx) * ty),
                    (x1, y1, tx * ty),
                ] {
                    let c = pixels
                        .get(py * *width + px)
                        .copied()
                        .unwrap_or(Color::BLANK);
                    let a = c.a as f32 / 255. * weight;
                    alpha += a;
                    rgb[0] += c.r as f32 * a;
                    rgb[1] += c.g as f32 * a;
                    rgb[2] += c.b as f32 * a;
                }
                if alpha <= 1e-6 {
                    return Color::BLANK;
                }
                Color::new(
                    (rgb[0] / alpha).round() as u8,
                    (rgb[1] / alpha).round() as u8,
                    (rgb[2] / alpha).round() as u8,
                    (alpha * 255.).round() as u8,
                )
            }
        }
    }

    pub fn es_opaco(&self) -> bool {
        match self {
            Relleno::Solido(color) | Relleno::Brillante(color, _) => color.a == 255,
            Relleno::Textura { .. } => false,
        }
    }

    pub fn resplandor(&self) -> Option<[f32; 3]> {
        match self {
            Relleno::Brillante(_, resplandor) => Some(*resplandor),
            _ => None,
        }
    }
}

pub const COLORES_MANUALES: &[(Color, &[usize])] = &[
    (
        Color::new(10, 10, 14, 230),
        &[38, 39, 40, 41, 107, 116, 117, 118, 119],
    ),
    (
        Color::new(40, 210, 120, 255),
        &[
            26, 30, 31, 32, 33, 34, 35, 36, 37, 48, 51, 52, 54, 55, 57, 59, 60, 63, 64, 65, 67, 71,
            74, 75, 76, 77, 104, 108, 109, 110, 111, 112, 113, 114, 115, 124, 126, 129, 130, 132,
            133, 135, 137, 138, 139, 142, 143, 144, 149, 152, 153, 154, 155,
        ],
    ),
    (
        Color::new(23, 26, 74, 255),
        &[
            0, 2, 4, 5, 14, 18, 19, 20, 21, 43, 44, 45, 46, 68, 70, 72, 73, 78, 80, 82, 83, 92, 96,
            97, 98, 99, 121, 122, 123, 146, 148, 150, 151,
        ],
    ),
    (
        Color::new(47, 44, 121, 255),
        &[3, 22, 23, 24, 25, 27, 28, 81, 100, 101, 102, 103, 105, 106],
    ),
];

pub fn asignar_relleno(indice: usize) -> Relleno {
    let manual = COLORES_MANUALES
        .iter()
        .find(|(_, indices)| indices.contains(&indice));
    let color = manual.map_or(COLOR_POR_DEFECTO, |(color, _)| *color);
    Relleno::Solido(color)
}

pub fn mezclar(origen: Color, destino: [u8; 4]) -> [u8; 4] {
    let a = origen.a as f32 / 255.0;
    let canal = |s: u8, d: u8| (s as f32 * a + d as f32 * (1.0 - a)).round() as u8;
    [
        canal(origen.r, destino[0]),
        canal(origen.g, destino[1]),
        canal(origen.b, destino[2]),
        255,
    ]
}
