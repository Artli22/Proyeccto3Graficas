//! Planeta procedural: icosfera subdividida, relieve por ruido fractal y color por bioma.
//! La malla se genera en CPU y se dibuja con el mismo pipeline que la nave.
use crate::pipeline::Mat4;
use cohete::color::Relleno;
use raylib::prelude::{Color, Vector3};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct PlanetaConfig {
    pub nombre: &'static str,
    pub radio: f32,
    pub semilla: u32,
    pub orbita_radio: f32,
    pub orbita_inicial: f32,
    pub orbita_velocidad: f32,   // rad/s
    pub rotacion_velocidad: f32, // rad/s
    pub subdivisiones: u32,
    pub relieve: f32, // altura maxima de montanas como fraccion del radio
}

const NIVEL_MAR: f32 = 0.5;

fn hash(x: i32, y: i32, z: i32, semilla: u32) -> f32 {
    let mut h = semilla
        ^ (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}

// Ruido de valor con interpolacion trilineal suavizada.
fn ruido(p: Vector3, semilla: u32) -> f32 {
    let (ix, iy, iz) = (p.x.floor(), p.y.floor(), p.z.floor());
    let suave = |t: f32| t * t * (3. - 2. * t);
    let (fx, fy, fz) = (suave(p.x - ix), suave(p.y - iy), suave(p.z - iz));
    let (ix, iy, iz) = (ix as i32, iy as i32, iz as i32);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let esquina = |dx, dy, dz| hash(ix + dx, iy + dy, iz + dz, semilla);
    let x00 = lerp(esquina(0, 0, 0), esquina(1, 0, 0), fx);
    let x10 = lerp(esquina(0, 1, 0), esquina(1, 1, 0), fx);
    let x01 = lerp(esquina(0, 0, 1), esquina(1, 0, 1), fx);
    let x11 = lerp(esquina(0, 1, 1), esquina(1, 1, 1), fx);
    lerp(lerp(x00, x10, fy), lerp(x01, x11, fy), fz)
}

// Ruido fractal normalizado a [0, 1].
fn fbm(dir: Vector3, semilla: u32) -> f32 {
    let (mut total, mut amplitud, mut frecuencia, mut suma) = (0., 1., 1.8, 0.);
    for octava in 0..5 {
        total += ruido(dir * frecuencia, semilla.wrapping_add(octava * 101)) * amplitud;
        suma += amplitud;
        amplitud *= 0.5;
        frecuencia *= 2.;
    }
    total / suma
}

fn color_bioma(n: f32, variacion: f32) -> Color {
    let v = 1. + (variacion - 0.5) * 0.12;
    let (r, g, b) = if n < NIVEL_MAR - 0.08 {
        (16., 48., 120.)
    } else if n < NIVEL_MAR {
        (30., 92., 170.)
    } else if n < NIVEL_MAR + 0.03 {
        (214., 196., 140.)
    } else if n < NIVEL_MAR + 0.11 {
        (74., 150., 66.)
    } else if n < NIVEL_MAR + 0.18 {
        (46., 110., 52.)
    } else if n < NIVEL_MAR + 0.25 {
        (128., 118., 108.)
    } else {
        (240., 242., 248.)
    };
    let canal = |c: f32| (c * v).clamp(0., 255.) as u8;
    Color::new(canal(r), canal(g), canal(b), 255)
}

fn icosaedro() -> (Vec<Vector3>, Vec<[usize; 3]>) {
    let phi = (1. + 5f32.sqrt()) / 2.;
    let vertices = [
        (-1., phi, 0.),
        (1., phi, 0.),
        (-1., -phi, 0.),
        (1., -phi, 0.),
        (0., -1., phi),
        (0., 1., phi),
        (0., -1., -phi),
        (0., 1., -phi),
        (phi, 0., -1.),
        (phi, 0., 1.),
        (-phi, 0., -1.),
        (-phi, 0., 1.),
    ]
    .iter()
    .map(|&(x, y, z)| Vector3::new(x, y, z).normalized())
    .collect();
    let caras = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    (vertices, caras)
}

// Cada pasada parte cada triangulo en cuatro y proyecta los puntos medios a la esfera.
fn subdividir(vertices: &mut Vec<Vector3>, caras: &[[usize; 3]]) -> Vec<[usize; 3]> {
    let mut medios: HashMap<(usize, usize), usize> = HashMap::new();
    let mut medio = |a: usize, b: usize, vertices: &mut Vec<Vector3>| {
        let clave = (a.min(b), a.max(b));
        *medios.entry(clave).or_insert_with(|| {
            vertices.push(((vertices[a] + vertices[b]) * 0.5).normalized());
            vertices.len() - 1
        })
    };
    let mut nuevas = Vec::with_capacity(caras.len() * 4);
    for &[a, b, c] in caras {
        let ab = medio(a, b, vertices);
        let bc = medio(b, c, vertices);
        let ca = medio(c, a, vertices);
        nuevas.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
    }
    nuevas
}

pub struct Planeta {
    pub config: PlanetaConfig,
    /// Triangulos en espacio local del planeta, con normal geometrica hacia afuera.
    pub triangulos: Vec<[Vector3; 3]>,
    pub rellenos: Vec<Relleno>,
    pub centro: Vector3,
    /// Desplazamiento del centro durante el ultimo `actualizar`.
    pub delta: Vector3,
    pub angulo_rotacion: f32,
    angulo_orbita: f32,
}

impl Planeta {
    pub fn nuevo(config: PlanetaConfig) -> Self {
        let mut planeta = Self {
            config,
            triangulos: Vec::new(),
            rellenos: Vec::new(),
            centro: Vector3::zero(),
            delta: Vector3::zero(),
            angulo_rotacion: 0.,
            angulo_orbita: config.orbita_inicial,
        };
        planeta.centro = planeta.posicion_orbital();
        planeta.generar_malla();
        planeta
    }

    fn altura_normalizada(&self, dir: Vector3) -> f32 {
        fbm(dir, self.config.semilla)
    }

    /// Distancia del centro a la superficie en la direccion local `dir` (unitaria).
    pub fn radio_superficie(&self, dir: Vector3) -> f32 {
        let n = self.altura_normalizada(dir).max(NIVEL_MAR);
        self.config.radio * (1. + self.config.relieve * (n - NIVEL_MAR) * 2.)
    }

    fn generar_malla(&mut self) {
        let (mut vertices, mut caras) = icosaedro();
        for _ in 0..self.config.subdivisiones {
            caras = subdividir(&mut vertices, &caras);
        }
        let alturas: Vec<f32> = vertices
            .iter()
            .map(|&d| self.altura_normalizada(d))
            .collect();
        let posiciones: Vec<Vector3> = vertices
            .iter()
            .map(|&d| d * self.radio_superficie(d))
            .collect();
        for (i, cara) in caras.iter().enumerate() {
            let mut tri = cara.map(|v| posiciones[v]);
            let centroide = (tri[0] + tri[1] + tri[2]) / 3.;
            let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            if normal.dot(centroide) < 0. {
                tri.swap(1, 2);
            }
            let n = cara.iter().map(|&v| alturas[v]).sum::<f32>() / 3.;
            let variacion = hash(i as i32, 7, 13, self.config.semilla);
            self.triangulos.push(tri);
            self.rellenos
                .push(Relleno::Solido(color_bioma(n, variacion)));
        }
    }

    fn posicion_orbital(&self) -> Vector3 {
        let (s, c) = self.angulo_orbita.sin_cos();
        Vector3::new(c * self.config.orbita_radio, 0., s * self.config.orbita_radio)
    }

    pub fn actualizar(&mut self, dt: f32) {
        self.angulo_orbita =
            (self.angulo_orbita + self.config.orbita_velocidad * dt).rem_euclid(std::f32::consts::TAU);
        self.angulo_rotacion = (self.angulo_rotacion + self.config.rotacion_velocidad * dt)
            .rem_euclid(std::f32::consts::TAU);
        let nuevo = self.posicion_orbital();
        self.delta = nuevo - self.centro;
        self.centro = nuevo;
    }

    pub fn matriz_modelo(&self) -> Mat4 {
        let (s, c) = self.angulo_rotacion.sin_cos();
        Mat4([
            [c, 0., s, self.centro.x],
            [0., 1., 0., self.centro.y],
            [-s, 0., c, self.centro.z],
            [0., 0., 0., 1.],
        ])
    }

    /// Vector local -> mundo (solo rotacion).
    pub fn a_mundo(&self, v: Vector3) -> Vector3 {
        let (s, c) = self.angulo_rotacion.sin_cos();
        Vector3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
    }

    /// Vector mundo -> local (solo rotacion).
    pub fn a_local(&self, v: Vector3) -> Vector3 {
        let (s, c) = self.angulo_rotacion.sin_cos();
        Vector3::new(c * v.x - s * v.z, v.y, s * v.x + c * v.z)
    }
}

pub struct SistemaPlanetario {
    pub planetas: Vec<Planeta>,
}

impl SistemaPlanetario {
    pub fn nuevo() -> Self {
        let terra = PlanetaConfig {
            nombre: "TERRA",
            radio: 150.,
            semilla: 42,
            orbita_radio: 800.,
            orbita_inicial: -std::f32::consts::FRAC_PI_2, // justo frente a la nave al iniciar
            orbita_velocidad: 0.01,
            rotacion_velocidad: 0.05,
            subdivisiones: 5,
            relieve: 0.12,
        };
        Self {
            planetas: vec![Planeta::nuevo(terra)],
        }
    }

    pub fn actualizar(&mut self, dt: f32) {
        for planeta in &mut self.planetas {
            planeta.actualizar(dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> PlanetaConfig {
        PlanetaConfig {
            nombre: "T",
            radio: 100.,
            semilla: 7,
            orbita_radio: 500.,
            orbita_inicial: 0.,
            orbita_velocidad: 0.1,
            rotacion_velocidad: 0.2,
            subdivisiones: 2,
            relieve: 0.1,
        }
    }
    #[test]
    fn mesh_is_reproducible_and_outward() {
        let a = Planeta::nuevo(config());
        let b = Planeta::nuevo(config());
        assert_eq!(a.triangulos.len(), 20 * 16);
        for (ta, tb) in a.triangulos.iter().zip(&b.triangulos) {
            assert!((ta[0] - tb[0]).length() < 1e-6);
            let normal = (ta[1] - ta[0]).cross(ta[2] - ta[0]);
            assert!(normal.dot(ta[0] + ta[1] + ta[2]) > 0.);
        }
    }
    #[test]
    fn surface_radius_within_relief_bounds() {
        let p = Planeta::nuevo(config());
        for d in [Vector3::new(1., 0., 0.), Vector3::new(0.3, 0.8, -0.5).normalized()] {
            let r = p.radio_superficie(d);
            assert!(r >= 100. - 1e-3 && r <= 100. * 1.12);
        }
    }
    #[test]
    fn local_world_rotation_roundtrip() {
        let mut p = Planeta::nuevo(config());
        p.actualizar(3.);
        let v = Vector3::new(1., 2., 3.);
        assert!((p.a_local(p.a_mundo(v)) - v).length() < 1e-4);
    }
}
