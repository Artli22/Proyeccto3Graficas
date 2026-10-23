//! Fragment shader propio ejecutado por la CPU, no GLSL.
use crate::luz::sombrear;
use cohete::color::Relleno;
use raylib::prelude::{Color, Vector2, Vector3};
#[derive(Clone, Copy)]
pub struct ShaderParams {
    pub normal: Vector3,
    pub eye: Vector3,
    pub engine_light: Option<(Vector3, f32)>,
    pub unlit: bool,
}
pub struct Fragment {
    pub world: Vector3,
    pub uv: Vector2,
}
pub fn fragment_shader(f: &Fragment, material: &Relleno, u: ShaderParams) -> Color {
    let color = material.color_en(f.uv);
    if u.unlit {
        return color;
    }
    let delta = u.eye - f.world;
    let view = if delta.length() > 1e-6 {
        delta.normalized()
    } else {
        Vector3::new(0., 0., 1.)
    };
    let mut normal = u.normal;
    if normal.dot(view) < 0. {
        normal = -normal;
    }
    let mut light = sombrear(normal, view);
    if let Some((position, power)) = u.engine_light {
        let delta = position - f.world;
        let distance = delta.length();
        if distance > 1e-4 && distance < 9. {
            let diffuse = normal.dot(delta / distance).max(0.);
            light.difuso =
                (light.difuso + diffuse * power * 0.6 / (1. + distance * distance * 0.05)).min(1.);
        }
    }
    light.aplicar(color)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unlit_selection_preserves_color_and_alpha() {
        let color = Color::new(255, 0, 255, 200);
        let f = Fragment {
            world: Vector3::zero(),
            uv: Vector2::zero(),
        };
        let params = ShaderParams {
            normal: Vector3::new(0., 0., 1.),
            eye: Vector3::new(0., 0., 4.),
            engine_light: None,
            unlit: true,
        };
        let output = fragment_shader(&f, &Relleno::Solido(color), params);
        assert_eq!([output.r, output.g, output.b, output.a], [255, 0, 255, 200]);
    }
}
