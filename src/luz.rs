use raylib::prelude::{Color, Vector3};

const AMBIENTE: f32 = 0.30;
const INTENSIDAD_DIFUSA: f32 = 0.70;
const INTENSIDAD_ESPECULAR: f32 = 0.85;
const BRILLO: f32 = 40.0;

fn direccion_luz() -> Vector3 {
    Vector3::new(-0.6, 0.7, 0.5).normalized()
}

#[derive(Clone, Copy)]
pub struct Sombreado {
    pub difuso: f32,
    pub especular: f32,
}

impl Sombreado {
    pub const NINGUNO: Sombreado = Sombreado { difuso: 1.0, especular: 0.0 };

    pub fn aplicar(self, color: Color) -> Color {
        let canal = |c: u8| (c as f32 * self.difuso + 255.0 * self.especular).clamp(0.0, 255.0) as u8;
        Color::new(canal(color.r), canal(color.g), canal(color.b), color.a)
    }
}

pub fn sombrear(normal: Vector3, hacia_camara: Vector3) -> Sombreado {
    let luz = direccion_luz();
    let n_luz = normal.dot(luz).max(0.0);
    let difuso = AMBIENTE + INTENSIDAD_DIFUSA * n_luz;

    let especular = if n_luz > 0.0 {
        let medio = (luz + hacia_camara).normalized();
        INTENSIDAD_ESPECULAR * normal.dot(medio).max(0.0).powf(BRILLO)
    } else {
        0.0
    };

    Sombreado { difuso, especular }
}
