use raylib::prelude::{Color, Vector2};

pub const COLOR_POR_DEFECTO: Color = Color::new(0, 0, 0, 255);

#[derive(Clone)]
pub enum Relleno {
    Solido(Color),
}

impl Relleno {
    pub fn color_en(&self, _uv: Vector2) -> Color {
        match self {
            Relleno::Solido(color) => *color,
        }
    }

    pub fn es_opaco(&self) -> bool {
        match self {
            Relleno::Solido(color) => color.a == 255,
        }
    }
}

pub const COLORES_MANUALES: &[(Color, &[usize])] = &[
    (Color::new(10, 10, 14, 230), &[38, 39, 40, 41, 107, 116, 117, 118, 119]),
    (Color::new(40, 210, 120, 255), &[26, 30, 31, 32, 33, 34, 35, 36, 37, 48, 51, 52, 54, 55, 57, 59, 60, 63, 64, 65, 67, 71, 74, 75, 76, 77, 104, 108, 109, 110, 111, 112, 113, 114, 115, 124, 126, 129, 130, 132, 133, 135, 137, 138, 139, 142, 143, 144, 149, 152, 153, 154, 155]),
    (Color::new(23, 26, 74, 255), &[0, 2, 4, 5, 14, 18, 19, 20, 21, 43, 44, 45, 46, 68, 70, 72, 73, 78, 80, 82, 83, 92, 96, 97, 98, 99, 121, 122, 123, 146, 148, 150, 151]),
    (Color::new(47, 44, 121, 255), &[3, 22, 23, 24, 25, 27, 28, 81, 100, 101, 102, 103, 105, 106]),
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
