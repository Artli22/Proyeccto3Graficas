//! Pipeline propio en CPU. Matrices por filas, vectores columna.
//! No llama a camaras, modelos, shaders ni dibujo 3D de raylib.
use raylib::prelude::{Vector2, Vector3};

pub const NEAR: f32 = 0.1;
pub const FAR: f32 = 100000.0;
#[derive(Clone, Copy, Debug)]
pub struct Mat4(pub [[f32; 4]; 4]);
impl Mat4 {
    pub fn identity() -> Self {
        Self([
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ])
    }
    pub fn mul(self, b: Self) -> Self {
        let mut out = [[0.; 4]; 4];
        for (r, row) in out.iter_mut().enumerate() {
            for (c, x) in row.iter_mut().enumerate() {
                for k in 0..4 {
                    *x += self.0[r][k] * b.0[k][c];
                }
            }
        }
        Self(out)
    }
    pub fn transform(self, v: [f32; 4]) -> [f32; 4] {
        self.0
            .map(|r| r[0] * v[0] + r[1] * v[1] + r[2] * v[2] + r[3] * v[3])
    }
}
#[derive(Clone, Copy)]
pub struct Uniforms {
    pub model: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub width: f32,
    pub height: f32,
}
impl Uniforms {
    pub fn scene(model: Mat4, view: Mat4, width: i32, height: i32) -> Self {
        let mut uniforms = Self::new(Vector3::zero(), 0., 0., 0., width, height);
        uniforms.model = model;
        uniforms.view = view;
        uniforms
    }
    pub fn camera_point(&self, world: Vector3) -> Vector3 {
        let p = self.view.transform([world.x, world.y, world.z, 1.]);
        Vector3::new(p[0], p[1], p[2])
    }
    pub fn camera_direction(&self, direction: Vector3) -> Vector3 {
        let p = self
            .view
            .transform([direction.x, direction.y, direction.z, 0.]);
        Vector3::new(p[0], p[1], p[2])
    }
    // Compatibilidad con los efectos CPU originales: profundidad = -z, distancia=0.
    pub fn effect_point(&self, world: Vector3) -> Vector3 {
        let p = self.camera_point(world);
        Vector3::new(p.x, p.y, -p.z)
    }

    pub fn new(
        center: Vector3,
        yaw: f32,
        pitch: f32,
        distance: f32,
        width: i32,
        height: i32,
    ) -> Self {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let mut translation = Mat4::identity();
        translation.0[0][3] = -center.x;
        translation.0[1][3] = -center.y;
        translation.0[2][3] = -center.z;
        let ry = Mat4([
            [cy, 0., sy, 0.],
            [0., 1., 0., 0.],
            [-sy, 0., cy, 0.],
            [0., 0., 0., 1.],
        ]);
        let rx = Mat4([
            [1., 0., 0., 0.],
            [0., cp, -sp, 0.],
            [0., sp, cp, 0.],
            [0., 0., 0., 1.],
        ]);
        // La camara mira hacia -Z; el espacio de vista usa profundidad positiva.
        let view = Mat4([
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., -1., distance],
            [0., 0., 0., 1.],
        ]);
        let focal = height as f32;
        let projection = Mat4([
            [2. * focal / width as f32, 0., 0., 0.],
            [0., 2. * focal / height as f32, 0., 0.],
            [
                0.,
                0.,
                (FAR + NEAR) / (FAR - NEAR),
                -2. * FAR * NEAR / (FAR - NEAR),
            ],
            [0., 0., 1., 0.],
        ]);
        Self {
            model: rx.mul(ry).mul(translation),
            view,
            projection,
            width: width as f32,
            height: height as f32,
        }
    }
}
#[derive(Clone, Copy)]
pub struct VertexOut {
    pub clip: [f32; 4],
    pub world: Vector3,
    pub uv: Vector2,
}
#[derive(Clone, Copy)]
pub struct ScreenVertex {
    pub screen: Vector3, // x,y en pantalla y profundidad positiva de camara
    pub world: Vector3,
    pub uv: Vector2,
}
/// Vertex shader de SOFTWARE: modelo -> vista -> proyeccion homogenea.
pub fn vertex_shader(position: Vector3, uv: Vector2, u: &Uniforms) -> VertexOut {
    let world = u.model.transform([position.x, position.y, position.z, 1.]);
    let camera = u.view.transform(world);
    VertexOut {
        clip: u.projection.transform(camera),
        world: Vector3::new(world[0], world[1], world[2]),
        uv,
    }
}
fn distance(v: VertexOut, plane: usize) -> f32 {
    let [x, y, z, w] = v.clip;
    match plane {
        0 => w + x,
        1 => w - x,
        2 => w + y,
        3 => w - y,
        4 => w + z,
        _ => w - z,
    }
}
fn interpolate(a: VertexOut, b: VertexOut, t: f32) -> VertexOut {
    let mut clip = [0.; 4];
    for (i, c) in clip.iter_mut().enumerate() {
        *c = a.clip[i] + (b.clip[i] - a.clip[i]) * t;
    }
    VertexOut {
        clip,
        world: a.world + (b.world - a.world) * t,
        uv: a.uv + (b.uv - a.uv) * t,
    }
}
/// Sutherland-Hodgman contra los seis planos ANTES de dividir por w.
pub fn clip_triangle(triangle: [VertexOut; 3]) -> Vec<[VertexOut; 3]> {
    let mut salida = Vec::new();
    clip_triangle_into(triangle, &mut salida);
    salida
}

pub fn clip_triangle_into(triangle: [VertexOut; 3], salida: &mut Vec<[VertexOut; 3]>) {
    if triangle
        .iter()
        .any(|v| v.clip.iter().any(|x| !x.is_finite()))
    {
        return;
    }
    let mascara = |v: VertexOut| (0..6).fold(0u8, |m, p| m | (((distance(v, p) < 0.) as u8) << p));
    let m = triangle.map(mascara);
    if m[0] & m[1] & m[2] != 0 {
        return;
    }
    if m[0] | m[1] | m[2] == 0 {
        salida.push(triangle);
        return;
    }
    let mut polygon = triangle.to_vec();
    for plane in 0..6 {
        if polygon.len() < 3 {
            return;
        }
        let mut output = Vec::with_capacity(polygon.len() + 1);
        let mut previous = *polygon.last().unwrap();
        let mut previous_distance = distance(previous, plane);
        for &current in &polygon {
            let current_distance = distance(current, plane);
            if (previous_distance >= 0.) != (current_distance >= 0.) {
                let t = (previous_distance / (previous_distance - current_distance)).clamp(0., 1.);
                output.push(interpolate(previous, current, t));
            }
            if current_distance >= 0. {
                output.push(current);
            }
            previous = current;
            previous_distance = current_distance;
        }
        polygon = output;
    }
    for i in 1..polygon.len().saturating_sub(1) {
        salida.push([polygon[0], polygon[i], polygon[i + 1]]);
    }
}
pub fn viewport(v: VertexOut, u: &Uniforms) -> ScreenVertex {
    let inverse = 1. / v.clip[3];
    ScreenVertex {
        screen: Vector3::new(
            (v.clip[0] * inverse + 1.) * u.width * 0.5,
            (1. - v.clip[1] * inverse) * u.height * 0.5,
            v.clip[3],
        ),
        world: v.world,
        uv: v.uv,
    }
}
pub fn prepare(positions: [Vector3; 3], uvs: [Vector2; 3], u: &Uniforms) -> Vec<[ScreenVertex; 3]> {
    let vertices = std::array::from_fn(|i| vertex_shader(positions[i], uvs[i], u));
    clip_triangle(vertices)
        .into_iter()
        .map(|t| t.map(|v| viewport(v, u)))
        .collect()
}
