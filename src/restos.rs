
use crate::modelo_planeta::simplificar;
use crate::pipeline::Mat4;
use crate::planeta::{Malla, SistemaPlanetario};
use crate::vuelo::Nave;
use cohete::color::Relleno;
use raylib::prelude::{Color, Vector3};
use std::fs;

pub const ESCALA_RESTOS: f32 = 1.5;

const ELASTICIDAD_NAVE: f32 = 0.4;
const ELASTICIDAD_RESTOS: f32 = 0.6;
const MASA_NAVE: f32 = 1.;
const MASA_RESTO: f32 = 4.;
const VELOCIDAD_MAXIMA: f32 = 80.;
const REBOTE_MAXIMO_NAVE: f32 = 60.;
const GIRO_MAXIMO: f32 = 1.5;
const RELAJACION_VELOCIDAD: f32 = 0.2;
const RELAJACION_GIRO: f32 = 0.3;
const HERENCIA_DERIVA: f32 = 0.3;
const DERIVA_MAXIMA: f32 = 6.;
const CELDAS_LOD: [u32; 3] = [40, 16, 6];
const RADIO_MINIMO_PX: f32 = 1.5;

const PALETAS: [[[u8; 3]; 4]; 5] = [
    [[40, 190, 110], [34, 38, 42], [185, 195, 200], [110, 72, 40]],
    [[47, 60, 160], [28, 28, 40], [170, 175, 210], [90, 48, 110]],
    [[225, 105, 30], [44, 40, 38], [205, 205, 200], [135, 58, 28]],
    [[190, 38, 48], [40, 34, 36], [200, 190, 185], [96, 44, 40]],
    [[40, 170, 190], [30, 40, 46], [200, 215, 220], [60, 100, 110]],
];


struct Colocacion {
    radio: f32,
    angulo: f32,
    altura: f32,
    rapidez: f32,
    rumbo: f32,
    giro: [f32; 3],
}

const COLOCACIONES: [Colocacion; 5] = [
    Colocacion { radio: 1300., angulo: 1.0, altura: 80., rapidez: 0.6, rumbo: 0.5, giro: [0.10, 0.22, 0.05] },
    Colocacion { radio: 2650., angulo: 4.2, altura: -120., rapidez: 0.9, rumbo: 2.0, giro: [0.18, 0.05, 0.12] },
    Colocacion { radio: 4100., angulo: 5.2, altura: 200., rapidez: 0.5, rumbo: 4.0, giro: [0.06, 0.15, 0.20] },
    Colocacion { radio: 8100., angulo: 2.0, altura: -300., rapidez: 1.2, rumbo: 1.0, giro: [0.22, 0.10, 0.04] },
    Colocacion { radio: 13600., angulo: 0.3, altura: 150., rapidez: 0.8, rumbo: 3.0, giro: [0.05, 0.12, 0.25] },
];

type Rot3 = [[f32; 3]; 3];

const IDENTIDAD: Rot3 = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];

fn mul3(a: &Rot3, b: &Rot3) -> Rot3 {
    let mut out = [[0.; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn rotar(rot: &mut Rot3, giro: Vector3, dt: f32) {
    let w = giro.length();
    if w < 1e-6 {
        return;
    }
    let e = giro / w;
    let (s, c) = (w * dt).sin_cos();
    let k: Rot3 = [[0., -e.z, e.y], [e.z, 0., -e.x], [-e.y, e.x, 0.]];
    let k2 = mul3(&k, &k);
    let mut delta = IDENTIDAD;
    for i in 0..3 {
        for j in 0..3 {
            delta[i][j] += s * k[i][j] + (1. - c) * k2[i][j];
        }
    }
    let nueva = mul3(&delta, rot);
    let fila = |i: usize| Vector3::new(nueva[i][0], nueva[i][1], nueva[i][2]);
    let r0 = fila(0).normalized();
    let r1 = fila(1);
    let r1 = (r1 - r0 * r1.dot(r0)).normalized();
    let r2 = r0.cross(r1);
    *rot = [[r0.x, r0.y, r0.z], [r1.x, r1.y, r1.z], [r2.x, r2.y, r2.z]];
}

fn a_mundo(rot: &Rot3, v: Vector3) -> Vector3 {
    Vector3::new(
        rot[0][0] * v.x + rot[0][1] * v.y + rot[0][2] * v.z,
        rot[1][0] * v.x + rot[1][1] * v.y + rot[1][2] * v.z,
        rot[2][0] * v.x + rot[2][1] * v.y + rot[2][2] * v.z,
    )
}

struct Geometria {
    triangulos: Vec<[Vector3; 3]>,
    /// Material (0..4) y tono (0..255) de cada triangulo.
    atributos: Vec<(u8, u8)>,
    radio_limite: f32,
    radio_colision: f32,
}

fn indice_material(nombre: &str) -> u8 {
    let base = nombre.split('.').next().unwrap_or(nombre).to_lowercase();
    match base.as_str() {
        "pintura_naranja_erosionada" => 0,
        "bordes_de_metal_expuesto" => 2,
        "oxido_e_impacto" => 3,
        _ => 1,
    }
}

fn tono_objeto(indice: u32) -> u8 {
    let h = indice.wrapping_mul(2654435761).rotate_left(13).wrapping_mul(40503);
    (h >> 8) as u8
}

fn cargar_geometria(ruta: &str, largo: f32) -> std::io::Result<Geometria> {
    let contenido = fs::read_to_string(ruta)?;
    let mut vertices: Vec<Vector3> = Vec::new();
    let mut caras: Vec<(Vec<usize>, u8, u8)> = Vec::new();
    let mut material = 1u8;
    let mut objeto = 0u32;
    for linea in contenido.lines() {
        let partes: Vec<&str> = linea.split_whitespace().collect();
        match partes.first().copied() {
            Some("o") => objeto += 1,
            Some("usemtl") if partes.len() >= 2 => material = indice_material(partes[1]),
            Some("v") if partes.len() >= 4 => {
                let c = |i: usize| partes[i].parse::<f32>().unwrap_or(0.);
                vertices.push(Vector3::new(c(1), c(2), c(3)));
            }
            Some("f") => {
                let indices: Vec<usize> = partes[1..]
                    .iter()
                    .filter_map(|p| p.split('/').next()?.parse::<usize>().ok())
                    .filter(|&i| i > 0 && i <= vertices.len())
                    .map(|i| i - 1)
                    .collect();
                caras.push((indices, material, tono_objeto(objeto)));
            }
            _ => {}
        }
    }
    if vertices.is_empty() || caras.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "el OBJ del resto no tiene geometria",
        ));
    }
    let (mut min, mut max) = (vertices[0], vertices[0]);
    for v in &vertices {
        min = Vector3::new(min.x.min(v.x), min.y.min(v.y), min.z.min(v.z));
        max = Vector3::new(max.x.max(v.x), max.y.max(v.y), max.z.max(v.z));
    }
    let centro = (min + max) * 0.5;
    let dimension = (max.x - min.x).max(max.y - min.y).max(max.z - min.z).max(1e-6);
    let escala = largo / dimension;

    let mut triangulos = Vec::new();
    let mut atributos = Vec::new();
    for (indices, material, tono) in caras {
        for i in 1..indices.len().saturating_sub(1) {
            let tri = [indices[0], indices[i], indices[i + 1]].map(|k| (vertices[k] - centro) * escala);
            if (tri[1] - tri[0]).cross(tri[2] - tri[0]).length() < 1e-9 {
                continue;
            }
            triangulos.push(tri);
            atributos.push((material, tono));
        }
    }
    let distancias: Vec<f32> = triangulos.iter().flatten().map(|v| v.length()).collect();
    let radio_limite = distancias.iter().copied().fold(0., f32::max);
    let cuadratico = (distancias.iter().map(|d| d * d).sum::<f32>() / distancias.len().max(1) as f32).sqrt();
    Ok(Geometria {
        triangulos,
        atributos,
        radio_limite,
        radio_colision: (cuadratico * 1.2).min(radio_limite),
    })
}

fn malla_coloreada(geo: &Geometria, paleta: &[[u8; 3]; 4]) -> Malla {
    let mut malla = Malla {
        triangulos: Vec::with_capacity(geo.triangulos.len()),
        normales: Vec::with_capacity(geo.triangulos.len()),
        rellenos: Vec::with_capacity(geo.triangulos.len()),
    };
    for (tri, &(material, tono)) in geo.triangulos.iter().zip(&geo.atributos) {
        let factor = 0.85 + 0.3 * tono as f32 / 255.;
        let [r, g, b] = paleta[material as usize].map(|c| (c as f32 * factor).clamp(0., 255.) as u8);
        malla.triangulos.push(*tri);
        malla.normales.push((tri[1] - tri[0]).cross(tri[2] - tri[0]).normalized());
        malla.rellenos.push(Relleno::Solido(Color::new(r, g, b, 255)));
    }
    malla
}

pub struct Resto {
    pub centro: Vector3,
    pub velocidad: Vector3,
    deriva: Vector3,
    giro: Vector3,
    giro_natural: Vector3,
    rot: Rot3,
    mallas: Vec<Malla>,
    pub radio_limite: f32,
    pub radio_colision: f32,
}

impl Resto {
    pub fn matriz_modelo(&self) -> Mat4 {
        let r = &self.rot;
        Mat4([
            [r[0][0], r[0][1], r[0][2], self.centro.x],
            [r[1][0], r[1][1], r[1][2], self.centro.y],
            [r[2][0], r[2][1], r[2][2], self.centro.z],
            [0., 0., 0., 1.],
        ])
    }

    /// Malla segun el tamano aparente en pixeles; `None` si es demasiado pequeno para verse.
    pub fn malla_lod(&self, radio_px: f32) -> Option<&Malla> {
        if radio_px < RADIO_MINIMO_PX {
            return None;
        }
        let nivel = match radio_px {
            r if r >= 160. => 0,
            r if r >= 50. => 1,
            r if r >= 14. => 2,
            _ => 3,
        };
        Some(&self.mallas[nivel.min(self.mallas.len() - 1)])
    }

    fn avanzar(&mut self, dt: f32) {
        self.centro += self.velocidad * dt;
        rotar(&mut self.rot, self.giro, dt);
        // Tras un golpe vuelve poco a poco a su deriva y giro naturales.
        self.velocidad = self.deriva + (self.velocidad - self.deriva) * (-RELAJACION_VELOCIDAD * dt).exp();
        self.giro = self.giro_natural + (self.giro - self.giro_natural) * (-RELAJACION_GIRO * dt).exp();
    }

    fn limitar(&mut self) {
        if self.velocidad.length() > VELOCIDAD_MAXIMA {
            self.velocidad = self.velocidad.normalized() * VELOCIDAD_MAXIMA;
        }
        if self.giro.length() > GIRO_MAXIMO {
            self.giro = self.giro.normalized() * GIRO_MAXIMO;
        }
        if self.deriva.length() > DERIVA_MAXIMA {
            self.deriva = self.deriva.normalized() * DERIVA_MAXIMA;
        }
    }

    /// Rebota contra una esfera fija o en movimiento.
    fn rebotar_en_esfera(&mut self, centro: Vector3, radio: f32) {
        let rel = self.centro - centro;
        let distancia = rel.length();
        let limite = radio + self.radio_colision;
        if distancia >= limite {
            return;
        }
        let normal = if distancia > 1e-4 { rel / distancia } else { Vector3::new(0., 1., 0.) };
        self.centro = centro + normal * limite;
        for v in [&mut self.velocidad, &mut self.deriva] {
            let normal_v = v.dot(normal);
            if normal_v < 0. {
                *v -= normal * (1. + ELASTICIDAD_RESTOS) * normal_v;
            }
        }
    }
}

pub struct Restos {
    pub restos: Vec<Resto>,
    previa: Option<Vector3>,
}

impl Restos {
    pub fn vacio() -> Self {
        Self { restos: Vec::new(), previa: None }
    }

    /// Carga el OBJ una vez y crea los restos; `tamano_nave` es la dimension mayor de la nave.
    pub fn nuevo(ruta: &str, tamano_nave: f32) -> std::io::Result<Self> {
        let geo = cargar_geometria(ruta, tamano_nave * ESCALA_RESTOS)?;
        let restos = COLOCACIONES
            .iter()
            .zip(PALETAS.iter())
            .map(|(c, paleta)| {
                let completa = malla_coloreada(&geo, paleta);
                let mut mallas: Vec<Malla> = CELDAS_LOD
                    .map(|celdas| simplificar(&completa, geo.radio_limite, celdas))
                    .into();
                mallas.insert(0, completa);
                let deriva = Vector3::new(c.rumbo.cos(), 0.1, c.rumbo.sin()).normalized() * c.rapidez;
                let giro = Vector3::new(c.giro[0], c.giro[1], c.giro[2]);
                Resto {
                    centro: Vector3::new(c.radio * c.angulo.cos(), c.altura, c.radio * c.angulo.sin()),
                    velocidad: deriva,
                    deriva,
                    giro,
                    giro_natural: giro,
                    rot: IDENTIDAD,
                    mallas,
                    radio_limite: geo.radio_limite,
                    radio_colision: geo.radio_colision,
                }
            })
            .collect();
        Ok(Self { restos, previa: None })
    }

    pub fn olvidar(&mut self) {
        self.previa = None;
    }

    pub fn actualizar(&mut self, dt: f32, sistema: &SistemaPlanetario) {
        for r in &mut self.restos {
            r.avanzar(dt);
            r.rebotar_en_esfera(sistema.sol.centro, sistema.sol.radio);
            for p in &sistema.planetas {
                let radio = p.config.radio * p.config.anillo.map_or(1.15, |(_, exterior)| exterior);
                r.rebotar_en_esfera(p.centro, radio);
            }
        }
        for i in 0..self.restos.len() {
            let (antes, despues) = self.restos.split_at_mut(i + 1);
            for b in despues {
                Self::chocar_entre_si(&mut antes[i], b);
            }
        }
        for r in &mut self.restos {
            r.limitar();
        }
    }

    fn chocar_entre_si(a: &mut Resto, b: &mut Resto) {
        let rel = b.centro - a.centro;
        let distancia = rel.length();
        let limite = a.radio_colision + b.radio_colision;
        if distancia >= limite {
            return;
        }
        let normal = if distancia > 1e-4 { rel / distancia } else { Vector3::new(0., 1., 0.) };
        let penetracion = limite - distancia;
        a.centro -= normal * (penetracion * 0.5);
        b.centro += normal * (penetracion * 0.5);
        let aproximacion = (a.velocidad - b.velocidad).dot(normal);
        if aproximacion > 0. {
            // Misma masa: se intercambian las componentes normales (con perdida).
            let j = (1. + ELASTICIDAD_RESTOS) * aproximacion * 0.5;
            a.velocidad -= normal * j;
            b.velocidad += normal * j;
            a.deriva += (b.centro - a.centro).normalized() * -j * HERENCIA_DERIVA;
            b.deriva += (b.centro - a.centro).normalized() * j * HERENCIA_DERIVA;
        }
    }

    pub fn colisionar_nave(&mut self, nave: &mut Nave, radio_nave: f32, fija: bool) -> bool {
        let inicio = self.previa.unwrap_or(nave.position);
        let mut choque = false;
        for resto in &mut self.restos {
            let fin = nave.position;
            let tramo = fin - inicio;
            let suma = radio_nave + resto.radio_colision;
            let largo2 = tramo.dot(tramo);
            let t = if largo2 > 1e-9 {
                ((resto.centro - inicio).dot(tramo) / largo2).clamp(0., 1.)
            } else {
                0.
            };
            if (inicio + tramo * t - resto.centro).length() >= suma {
                continue;
            }
            // Primer instante del tramo en que la nave toca el resto.
            let o = inicio - resto.centro;
            let s = if largo2 > 1e-9 {
                let b = 2. * tramo.dot(o);
                let disc = b * b - 4. * largo2 * (o.dot(o) - suma * suma);
                ((-b - disc.max(0.).sqrt()) / (2. * largo2)).clamp(0., 1.)
            } else {
                0.
            };
            let contacto = inicio + tramo * s - resto.centro;
            let normal = if contacto.length() > 1e-4 {
                contacto.normalized()
            } else if largo2 > 1e-9 {
                -tramo.normalized()
            } else {
                Vector3::new(0., 1., 0.)
            };
            let sobre_nave = resto.centro + normal * (suma + 0.02);
            if fija {
                resto.centro = nave.position - normal * (suma + 0.02);
            } else {
                nave.position = sobre_nave;
            }

            let v_nave = nave.velocity + nave.deriva;
            let relativa = v_nave - resto.velocidad;
            let aproximacion = relativa.dot(normal);
            if aproximacion < 0. {
                let inversa_nave = if fija { 0. } else { 1. / MASA_NAVE };
                let inversa_resto = 1. / MASA_RESTO;
                let j = -(1. + ELASTICIDAD_NAVE) * aproximacion / (inversa_nave + inversa_resto);
                let antes = resto.velocidad;
                resto.velocidad -= normal * (j * inversa_resto);
                resto.deriva += (resto.velocidad - antes) * HERENCIA_DERIVA;
                // El roce tangencial hace girar al resto.
                let tangencial = relativa - normal * aproximacion;
                resto.giro += normal.cross(tangencial) * (0.3 / resto.radio_colision.max(1e-3));
                resto.limitar();
                if !fija {
                    let nueva = v_nave + normal * (j * inversa_nave);
                    let adelante = nave.forward();
                    let avance = nueva.dot(adelante).max(0.);
                    nave.set_speed(avance * Nave::METERS_PER_UNIT * 3.6);
                    let resto_v = nueva - adelante * avance;
                    nave.deriva = if resto_v.length() > REBOTE_MAXIMO_NAVE {
                        resto_v.normalized() * REBOTE_MAXIMO_NAVE
                    } else {
                        resto_v
                    };
                }
            }
            choque = true;
        }
        self.previa = Some(nave.position);
        choque
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modelo_planeta::color_medio;

    fn ruta() -> String {
        format!("{}/src/assets/Nave_destruida.obj", env!("CARGO_MANIFEST_DIR"))
    }

    fn restos() -> Restos {
        Restos::nuevo(&ruta(), 3.).expect("Nave_destruida.obj debe cargarse")
    }

    #[test]
    fn five_wrecks_each_with_its_own_palette_and_decreasing_detail() {
        let r = restos();
        assert_eq!(r.restos.len(), 5);
        for resto in &r.restos {
            let n: Vec<usize> = resto.mallas.iter().map(|m| m.triangulos.len()).collect();
            assert!(n[0] > 1000, "{n:?}");
            assert!(n.windows(2).all(|w| w[0] > w[1]), "{n:?}");
            assert!(resto.radio_colision > 0. && resto.radio_colision <= resto.radio_limite);
        }
        let medios: Vec<(u8, u8, u8)> = r
            .restos
            .iter()
            .map(|x| color_medio(&x.mallas[0]))
            .map(|c| (c.r, c.g, c.b))
            .collect();
        for i in 0..medios.len() {
            for j in i + 1..medios.len() {
                assert_ne!(medios[i], medios[j]);
            }
        }
    }

    #[test]
    fn tiny_wrecks_are_not_drawn_and_nearby_ones_use_full_detail() {
        let r = restos();
        assert!(r.restos[0].malla_lod(0.5).is_none());
        assert!(
            r.restos[0].malla_lod(500.).unwrap().triangulos.len()
                > r.restos[0].malla_lod(5.).unwrap().triangulos.len()
        );
    }

    #[test]
    fn spin_keeps_the_orientation_orthonormal() {
        let mut rot = IDENTIDAD;
        for _ in 0..5000 {
            rotar(&mut rot, Vector3::new(0.3, 0.7, -0.2), 0.016);
        }
        let v = a_mundo(&rot, Vector3::new(1., 2., 3.));
        assert!((v.length() - Vector3::new(1., 2., 3.).length()).abs() < 1e-3);
        let d = (0..3).map(|i| rot[0][i] * rot[1][i]).sum::<f32>();
        assert!(d.abs() < 1e-4);
    }

    #[test]
    fn wrecks_start_apart_from_the_sun_and_drift() {
        let mut r = restos();
        let sistema = SistemaPlanetario::nuevo();
        for resto in &r.restos {
            assert!(resto.centro.length() > sistema.sol.radio + resto.radio_colision);
        }
        let antes = r.restos[1].centro;
        r.actualizar(1., &sistema);
        assert!((r.restos[1].centro - antes).length() > 0.1);
    }

    #[test]
    fn wrecks_bounce_off_the_sun() {
        let mut r = restos();
        let sistema = SistemaPlanetario::nuevo();
        r.restos[0].centro = Vector3::new(sistema.sol.radio, 0., 0.);
        r.restos[0].velocidad = Vector3::new(-5., 0., 0.);
        r.restos[0].deriva = r.restos[0].velocidad;
        r.actualizar(0.016, &sistema);
        assert!(r.restos[0].centro.length() >= sistema.sol.radio + r.restos[0].radio_colision - 1e-2);
        assert!(r.restos[0].velocidad.x > 0.);
    }

    #[test]
    fn fast_ship_cannot_tunnel_through_a_wreck_and_loses_speed() {
        let mut r = restos();
        for resto in &mut r.restos {
            resto.centro = Vector3::new(5e5, 0., 0.);
        }
        r.restos[0].centro = Vector3::zero();
        r.restos[0].velocidad = Vector3::zero();
        r.restos[0].deriva = Vector3::zero();
        let radio_nave = 1.5;

        // Con yaw 0 la nave mira hacia -Z: arranca en +Z y atraviesa el resto en un solo cuadro.
        let mut nave = Nave::new();
        nave.position = Vector3::new(0., 0., 400.);
        nave.set_speed(150_000.);
        assert!(!r.colisionar_nave(&mut nave, radio_nave, false));
        nave.position = Vector3::new(0., 0., -400.);
        assert!(r.colisionar_nave(&mut nave, radio_nave, false));

        let limite = radio_nave + r.restos[0].radio_colision;
        assert!(nave.position.z > 0. && nave.position.length() >= limite - 1e-3);
        assert!(nave.speed_kmh < 150_000.);
        assert!(r.restos[0].velocidad.z < 0., "el resto recibe el golpe hacia adelante de la nave");
    }

    #[test]
    fn landed_ship_pushes_the_wreck_away_instead_of_moving() {
        let mut r = restos();
        for resto in &mut r.restos {
            resto.centro = Vector3::new(5e5, 0., 0.);
        }
        r.restos[0].centro = Vector3::new(0., 0., 3.);
        r.restos[0].velocidad = Vector3::new(0., 0., -2.);
        let mut nave = Nave::new();
        nave.position = Vector3::zero();
        let antes = nave.position;
        assert!(r.colisionar_nave(&mut nave, 1.5, true));
        assert_eq!(nave.position, antes);
        assert!((r.restos[0].centro - nave.position).length() >= 1.5 + r.restos[0].radio_colision);
    }

    #[test]
    fn wrecks_collide_with_each_other() {
        let mut r = restos();
        let sistema = SistemaPlanetario::nuevo();
        let rc = r.restos[0].radio_colision;
        r.restos[0].centro = Vector3::new(30000., 0., 0.);
        r.restos[1].centro = Vector3::new(30000. + rc, 0., 0.);
        r.actualizar(0., &sistema);
        let d = (r.restos[1].centro - r.restos[0].centro).length();
        assert!(d >= rc + r.restos[1].radio_colision - 1e-2);
    }
}
