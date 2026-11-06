use raylib::prelude::{Color, Vector3};

const AMBIENTE: f32 = 0.12;
const INTENSIDAD_DIFUSA: f32 = 0.70;
const INTENSIDAD_ESPECULAR: f32 = 0.85;
const BRILLO: f32 = 40.0;
const DISTANCIA_MEDIA_SOL: f32 = 4800.0;
const LUZ_MINIMA: f32 = 0.2;
const ENVOLVENTE: f32 = 0.12;
const TINTE_SOL: [f32; 3] = [1.0, 0.94, 0.82];
const TINTE_AMBIENTE: [f32; 3] = [0.80, 0.88, 1.0];

/// Direccion de luz de respaldo cuando no hay una fuente en la escena.
pub fn direccion_fija() -> Vector3 {
    Vector3::new(-0.6, 0.7, 0.5).normalized()
}

/// Atenuacion de la luz del sol con la distancia (ley del inverso del cuadrado suavizada).
pub fn atenuacion(distancia: f32) -> f32 {
    (1.0 / (1.0 + (distancia / DISTANCIA_MEDIA_SOL).powi(2))).max(LUZ_MINIMA)
}

#[derive(Clone, Copy)]
pub struct Sombreado {
    pub difuso: f32,
    pub especular: f32,
    /// Multiplicador por canal de la luz difusa (mezcla de tinte solar y ambiental).
    pub tinte: [f32; 3],
}

impl Sombreado {
    pub const NINGUNO: Sombreado = Sombreado { difuso: 1.0, especular: 0.0, tinte: [1.0; 3] };

    pub fn aplicar(self, color: Color) -> Color {
        let canal = |c: u8, k: usize| {
            (c as f32 * self.difuso * self.tinte[k] + 255.0 * self.especular).clamp(0.0, 255.0) as u8
        };
        Color::new(canal(color.r, 0), canal(color.g, 1), canal(color.b, 2), color.a)
    }
}

/// Blinn-Phong: `luz` es la direccion unitaria hacia la fuente e `intensidad` su fuerza en el
/// punto (1 = sin atenuar). El ambiente no depende de la fuente.
pub fn sombrear(normal: Vector3, hacia_camara: Vector3, luz: Vector3, intensidad: f32) -> Sombreado {
    let n_luz = normal.dot(luz).max(0.0);
    // Lambert con envolvente: el paso de dia a noche es gradual, como con una atmosfera.
    let envuelto = ((normal.dot(luz) + ENVOLVENTE) / (1.0 + ENVOLVENTE)).clamp(0.0, 1.0);
    let directa = INTENSIDAD_DIFUSA * envuelto * intensidad;
    let difuso = AMBIENTE + directa;
    let sol = if difuso > 0.0 { directa / difuso } else { 0.0 };
    let tinte = std::array::from_fn(|k| TINTE_AMBIENTE[k] + (TINTE_SOL[k] - TINTE_AMBIENTE[k]) * sol);

    let especular = if n_luz > 0.0 {
        let medio = (luz + hacia_camara).normalized();
        INTENSIDAD_ESPECULAR * intensidad * normal.dot(medio).max(0.0).powf(BRILLO)
    } else {
        0.0
    };

    Sombreado { difuso, especular, tinte }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces_facing_the_light_are_brighter() {
        let luz = Vector3::new(1., 0., 0.);
        let camara = Vector3::new(0., 0., 1.);
        let de_frente = sombrear(Vector3::new(1., 0., 0.), camara, luz, 1.);
        let de_espaldas = sombrear(Vector3::new(-1., 0., 0.), camara, luz, 1.);
        assert!(de_frente.difuso > de_espaldas.difuso);
        // El lado oscuro conserva solo el ambiente.
        assert!((de_espaldas.difuso - AMBIENTE).abs() < 1e-6);
    }

    #[test]
    fn light_weakens_with_distance_but_keeps_a_floor() {
        assert!(atenuacion(0.) > atenuacion(3600.));
        assert!(atenuacion(3600.) > atenuacion(10_000.));
        assert!((atenuacion(1e9) - LUZ_MINIMA).abs() < 1e-6);
    }

    #[test]
    fn dimmer_light_lowers_diffuse_term() {
        let luz = Vector3::new(0., 1., 0.);
        let n = Vector3::new(0., 1., 0.);
        let v = Vector3::new(0., 0., 1.);
        assert!(sombrear(n, v, luz, 1.).difuso > sombrear(n, v, luz, 0.4).difuso);
    }
}
