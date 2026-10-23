use crate::fragment::{fragment_shader, Fragment, ShaderParams};
use crate::framebuffer::Band;
use crate::pipeline::ScreenVertex;
use cohete::color::Relleno;
use raylib::prelude::Vector3;

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
    framebuffer: &mut Band,
    mut vertices: [ScreenVertex; 3],
    relleno: &Relleno,
    shader: ShaderParams,
) {
    let mut area = edge(
        vertices[0].screen,
        vertices[1].screen,
        vertices[2].screen.x,
        vertices[2].screen.y,
    );
    if !area.is_finite() || area.abs() < 1e-6 {
        return;
    }
    if area < 0. {
        vertices.swap(1, 2);
        area = -area;
    }
    let [va, vb, vc] = vertices;
    let [a, b, c] = vertices.map(|v| v.screen);
    if [a, b, c].iter().any(|v| v.z <= 0. || !v.z.is_finite()) {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.) as i32;
    let max_x =
        a.x.max(b.x)
            .max(c.x)
            .ceil()
            .min((framebuffer.width - 1) as f32) as i32;
    let min_y = (a.y.min(b.y).min(c.y).floor() as i32).max(framebuffer.y0);
    let max_y = (a.y.max(b.y).max(c.y).ceil() as i32).min(framebuffer.y1 - 1);
    let top_left = |p: Vector3, q: Vector3| q.y < p.y || (q.y == p.y && q.x > p.x);
    let inclusive = [top_left(b, c), top_left(c, a), top_left(a, b)];
    let inside = |e: f32, i: usize| e > 0. || (e == 0. && inclusive[i]);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let e = [edge(b, c, px, py), edge(c, a, px, py), edge(a, b, px, py)];
            if !inside(e[0], 0) || !inside(e[1], 1) || !inside(e[2], 2) {
                continue;
            }
            let weights = [
                e[0] / (area * a.z),
                e[1] / (area * b.z),
                e[2] / (area * c.z),
            ];
            let inv_z = weights[0] + weights[1] + weights[2];
            if inv_z <= 0. || !inv_z.is_finite() {
                continue;
            }
            if !framebuffer.visible(x, y, 1. / inv_z) {
                continue;
            }
            let f = Fragment {
                uv: (va.uv * weights[0] + vb.uv * weights[1] + vc.uv * weights[2]) / inv_z,
                world: (va.world * weights[0] + vb.world * weights[1] + vc.world * weights[2])
                    / inv_z,
            };
            framebuffer.set_current_color(fragment_shader(&f, relleno, shader));
            framebuffer.point(x, y, 1. / inv_z);
        }
    }
}
