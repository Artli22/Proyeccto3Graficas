use crate::color::Relleno;
use crate::framebuffer::Framebuffer;
use crate::luz::Sombreado;
use raylib::prelude::{Vector2, Vector3};

fn edge(a: Vector3, b: Vector3, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

pub fn profundidad_en(v: [Vector3; 3], px: f32, py: f32) -> Option<f32> {
    let [a, b, c] = v;
    let area = edge(a, b, c.x, c.y);
    if area.abs() < 1e-6 {
        return None;
    }
    let w0 = edge(b, c, px, py) / area;
    let w1 = edge(c, a, px, py) / area;
    let w2 = edge(a, b, px, py) / area;
    if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
        Some(1.0 / (w0 / a.z + w1 / b.z + w2 / c.z))
    } else {
        None
    }
}

pub fn triangle(
    framebuffer: &mut Framebuffer,
    v: [Vector3; 3],
    uvs: [Vector2; 3],
    relleno: &Relleno,
    sombreado: Sombreado,
) {
    let [a, b, c] = v;
    let area = edge(a, b, c.x, c.y);
    if area.abs() < 1e-6 {
        return;
    }

    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
    let max_x = a.x.max(b.x).max(c.x).ceil().min((framebuffer.width - 1) as f32) as i32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let max_y = a.y.max(b.y).max(c.y).ceil().min((framebuffer.height - 1) as f32) as i32;

    let (inv_a, inv_b, inv_c) = (1.0 / a.z, 1.0 / b.z, 1.0 / c.z);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(b, c, px, py) / area;
            let w1 = edge(c, a, px, py) / area;
            let w2 = edge(a, b, px, py) / area;

            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let (p0, p1, p2) = (w0 * inv_a, w1 * inv_b, w2 * inv_c);
                let inv_z = p0 + p1 + p2;
                let uv = (uvs[0] * p0 + uvs[1] * p1 + uvs[2] * p2) / inv_z;

                framebuffer.set_current_color(sombreado.aplicar(relleno.color_en(uv)));
                framebuffer.point(x, y, 1.0 / inv_z);
            }
        }
    }
}
