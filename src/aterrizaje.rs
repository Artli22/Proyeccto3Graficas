//! Aproximacion asistida, colision y aterrizaje sobre planetas procedurales.
//! Mientras la nave esta posada su posicion se guarda en el marco local del planeta,
//! asi que gira y orbita junto con el.
use crate::planeta::SistemaPlanetario;
use crate::vuelo::{Base, Nave};
use raylib::prelude::Vector3;

/// La asistencia empieza a esta altura sobre la superficie, en radios del planeta.
const ALTURA_ASISTENTE: f32 = 3.0;
/// El planeta arrastra a la nave (marco de referencia comovil) por debajo de esta altura.
const ALTURA_ARRASTRE: f32 = 2.5;
/// Velocidad maxima permitida al tocar el suelo (km/h).
const VELOCIDAD_ATERRIZAJE: f32 = 800.;
/// Tope de velocidad al contacto y pendiente del tope por unidad de altura (km/h).
const VELOCIDAD_CONTACTO: f32 = 400.;
const KMH_POR_UNIDAD: f32 = 72.;
/// Distancia centro-nave a superficie estando posada, relativa al tamano de la nave.
const HOLGURA: f32 = 0.2;
/// Componente minima de avance hacia el suelo para considerar un aterrizaje.
const MIN_DESCENSO: f32 = 0.05;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Estado {
    EnVuelo,
    Aproximando {
        planeta: usize,
        altitud: f32,
        progreso: f32,
    },
    Posada {
        planeta: usize,
    },
    Colision,
}

#[derive(Clone, Copy)]
struct Anclaje {
    planeta: usize,
    normal: Vector3,   // local al planeta
    adelante: Vector3, // local al planeta, perpendicular a `normal`
    radio: f32,        // distancia del centro del planeta al centro de la nave
}

pub struct GestorAterrizaje {
    estado: Estado,
    anclaje: Option<Anclaje>,
    aviso_colision: f32,
    esperar_soltar: bool,
}

impl GestorAterrizaje {
    pub fn nuevo() -> Self {
        Self {
            estado: Estado::EnVuelo,
            anclaje: None,
            aviso_colision: 0.,
            esperar_soltar: false,
        }
    }

    pub fn posada(&self) -> bool {
        self.anclaje.is_some()
    }

    /// Texto y progreso (0..1) para el HUD.
    pub fn hud(&self, sistema: &SistemaPlanetario) -> (String, f32) {
        match self.estado {
            Estado::EnVuelo => (String::new(), 0.),
            Estado::Aproximando {
                planeta,
                altitud,
                progreso,
            } => (
                format!(
                    "{} ALTITUD {} M",
                    sistema.planetas[planeta].config.nombre,
                    (altitud.max(0.) * Nave::METERS_PER_UNIT) as u32
                ),
                progreso,
            ),
            Estado::Posada { planeta } => (
                format!(
                    "ATERRIZADO EN {} - ESPACIO: DESPEGAR",
                    sistema.planetas[planeta].config.nombre
                ),
                1.,
            ),
            Estado::Colision => ("COLISION - VELOCIDAD EXCESIVA".to_string(), 0.),
        }
    }

    /// Vuelo libre: arrastre del planeta, tope de velocidad al descender, colision y aterrizaje.
    /// Se llama despues de `Nave::step`.
    pub fn vuelo(&mut self, nave: &mut Nave, sistema: &SistemaPlanetario, size: f32, dt: f32) {
        self.aviso_colision = (self.aviso_colision - dt).max(0.);
        self.estado = if self.aviso_colision > 0. {
            Estado::Colision
        } else {
            Estado::EnVuelo
        };
        let holgura = size * HOLGURA;
        for (i, planeta) in sistema.planetas.iter().enumerate() {
            let radio = planeta.config.radio;
            let altura = (nave.position - planeta.centro).length() - radio;
            let t = (1. - altura / (ALTURA_ARRASTRE * radio)).clamp(0., 1.);
            nave.position += planeta.delta * (t * t * (3. - 2. * t));

            let rel = nave.position - planeta.centro;
            let distancia = rel.length().max(1e-3);
            let dir = rel / distancia;
            let dir_local = planeta.a_local(dir);
            let contacto = planeta.radio_superficie(dir_local) + holgura;
            let altitud = distancia - contacto;
            let descenso = -nave.forward().dot(dir);

            if altitud < ALTURA_ASISTENTE * radio {
                if self.aviso_colision <= 0. {
                    self.estado = Estado::Aproximando {
                        planeta: i,
                        altitud,
                        progreso: (1. - altitud / (ALTURA_ASISTENTE * radio)).clamp(0., 1.),
                    };
                }
                if descenso > MIN_DESCENSO && nave.speed_kmh > 0. {
                    let tope = VELOCIDAD_CONTACTO + altitud.max(0.) * KMH_POR_UNIDAD;
                    if nave.speed_kmh > tope {
                        nave.set_speed(tope);
                    }
                }
            }

            if distancia <= contacto {
                nave.position = planeta.centro + dir * contacto;
                if descenso > MIN_DESCENSO {
                    if nave.speed_kmh <= VELOCIDAD_ATERRIZAJE {
                        self.posar(nave, i, sistema, dir_local, contacto);
                    } else {
                        nave.set_speed(0.);
                        self.aviso_colision = 2.;
                        self.estado = Estado::Colision;
                    }
                }
                return;
            }
        }
    }

    fn posar(
        &mut self,
        nave: &mut Nave,
        planeta: usize,
        sistema: &SistemaPlanetario,
        normal_local: Vector3,
        radio: f32,
    ) {
        let p = &sistema.planetas[planeta];
        let normal = p.a_mundo(normal_local);
        let forward = nave.forward();
        let mut tangente = forward - normal * forward.dot(normal);
        if tangente.length() < 1e-3 {
            tangente = nave.right();
        }
        self.anclaje = Some(Anclaje {
            planeta,
            normal: normal_local,
            adelante: p.a_local(tangente.normalized()),
            radio,
        });
        self.esperar_soltar = true;
        nave.set_speed(0.);
        self.aplicar(nave, sistema);
    }

    fn aplicar(&mut self, nave: &mut Nave, sistema: &SistemaPlanetario) {
        let Some(a) = self.anclaje else { return };
        let p = &sistema.planetas[a.planeta];
        let up = p.a_mundo(a.normal);
        let forward = p.a_mundo(a.adelante);
        nave.base = Some(Base {
            right: forward.cross(up).normalized(),
            up,
            forward,
        });
        nave.position = p.centro + up * a.radio;
        nave.speed_kmh = 0.;
        nave.velocity = Vector3::zero();
        self.estado = Estado::Posada { planeta: a.planeta };
    }

    /// Nave posada: A/D giran sobre el terreno y ESPACIO despega. No se llama a `Nave::step`.
    pub fn superficie(
        &mut self,
        nave: &mut Nave,
        sistema: &SistemaPlanetario,
        yaw: f32,
        despegar: bool,
    ) {
        let Some(mut a) = self.anclaje else { return };
        if !despegar {
            self.esperar_soltar = false;
        }
        if despegar && !self.esperar_soltar {
            let p = &sistema.planetas[a.planeta];
            let rumbo = (p.a_mundo(a.normal) * 0.9 + p.a_mundo(a.adelante) * 0.3).normalized();
            nave.liberar_base(rumbo);
            nave.set_speed(0.);
            self.anclaje = None;
            self.estado = Estado::EnVuelo;
            return;
        }
        // Un giro a la derecha es horario visto desde la normal, es decir angulo negativo.
        let (s, c) = (-yaw).sin_cos();
        a.adelante = (a.adelante * c + a.normal.cross(a.adelante) * s).normalized();
        self.anclaje = Some(a);
        self.aplicar(nave, sistema);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planeta::SistemaPlanetario;

    fn escena() -> (Nave, SistemaPlanetario) {
        let sistema = SistemaPlanetario::nuevo();
        let mut nave = Nave::new();
        let p = &sistema.planetas[0];
        nave.position = p.centro + Vector3::new(0., p.config.radio * 2., 0.);
        nave.pitch = -1.3;
        (nave, sistema)
    }

    #[test]
    fn slow_descent_lands_and_follows_planet() {
        let (mut nave, mut sistema) = escena();
        let mut gestor = GestorAterrizaje::nuevo();
        for _ in 0..4000 {
            nave.step(0., 0., true, 0.6, 0.05);
            gestor.vuelo(&mut nave, &sistema, 13., 0.05);
            if gestor.posada() {
                break;
            }
        }
        assert!(gestor.posada(), "la nave debia aterrizar");
        let antes = nave.position - sistema.planetas[0].centro;
        for _ in 0..200 {
            sistema.actualizar(0.05);
            gestor.superficie(&mut nave, &sistema, 0., false);
        }
        let despues = nave.position - sistema.planetas[0].centro;
        assert!((antes.length() - despues.length()).abs() < 1e-3);
        assert!((nave.up().dot(despues.normalized()) - 1.).abs() < 1e-3);
    }

    #[test]
    fn takeoff_requires_release_after_landing() {
        let (mut nave, sistema) = escena();
        let mut gestor = GestorAterrizaje::nuevo();
        for _ in 0..4000 {
            nave.step(0., 0., true, 0.6, 0.05);
            gestor.vuelo(&mut nave, &sistema, 13., 0.05);
            if gestor.posada() {
                break;
            }
        }
        gestor.superficie(&mut nave, &sistema, 0., true);
        assert!(gestor.posada());
        gestor.superficie(&mut nave, &sistema, 0., false);
        gestor.superficie(&mut nave, &sistema, 0., true);
        assert!(!gestor.posada());
        assert!(nave.base.is_none());
    }

    #[test]
    fn never_ends_inside_planet_at_full_speed() {
        let (mut nave, sistema) = escena();
        nave.position = sistema.planetas[0].centro + Vector3::new(0., 1000., 0.);
        nave.speed_kmh = 28_000.;
        let mut gestor = GestorAterrizaje::nuevo();
        for _ in 0..4000 {
            nave.step(0., 0., true, 1., 0.05);
            gestor.vuelo(&mut nave, &sistema, 13., 0.05);
            let p = &sistema.planetas[0];
            let d = (nave.position - p.centro).length();
            assert!(d >= p.config.radio, "la nave atraveso el planeta");
        }
    }
}
