
use crate::luz::{atenuacion, direccion_fija, sombrear};
use cohete::color::Relleno;
use raylib::prelude::{Color, Vector2, Vector3};
#[derive(Clone, Copy)]
pub struct ShaderParams {
    pub normal: Vector3,
    pub eye: Vector3,
    pub engine_light: Option<(Vector3, f32)>,
    pub unlit: bool,
    pub sol: Option<Vector3>,
    pub emisivo: Option<(Vector3, f32)>,
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
    if let Some((centro, tiempo)) = u.emisivo {
        return crate::sol::color_estrella(f.world, centro, view, tiempo);
    }
    let mut normal = u.normal;
    if normal.dot(view) < 0. {
        normal = -normal;
    }
    let (luz, intensidad) = match u.sol {
        Some(posicion) => {
            let hacia_sol = posicion - f.world;
            let distancia = hacia_sol.length();
            if distancia > 1e-3 {
                (hacia_sol / distancia, atenuacion(distancia))
            } else {
                (direccion_fija(), 1.)
            }
        }
        None => (direccion_fija(), 1.),
    };
    let mut light = sombrear(normal, view, luz, intensidad);
    if let Some((position, power)) = u.engine_light {
        let delta = position - f.world;
        let distance = delta.length();
        if distance > 1e-4 && distance < 9. {
            let diffuse = normal.dot(delta / distance).max(0.);
            light.difuso =
                (light.difuso + diffuse * power * 0.6 / (1. + distance * distance * 0.05)).min(1.);
        }
    }
    let iluminado = light.aplicar(color);
    match material.resplandor() {
        Some(resplandor) => aplicar_resplandor(iluminado, resplandor),
        None => iluminado,
    }
}

/// Mezcla "screen" del resplandor sobre el color ya iluminado: aclara sin saturar y deja que
/// el lado diurno conserve su sombreado mientras el lado nocturno sigue brillando.
fn aplicar_resplandor(c: Color, e: [f32; 3]) -> Color {
    let canal = |v: u8, e: f32| (255. - (255. - v as f32) * (255. - e) / 255.).clamp(0., 255.) as u8;
    Color::new(canal(c.r, e[0]), canal(c.g, e[1]), canal(c.b, e[2]), c.a)
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
            sol: None,
            emisivo: None,
        };
        let output = fragment_shader(&f, &Relleno::Solido(color), params);
        assert_eq!([output.r, output.g, output.b, output.a], [255, 0, 255, 200]);
    }

    fn brillo(c: Color) -> u32 {
        c.r as u32 + c.g as u32 + c.b as u32
    }

    #[test]
    fn sun_lights_the_side_that_faces_it() {
        let sol = Vector3::new(1000., 0., 0.);
        let punto = Fragment {
            world: Vector3::new(100., 0., 0.),
            uv: Vector2::zero(),
        };
        let material = Relleno::Solido(Color::new(120, 120, 120, 255));
        let con = |normal: Vector3| ShaderParams {
            normal,
            eye: Vector3::new(100., 0., 500.),
            engine_light: None,
            unlit: false,
            sol: Some(sol),
            emisivo: None,
        };
        let iluminado = fragment_shader(&punto, &material, con(Vector3::new(1., 0., 0.)));
        let a_oscuras = fragment_shader(&punto, &material, con(Vector3::new(-1., 0., 0.)));
        assert!(brillo(iluminado) > brillo(a_oscuras));
    }

    #[test]
    fn glowing_material_shines_on_the_dark_side_and_keeps_shading() {
        let sol = Vector3::new(1000., 0., 0.);
        let punto = Fragment {
            world: Vector3::new(100., 0., 0.),
            uv: Vector2::zero(),
        };
        let color = Color::new(70, 170, 110, 255);
        let con = |normal: Vector3| ShaderParams {
            normal,
            eye: Vector3::new(100., 0., 500.),
            engine_light: None,
            unlit: false,
            sol: Some(sol),
            emisivo: None,
        };
        let luz = Vector3::new(1., 0., 0.);
        let sombra = Vector3::new(-1., 0., 0.);
        let normal = Relleno::Solido(color);
        let brillante = Relleno::Brillante(color, [105., 225., 140.]);
        let noche = fragment_shader(&punto, &brillante, con(sombra));
        assert!(brillo(noche) > 2 * brillo(fragment_shader(&punto, &normal, con(sombra))));
        assert!(noche.g > noche.r && noche.g > noche.b);
        assert!(brillo(fragment_shader(&punto, &brillante, con(luz))) > brillo(noche));
    }

    #[test]
    fn emissive_surface_ignores_scene_lighting() {
        let f = Fragment {
            world: Vector3::new(0., 0., 700.),
            uv: Vector2::zero(),
        };
        let material = Relleno::Solido(Color::BLACK);
        let params = ShaderParams {
            normal: Vector3::new(0., 0., 1.),
            eye: Vector3::new(0., 0., 2000.),
            engine_light: None,
            unlit: false,
            sol: Some(Vector3::new(-5000., 0., 0.)),
            emisivo: Some((Vector3::zero(), 3.)),
        };
        assert!(brillo(fragment_shader(&f, &material, params)) > 100);
    }
}
