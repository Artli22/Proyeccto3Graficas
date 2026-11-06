
use crate::pipeline::{Mat4, Uniforms, FAR, NEAR};
use crate::sol::{Sol, RADIO_SOL};
use cohete::color::Relleno;
use raylib::prelude::{Color, Vector3};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub enum Aspecto {
    Tierra,
    Rocoso {
        oscuro: [f32; 3],
        claro: [f32; 3],
        casquetes: bool,
    },
    Bandas {
        a: [f32; 3],
        b: [f32; 3],
        frecuencia: f32,
        turbulencia: f32,
    },
}

#[derive(Clone, Copy)]
pub struct PlanetaConfig {
    pub nombre: &'static str,
    pub radio: f32,
    pub semilla: u32,
    pub padre: Option<usize>,
    pub orbita_radio: f32,
    pub orbita_inicial: f32,
    pub orbita_velocidad: f32,   
    pub rotacion_velocidad: f32, 
    pub inclinacion: f32,        
    pub subdivisiones: u32,
    pub relieve: f32, 
    pub aspecto: Aspecto,
    pub anillo: Option<(f32, f32)>,
    /// Aceleracion gravitatoria en la superficie (unidades/s^2). `None` la deriva del radio
    /// (`GRAVEDAD_POR_RADIO * radio`), de modo que los cuerpos grandes atraen mas.
    pub gravedad: Option<f32>,
}

/// Gravedad superficial por unidad de radio cuando el planeta no define `gravedad`.
pub const GRAVEDAD_POR_RADIO: f32 = 0.01;
/// Alcance de la gravedad sobre la superficie, en radios del cuerpo.
pub const GRAVEDAD_ALCANCE: f32 = 4.;

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

fn fbm(dir: Vector3, semilla: u32) -> f32 {
    fbm_n(dir, semilla, 5, 1.8)
}

// Ruido fractal con `octavas` capas; menos capas cuestan menos por fragmento.
pub(crate) fn fbm_n(dir: Vector3, semilla: u32, octavas: u32, frecuencia_base: f32) -> f32 {
    let (mut total, mut amplitud, mut frecuencia, mut suma) = (0., 1., frecuencia_base, 0.);
    for octava in 0..octavas {
        total += ruido(dir * frecuencia, semilla.wrapping_add(octava * 101)) * amplitud;
        suma += amplitud;
        amplitud *= 0.5;
        frecuencia *= 2.;
    }
    total / suma
}

pub(crate) fn mezcla(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

impl Aspecto {
    /// Color representativo del cuerpo, para el mapa y la lista del HUD.
    pub fn color(&self) -> Color {
        let c = match *self {
            Aspecto::Tierra => [40., 110., 190.],
            Aspecto::Rocoso { oscuro, claro, .. } => mezcla(oscuro, claro, 0.5),
            Aspecto::Bandas { a, b, .. } => mezcla(a, b, 0.5),
        };
        Color::new(c[0] as u8, c[1] as u8, c[2] as u8, 255)
    }
}

fn a_color(c: [f32; 3], variacion: f32) -> Color {
    let v = 1. + (variacion - 0.5) * 0.12;
    let canal = |x: f32| (x * v).clamp(0., 255.) as u8;
    Color::new(canal(c[0]), canal(c[1]), canal(c[2]), 255)
}

fn color_bioma(n: f32) -> [f32; 3] {
    if n < NIVEL_MAR - 0.08 {
        [16., 48., 120.]
    } else if n < NIVEL_MAR {
        [30., 92., 170.]
    } else if n < NIVEL_MAR + 0.03 {
        [214., 196., 140.]
    } else if n < NIVEL_MAR + 0.11 {
        [74., 150., 66.]
    } else if n < NIVEL_MAR + 0.18 {
        [46., 110., 52.]
    } else if n < NIVEL_MAR + 0.25 {
        [128., 118., 108.]
    } else {
        [240., 242., 248.]
    }
}

fn color_triangulo(aspecto: Aspecto, n: f32, dir: Vector3, variacion: f32) -> Color {
    let base = match aspecto {
        Aspecto::Tierra => color_bioma(n),
        Aspecto::Rocoso {
            oscuro,
            claro,
            casquetes,
        } => {
            if casquetes && dir.y.abs() > 0.93 {
                [236., 240., 245.]
            } else {
                mezcla(oscuro, claro, (n - 0.3) / 0.4)
            }
        }
        Aspecto::Bandas {
            a,
            b,
            frecuencia,
            turbulencia,
        } => {
            let fase = dir.y * frecuencia + (n - 0.5) * turbulencia * std::f32::consts::TAU;
            mezcla(a, b, 0.5 + 0.5 * fase.sin())
        }
    };
    a_color(base, variacion)
}

fn radio_en(config: &PlanetaConfig, n: f32) -> f32 {
    match config.aspecto {
        Aspecto::Tierra => {
            config.radio * (1. + config.relieve * (n.max(NIVEL_MAR) - NIVEL_MAR) * 2.)
        }
        Aspecto::Rocoso { .. } => config.radio * (1. + config.relieve * (n - 0.5) * 2.),
        Aspecto::Bandas { .. } => config.radio,
    }
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

pub struct Malla {
    pub triangulos: Vec<[Vector3; 3]>,
    pub normales: Vec<Vector3>,
    pub rellenos: Vec<Relleno>,
}

/// Esfera lisa de un solo color, con las normales hacia afuera (la usa el sol).
pub(crate) fn malla_esfera(radio: f32, subdivisiones: u32, color: Color) -> Malla {
    let (mut vertices, mut caras) = icosaedro();
    for _ in 0..subdivisiones {
        caras = subdividir(&mut vertices, &caras);
    }
    let mut malla = Malla {
        triangulos: Vec::with_capacity(caras.len()),
        normales: Vec::with_capacity(caras.len()),
        rellenos: Vec::with_capacity(caras.len()),
    };
    for cara in &caras {
        let mut tri = cara.map(|v| vertices[v] * radio);
        let centroide = (tri[0] + tri[1] + tri[2]) / 3.;
        let mut normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
        if normal.dot(centroide) < 0. {
            tri.swap(1, 2);
            normal = -normal;
        }
        malla.triangulos.push(tri);
        malla.normales.push(normal.normalized());
        malla.rellenos.push(Relleno::Solido(color));
    }
    malla
}

/// Descarte de una esfera contra el frustum. Devuelve su radio aparente en pixeles o `None`
/// si no puede verse. `radio_limite` envuelve todo el cuerpo (anillos incluidos).
pub(crate) fn proyeccion_esfera(
    u: &Uniforms,
    centro: Vector3,
    radio_limite: f32,
    radio: f32,
) -> Option<f32> {
    let c = u.camera_point(centro);
    let r = radio_limite;
    if c.z + r < NEAR || c.z - r > FAR {
        return None;
    }
    let (a, b) = (u.projection.0[0][0], u.projection.0[1][1]);
    for (coef, eje) in [(a, c.x), (b, c.y)] {
        let norma = (1. + coef * coef).sqrt();
        if c.z + coef * eje < -r * norma || c.z - coef * eje < -r * norma {
            return None;
        }
    }
    if c.z <= radio {
        return Some(f32::MAX);
    }
    Some(radio * u.height / c.z)
}

fn construir_malla(config: &PlanetaConfig, subdivisiones: u32) -> Malla {
    let (mut vertices, mut caras) = icosaedro();
    for _ in 0..subdivisiones {
        caras = subdividir(&mut vertices, &caras);
    }
    let alturas: Vec<f32> = vertices.iter().map(|&d| fbm(d, config.semilla)).collect();
    let posiciones: Vec<Vector3> = vertices
        .iter()
        .zip(&alturas)
        .map(|(&d, &n)| d * radio_en(config, n))
        .collect();
    let mut malla = Malla {
        triangulos: Vec::with_capacity(caras.len()),
        normales: Vec::with_capacity(caras.len()),
        rellenos: Vec::with_capacity(caras.len()),
    };
    for (i, cara) in caras.iter().enumerate() {
        let mut tri = cara.map(|v| posiciones[v]);
        let centroide = (tri[0] + tri[1] + tri[2]) / 3.;
        let mut normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
        if normal.dot(centroide) < 0. {
            tri.swap(1, 2);
            normal = -normal;
        }
        if normal.length() < 1e-9 {
            continue;
        }
        let n = cara.iter().map(|&v| alturas[v]).sum::<f32>() / 3.;
        let dir = (vertices[cara[0]] + vertices[cara[1]] + vertices[cara[2]]).normalized();
        let variacion = hash(i as i32, 7, 13, config.semilla);
        malla.triangulos.push(tri);
        malla.normales.push(normal.normalized());
        malla
            .rellenos
            .push(Relleno::Solido(color_triangulo(config.aspecto, n, dir, variacion)));
    }
    malla
}

fn construir_anillo(config: &PlanetaConfig, interior: f32, exterior: f32) -> Malla {
    const SEGMENTOS: usize = 96;
    const BANDAS: usize = 8;
    let mut malla = Malla {
        triangulos: Vec::new(),
        normales: Vec::new(),
        rellenos: Vec::new(),
    };
    let punto = |radio: f32, angulo: f32| {
        let (s, c) = angulo.sin_cos();
        Vector3::new(c * radio, 0., s * radio)
    };
    for banda in 0..BANDAS {
        if banda == 5 {
            continue; 
        }
        let r0 = config.radio * (interior + (exterior - interior) * banda as f32 / BANDAS as f32);
        let r1 =
            config.radio * (interior + (exterior - interior) * (banda + 1) as f32 / BANDAS as f32);
        let brillo = 0.6 + 0.4 * hash(banda as i32, 3, 5, config.semilla);
        let color = Color::new(
            (210. * brillo) as u8,
            (190. * brillo) as u8,
            (150. * brillo) as u8,
            255,
        );
        for s in 0..SEGMENTOS {
            let a0 = s as f32 / SEGMENTOS as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / SEGMENTOS as f32 * std::f32::consts::TAU;
            let q = [punto(r0, a0), punto(r0, a1), punto(r1, a1), punto(r1, a0)];
            for tri in [
                [q[0], q[1], q[2]],
                [q[0], q[2], q[3]],
                [q[0], q[2], q[1]],
                [q[0], q[3], q[2]],
            ] {
                malla.triangulos.push(tri);
                malla.rellenos.push(Relleno::Solido(color));
            }
        }
    }
    malla
}

fn rot_y(v: Vector3, a: f32) -> Vector3 {
    let (s, c) = a.sin_cos();
    Vector3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}

fn rot_x(v: Vector3, a: f32) -> Vector3 {
    let (s, c) = a.sin_cos();
    Vector3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
}

pub struct Planeta {
    pub config: PlanetaConfig,
    pub mallas: Vec<Malla>,
    pub anillo: Option<Malla>,
    pub centro: Vector3,
    pub delta: Vector3,
    pub angulo_rotacion: f32,
    angulo_orbita: f32,
    radio_limite: f32,
}

impl Planeta {
    pub fn nuevo(config: PlanetaConfig, centro_padre: Vector3) -> Self {
        let mut niveles: Vec<u32> = Vec::new();
        for nivel in [
            config.subdivisiones,
            config.subdivisiones.min(3),
            config.subdivisiones.min(2),
        ] {
            if niveles.last() != Some(&nivel) {
                niveles.push(nivel);
            }
        }
        let relieve = match config.aspecto {
            Aspecto::Bandas { .. } => 0.,
            _ => config.relieve,
        };
        let anillo_max = config.anillo.map_or(1., |(_, exterior)| exterior);
        let mut planeta = Self {
            config,
            mallas: niveles
                .iter()
                .map(|&n| construir_malla(&config, n))
                .collect(),
            anillo: config
                .anillo
                .map(|(i, e)| construir_anillo(&config, i, e)),
            centro: Vector3::zero(),
            delta: Vector3::zero(),
            angulo_rotacion: 0.,
            angulo_orbita: config.orbita_inicial,
            radio_limite: config.radio * (1. + relieve).max(anillo_max),
        };
        planeta.centro = centro_padre + planeta.posicion_orbital();
        planeta
    }

    /// Distancia del centro a la superficie en la direccion local `dir` (unitaria).
    pub fn radio_superficie(&self, dir: Vector3) -> f32 {
        radio_en(&self.config, fbm(dir, self.config.semilla))
    }

    /// Aceleracion gravitatoria en la superficie (unidades/s^2).
    pub fn gravedad_superficie(&self) -> f32 {
        self.config
            .gravedad
            .unwrap_or(GRAVEDAD_POR_RADIO * self.config.radio)
            .max(0.)
    }

    /// Aceleracion (unidades/s^2) que este cuerpo ejerce sobre un punto del mundo: apunta al
    /// centro, cae con el cuadrado de la distancia y se apaga suavemente al llegar al alcance.
    pub fn aceleracion_gravedad(&self, punto: Vector3) -> Vector3 {
        let radio = self.config.radio;
        let rel = self.centro - punto;
        let distancia = rel.length().max(radio);
        let alcance = GRAVEDAD_ALCANCE * radio;
        let t = (1. - (distancia - radio) / alcance).clamp(0., 1.);
        let ventana = t * t * (3. - 2. * t);
        let magnitud = self.gravedad_superficie() * (radio / distancia).powi(2) * ventana;
        if rel.length() < 1e-3 {
            return Vector3::zero();
        }
        rel.normalized() * magnitud
    }

    fn posicion_orbital(&self) -> Vector3 {
        let (s, c) = self.angulo_orbita.sin_cos();
        Vector3::new(c * self.config.orbita_radio, 0., s * self.config.orbita_radio)
    }

    /// Avanza orbita y rotacion; `centro_padre` es el centro actual del cuerpo que orbita.
    pub fn actualizar(&mut self, dt: f32, centro_padre: Vector3) {
        self.angulo_orbita = (self.angulo_orbita + self.config.orbita_velocidad * dt)
            .rem_euclid(std::f32::consts::TAU);
        self.angulo_rotacion = (self.angulo_rotacion + self.config.rotacion_velocidad * dt)
            .rem_euclid(std::f32::consts::TAU);
        let nuevo = centro_padre + self.posicion_orbital();
        self.delta = nuevo - self.centro;
        self.centro = nuevo;
    }

    pub fn matriz_modelo(&self) -> Mat4 {
        let (sr, cr) = self.angulo_rotacion.sin_cos();
        let (si, ci) = self.config.inclinacion.sin_cos();
        let giro = Mat4([
            [cr, 0., sr, 0.],
            [0., 1., 0., 0.],
            [-sr, 0., cr, 0.],
            [0., 0., 0., 1.],
        ]);
        let inclinar = Mat4([
            [1., 0., 0., 0.],
            [0., ci, -si, 0.],
            [0., si, ci, 0.],
            [0., 0., 0., 1.],
        ]);
        let mut trasladar = Mat4::identity();
        trasladar.0[0][3] = self.centro.x;
        trasladar.0[1][3] = self.centro.y;
        trasladar.0[2][3] = self.centro.z;
        trasladar.mul(inclinar).mul(giro)
    }

    /// Vector local -> mundo (solo rotacion).
    pub fn a_mundo(&self, v: Vector3) -> Vector3 {
        rot_x(rot_y(v, self.angulo_rotacion), self.config.inclinacion)
    }

    /// Vector mundo -> local (solo rotacion).
    pub fn a_local(&self, v: Vector3) -> Vector3 {
        rot_y(rot_x(v, -self.config.inclinacion), -self.angulo_rotacion)
    }

    /// Descarte por esfera contra el frustum. Devuelve el radio aparente en pixeles
    /// o `None` si el cuerpo no puede verse.
    pub fn proyeccion(&self, u: &Uniforms) -> Option<f32> {
        proyeccion_esfera(u, self.centro, self.radio_limite, self.config.radio)
    }

    /// Malla segun el tamano aparente: menos triangulos cuanto mas pequeno se ve.
    pub fn malla_lod(&self, radio_px: f32) -> &Malla {
        let nivel = if radio_px >= 120. {
            0
        } else if radio_px >= 35. {
            1
        } else {
            2
        };
        &self.mallas[nivel.min(self.mallas.len() - 1)]
    }
}

pub struct SistemaPlanetario {
    pub planetas: Vec<Planeta>,
    /// Fuente de luz y centro de las orbitas.
    pub sol: Sol,
    /// Segundos simulados; anima el shader del sol.
    pub tiempo: f32,
}

const RADIO_TIERRA: f32 = 150.;

impl SistemaPlanetario {
    pub fn nuevo() -> Self {
        use std::f32::consts::{FRAC_PI_2, PI};
        let rocoso = |oscuro, claro, casquetes| Aspecto::Rocoso {
            oscuro,
            claro,
            casquetes,
        };
        let bandas = |a, b, frecuencia, turbulencia| Aspecto::Bandas {
            a,
            b,
            frecuencia,
            turbulencia,
        };
        
        let base = PlanetaConfig {
            nombre: "",
            radio: 0.,
            semilla: 0,
            padre: None,
            orbita_radio: 0.,
            orbita_inicial: 0.,
            orbita_velocidad: 0.,
            rotacion_velocidad: 0.04,
            inclinacion: 0.,
            subdivisiones: 4,
            relieve: 0.,
            aspecto: Aspecto::Tierra,
            anillo: None,
            gravedad: None,
        };
        let configs = [
            PlanetaConfig {
                nombre: "MERCURIO",
                radio: 20.,
                semilla: 11,
                orbita_radio: 1500.,
                orbita_inicial: 0.8,
                orbita_velocidad: 0.0133,
                subdivisiones: 3,
                relieve: 0.12,
                aspecto: rocoso([70., 66., 62.], [165., 156., 146.], false),
                ..base
            },
            PlanetaConfig {
                nombre: "VENUS",
                radio: 140.,
                semilla: 23,
                orbita_radio: 2500.,
                orbita_inicial: 2.4,
                orbita_velocidad: 0.0072,
                inclinacion: 3.09,
                aspecto: bandas([214., 168., 88.], [244., 220., 150.], 2.5, 1.4),
                ..base
            },
            PlanetaConfig {
                nombre: "TERRA",
                radio: RADIO_TIERRA,
                semilla: 42,
                orbita_radio: 3600.,
                orbita_inicial: -FRAC_PI_2, // frente a la nave al iniciar
                orbita_velocidad: 0.0042,
                rotacion_velocidad: 0.05,
                inclinacion: 0.41,
                subdivisiones: 5,
                relieve: 0.12,
                ..base
            },
            PlanetaConfig {
                nombre: "LUNA",
                radio: RADIO_TIERRA * 0.2727,
                semilla: 57,
                padre: Some(2),
                orbita_radio: 1100.,
                orbita_inicial: -FRAC_PI_2 + 0.5, // visible desde el punto de aparicion de la nave
                orbita_velocidad: 0.04,
                rotacion_velocidad: 0.04, // rotacion sincrona: siempre muestra la misma cara
                relieve: 0.08,
                aspecto: rocoso([88., 88., 92.], [196., 196., 200.], false),
                ..base
            },
            PlanetaConfig {
                nombre: "MARTE",
                radio: 80.,
                semilla: 71,
                orbita_radio: 4800.,
                orbita_inicial: 4.0,
                orbita_velocidad: 0.0027,
                inclinacion: 0.44,
                relieve: 0.15,
                aspecto: rocoso([110., 46., 28.], [205., 118., 72.], true),
                ..base
            },
            PlanetaConfig {
                nombre: "JUPITER",
                radio: 600.,
                semilla: 83,
                orbita_radio: 7500.,
                orbita_inicial: 5.5,
                orbita_velocidad: 0.0013,
                rotacion_velocidad: 0.03,
                inclinacion: 0.05,
                subdivisiones: 5,
                aspecto: bandas([168., 110., 74.], [240., 214., 176.], 9., 0.8),
                ..base
            },
            PlanetaConfig {
                nombre: "SATURNO",
                radio: 520.,
                semilla: 97,
                orbita_radio: 10500.,
                orbita_inicial: 1.2,
                orbita_velocidad: 0.00076,
                rotacion_velocidad: 0.03,
                inclinacion: 0.47,
                subdivisiones: 5,
                aspecto: bandas([200., 168., 108.], [243., 228., 184.], 7., 0.5),
                anillo: Some((1.35, 2.3)),
                ..base
            },
            PlanetaConfig {
                nombre: "URANO",
                radio: 300.,
                semilla: 101,
                orbita_radio: 13500.,
                orbita_inicial: 3.3,
                orbita_velocidad: 0.00052,
                inclinacion: 1.7,
                aspecto: bandas([112., 196., 208.], [166., 228., 232.], 4., 0.3),
                ..base
            },
            PlanetaConfig {
                nombre: "NEPTUNO",
                radio: 290.,
                semilla: 113,
                orbita_radio: 16500.,
                orbita_inicial: PI * 0.2,
                orbita_velocidad: 0.00036,
                inclinacion: 0.49,
                aspecto: bandas([34., 62., 186.], [74., 116., 232.], 6., 0.7),
                ..base
            },
            PlanetaConfig {
                nombre: "PLUTON",
                radio: 10.,
                semilla: 127,
                orbita_radio: 19500.,
                orbita_inicial: 4.7,
                orbita_velocidad: 0.00026,
                rotacion_velocidad: 0.02,
                inclinacion: 2.1,
                subdivisiones: 3,
                relieve: 0.15,
                aspecto: rocoso([118., 84., 62.], [236., 216., 196.], false),
                ..base
            },
        ];
        let mut planetas: Vec<Planeta> = Vec::new();
        for config in configs {
            let centro_padre = config.padre.map_or(Vector3::zero(), |p| planetas[p].centro);
            planetas.push(Planeta::nuevo(config, centro_padre));
        }
        Self {
            planetas,
            sol: Sol::nuevo(RADIO_SOL),
            tiempo: 0.,
        }
    }

    /// Los padres van antes que sus satelites, asi el hijo hereda el movimiento ya actualizado.
    pub fn actualizar(&mut self, dt: f32) {
        self.tiempo += dt;
        for i in 0..self.planetas.len() {
            let centro_padre = self.planetas[i]
                .config
                .padre
                .map_or(Vector3::zero(), |p| self.planetas[p].centro);
            self.planetas[i].actualizar(dt, centro_padre);
        }
    }

    #[cfg(test)]
    pub fn indice(&self, nombre: &str) -> Option<usize> {
        self.planetas.iter().position(|p| p.config.nombre == nombre)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(orbita_inicial: f32) -> PlanetaConfig {
        PlanetaConfig {
            nombre: "T",
            radio: 100.,
            semilla: 7,
            padre: None,
            orbita_radio: 500.,
            orbita_inicial,
            orbita_velocidad: 0.1,
            rotacion_velocidad: 0.2,
            inclinacion: 0.3,
            subdivisiones: 2,
            relieve: 0.1,
            aspecto: Aspecto::Tierra,
            anillo: None,
            gravedad: None,
        }
    }
    #[test]
    fn mesh_is_reproducible_and_outward() {
        let a = Planeta::nuevo(config(0.), Vector3::zero());
        let b = Planeta::nuevo(config(0.), Vector3::zero());
        assert_eq!(a.mallas[0].triangulos.len(), 20 * 16);
        for (ta, tb) in a.mallas[0].triangulos.iter().zip(&b.mallas[0].triangulos) {
            assert!((ta[0] - tb[0]).length() < 1e-6);
            let normal = (ta[1] - ta[0]).cross(ta[2] - ta[0]);
            assert!(normal.dot(ta[0] + ta[1] + ta[2]) > 0.);
        }
    }
    #[test]
    fn surface_radius_within_relief_bounds() {
        let p = Planeta::nuevo(config(0.), Vector3::zero());
        for d in [Vector3::new(1., 0., 0.), Vector3::new(0.3, 0.8, -0.5).normalized()] {
            let r = p.radio_superficie(d);
            assert!(r >= 100. - 1e-3 && r <= 100. * 1.1 + 1e-3);
        }
    }
    #[test]
    fn gravity_points_to_center_scales_with_size_and_is_overridable() {
        let p = Planeta::nuevo(config(0.), Vector3::zero());
        let punto = p.centro + Vector3::new(0., 200., 0.);
        let a = p.aceleracion_gravedad(punto);
        assert!(a.y < 0. && a.x.abs() < 1e-4 && a.z.abs() < 1e-4);
        assert!(p.aceleracion_gravedad(p.centro + Vector3::new(0., 150., 0.)).length() > a.length());
        let lejos = p.centro + Vector3::new(0., 100. * (GRAVEDAD_ALCANCE + 1.5), 0.);
        assert_eq!(p.aceleracion_gravedad(lejos).length(), 0.);
        let mut grande = config(0.);
        grande.radio = 200.;
        let g = Planeta::nuevo(grande, Vector3::zero());
        assert!(g.gravedad_superficie() > p.gravedad_superficie());
        let mut fija = config(0.);
        fija.gravedad = Some(9.);
        assert_eq!(Planeta::nuevo(fija, Vector3::zero()).gravedad_superficie(), 9.);
    }
    #[test]
    fn local_world_rotation_roundtrip() {
        let mut p = Planeta::nuevo(config(0.), Vector3::zero());
        p.actualizar(3., Vector3::zero());
        let v = Vector3::new(1., 2., 3.);
        assert!((p.a_local(p.a_mundo(v)) - v).length() < 1e-4);
    }
    #[test]
    fn model_matrix_matches_a_mundo() {
        let mut p = Planeta::nuevo(config(1.), Vector3::zero());
        p.actualizar(2., Vector3::zero());
        let v = Vector3::new(3., -1., 2.);
        let m = p.matriz_modelo().transform([v.x, v.y, v.z, 1.]);
        let esperado = p.centro + p.a_mundo(v);
        assert!((Vector3::new(m[0], m[1], m[2]) - esperado).length() < 1e-3);
    }
    #[test]
    fn frustum_culls_bodies_outside_view() {
        let u = Uniforms::scene(Mat4::identity(), Mat4::identity(), 1200, 800);
        let enfrente = Planeta::nuevo(config(std::f32::consts::FRAC_PI_2), Vector3::zero());
        let detras = Planeta::nuevo(config(-std::f32::consts::FRAC_PI_2), Vector3::zero());
        let lateral = Planeta::nuevo(config(0.), Vector3::zero());
        assert!(enfrente.proyeccion(&u).is_some());
        assert!(detras.proyeccion(&u).is_none());
        assert!(lateral.proyeccion(&u).is_none());
    }
    #[test]
    fn lod_uses_fewer_triangles_when_far() {
        let mut c = config(0.);
        c.subdivisiones = 5;
        let p = Planeta::nuevo(c, Vector3::zero());
        assert!(p.malla_lod(500.).triangulos.len() > p.malla_lod(10.).triangulos.len());
    }
    #[test]
    fn sun_sits_at_origin_and_planets_orbit_outside_it() {
        let mut s = SistemaPlanetario::nuevo();
        assert_eq!(s.sol.centro, Vector3::zero());
        for p in s.planetas.iter().filter(|p| p.config.padre.is_none()) {
            assert!(p.config.orbita_radio - p.config.radio > s.sol.radio + 100.);
        }
        s.actualizar(2.);
        assert!((s.tiempo - 2.).abs() < 1e-6);
    }

    #[test]
    fn system_has_requested_bodies_and_scales() {
        let s = SistemaPlanetario::nuevo();
        let radio = |n: &str| s.planetas[s.indice(n).unwrap()].config.radio;
        assert_eq!(s.planetas.len(), 10);
        assert!(radio("MERCURIO") < radio("MARTE") && radio("PLUTON") < radio("MERCURIO"));
        assert!((radio("LUNA") / radio("TERRA") - 0.2727).abs() < 1e-3);
        let luna = &s.planetas[s.indice("LUNA").unwrap()];
        let tierra = &s.planetas[s.indice("TERRA").unwrap()];
        assert!(((luna.centro - tierra.centro).length() - luna.config.orbita_radio).abs() < 1e-2);
    }
}
