use crate::planeta::SistemaPlanetario;
use raylib::prelude::*;
use std::f32::consts::{PI, TAU};

const NUM_PARTICULAS: usize = 2600;
const RADIO_TUBO: f32 = 110.;
const RADIO_PARTICULA: f32 = 14.;
const AMPLITUD_ONDA: f32 = 260.;
const ALTURA_CONTROL: f32 = 2200.;
const VELOCIDAD_FLUJO: f32 = 0.05;

/// Punto listo para dibujar con `particle_renderer` (mismo formato que las particulas del propulsor).
pub struct PuntoLinea {
    pub position: Vector3,
    pub radius: f32,
    pub color: Color,
}

struct Particula {
    t: f32,
    angulo: f32,
    radial: f32,
    velocidad: f32,
    tam: f32,
    tono: f32,
}

/// Tubo de puntos rojos brillantes entre la parte inferior de JUPITER y la superior del sol.
/// Las posiciones se recalculan cada cuadro a partir de los extremos, asi que el tubo sigue a
/// Jupiter mientras orbita.
pub struct LineaPetrova {
    particulas: Vec<Particula>,
    puntos: Vec<PuntoLinea>,
    indice_jupiter: Option<usize>,
    tiempo: f32,
}

fn rand(estado: &mut u32) -> f32 {
    let mut x = *estado;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *estado = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn arriba() -> Vector3 {
    Vector3::new(0., 1., 0.)
}

/// Paleta de rojos: de corinto (vino oscuro) a carmesi y rojo vivo; el brillo del pulso
/// acerca cada tono a un rojo incandescente.
fn tono_rojo(tono: f32, brillo: f32) -> [u8; 3] {
    const CORINTO: [f32; 3] = [190., 12., 50.];
    const CARMESI: [f32; 3] = [235., 20., 60.];
    const VIVO: [f32; 3] = [255., 35., 25.];
    const INCANDESCENTE: [f32; 3] = [255., 110., 95.];
    let mezcla = |a: [f32; 3], b: [f32; 3], t: f32| std::array::from_fn::<f32, 3, _>(|i| a[i] + (b[i] - a[i]) * t);
    let base = if tono < 0.5 {
        mezcla(CORINTO, CARMESI, tono * 2.)
    } else {
        mezcla(CARMESI, VIVO, (tono - 0.5) * 2.)
    };
    mezcla(base, INCANDESCENTE, brillo * 0.8).map(|c| c.clamp(0., 255.) as u8)
}

/// (inicio, fin): parte inferior de Jupiter y parte superior del sol.
fn extremos(sistema: &SistemaPlanetario, jupiter: usize) -> (Vector3, Vector3) {
    let j = &sistema.planetas[jupiter];
    (
        j.centro - arriba() * j.config.radio,
        sistema.sol.centro + arriba() * sistema.sol.radio,
    )
}

/// Bezier cubica con forma de "S": sale de Jupiter hacia abajo y cae sobre el sol desde arriba.
fn curva(inicio: Vector3, fin: Vector3, t: f32) -> (Vector3, Vector3) {
    let p1 = inicio - arriba() * ALTURA_CONTROL;
    let p2 = fin + arriba() * ALTURA_CONTROL;
    let omt = 1. - t;
    let pos = inicio * (omt * omt * omt)
        + p1 * (3. * omt * omt * t)
        + p2 * (3. * omt * t * t)
        + fin * (t * t * t);
    let tangente = (p1 - inicio) * (3. * omt * omt)
        + (p2 - p1) * (6. * omt * t)
        + (fin - p2) * (3. * t * t);
    (pos, tangente.normalized())
}

impl LineaPetrova {
    pub fn nueva(sistema: &SistemaPlanetario) -> Self {
        let mut estado = 0x1B87_3593_u32;
        let particulas = (0..NUM_PARTICULAS)
            .map(|_| Particula {
                t: rand(&mut estado),
                angulo: rand(&mut estado) * TAU,
                radial: if rand(&mut estado) < 0.8 {
                    0.8 + 0.2 * rand(&mut estado)
                } else {
                    rand(&mut estado)
                },
                velocidad: VELOCIDAD_FLUJO * (0.8 + 0.4 * rand(&mut estado)),
                tam: 0.7 + rand(&mut estado) * 0.6,
                tono: rand(&mut estado),
            })
            .collect();
        LineaPetrova {
            particulas,
            puntos: Vec::with_capacity(NUM_PARTICULAS),
            indice_jupiter: sistema.planetas.iter().position(|p| p.config.nombre == "JUPITER"),
            tiempo: 0.,
        }
    }

    pub fn puntos(&self) -> &[PuntoLinea] {
        &self.puntos
    }

    pub fn apagar(&mut self) {
        self.puntos.clear();
    }

    /// Debe llamarse despues de `sistema.actualizar` para usar la posicion actual de Jupiter.
    pub fn actualizar(&mut self, dt: f32, sistema: &SistemaPlanetario) {
        self.puntos.clear();
        let Some(jupiter) = self.indice_jupiter else {
            return;
        };
        self.tiempo += dt;
        let tiempo = self.tiempo;
        let (inicio, fin) = extremos(sistema, jupiter);

        // El tubo vive en el plano vertical que contiene a Jupiter, el sol y la vertical:
        // `lado` es la normal de ese plano y nunca es paralela a la tangente.
        let mut horizontal = fin - inicio;
        horizontal.y = 0.;
        let horizontal = if horizontal.length() < 1e-3 {
            Vector3::new(1., 0., 0.)
        } else {
            horizontal.normalized()
        };
        let lado = arriba().cross(horizontal).normalized();

        for p in &mut self.particulas {
            p.t = (p.t - p.velocidad * dt).rem_euclid(1.);
            let t = p.t;
            let (centro, tangente) = curva(inicio, fin, t);
            let normal = tangente.cross(lado).normalized();

            // Ondulaciones del eje (fijas en los extremos) y estrechamiento del tubo.
            let envolvente = (PI * t).sin();
            let onda = lado * (AMPLITUD_ONDA * envolvente * (t * TAU * 2.5 + tiempo * 0.8).sin())
                + normal * (AMPLITUD_ONDA * 0.5 * envolvente * (t * TAU * 4. + tiempo * 0.6 + 1.3).sin());
            let radio_tubo = RADIO_TUBO
                * (0.35 + 0.65 * envolvente.powf(0.6))
                * (1. + 0.25 * (t * 18. + tiempo * 2.).sin());

            let angulo = p.angulo + t * TAU * 3. + tiempo * 0.7;
            let lateral = lado * angulo.cos() + normal * angulo.sin();
            let position = centro + onda + lateral * (radio_tubo * p.radial);

            // Pulsos brillantes que viajan del sol hacia Jupiter.
            let pulso = 0.5 + 0.5 * (t * 60. + tiempo * 5.).sin();
            let brillo = pulso * pulso;
            let entrada = ((1. - t) * 30.).min(1.);
            let alfa = (170. + 80. * brillo) * entrada;
            let [r, g, b] = tono_rojo(p.tono, brillo);
            self.puntos.push(PuntoLinea {
                position,
                radius: RADIO_PARTICULA * p.tam,
                color: Color::new(r, g, b, alfa.clamp(0., 255.) as u8),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_curva_une_jupiter_con_la_parte_superior_del_sol() {
        let sistema = SistemaPlanetario::nuevo();
        let j = sistema.planetas.iter().position(|p| p.config.nombre == "JUPITER").unwrap();
        let (inicio, fin) = extremos(&sistema, j);
        assert!((curva(inicio, fin, 0.).0 - inicio).length() < 1e-2);
        assert!((curva(inicio, fin, 1.).0 - fin).length() < 1e-2);
        assert!(inicio.y < sistema.planetas[j].centro.y);
        assert!(fin.y > sistema.sol.centro.y);
    }

    #[test]
    fn el_tubo_sigue_a_jupiter_en_su_orbita() {
        let mut sistema = SistemaPlanetario::nuevo();
        let mut linea = LineaPetrova::nueva(&sistema);
        sistema.actualizar(0.01);
        linea.actualizar(0.01, &sistema);
        assert_eq!(linea.puntos().len(), NUM_PARTICULAS);
        let j = sistema.planetas.iter().position(|p| p.config.nombre == "JUPITER").unwrap();
        let antes = sistema.planetas[j].centro;
        let media = |l: &LineaPetrova| {
            l.puntos().iter().fold(Vector3::zero(), |a, p| a + p.position) / NUM_PARTICULAS as f32
        };
        let m0 = media(&linea);
        for _ in 0..200 {
            sistema.actualizar(0.05);
        }
        linea.actualizar(0.05, &sistema);
        let desplazamiento_jupiter = sistema.planetas[j].centro - antes;
        assert!(desplazamiento_jupiter.length() > 1.);
        assert!((media(&linea) - m0).length() > 0.1 * desplazamiento_jupiter.length());
    }
}
