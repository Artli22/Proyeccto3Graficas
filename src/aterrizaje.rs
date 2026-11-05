//! Aproximacion asistida, colision y aterrizaje sobre planetas procedurales.
//! Mientras la nave esta posada su posicion se guarda en el marco local del planeta,
//! asi que gira y orbita junto con el.
use crate::planeta::SistemaPlanetario;
use crate::vuelo::{Base, Nave};
use raylib::prelude::Vector3;

/// Se muestra el aviso de aproximacion a esta altura sobre la superficie, en radios del cuerpo.
const ALTURA_AVISO: f32 = 3.0;
/// El cuerpo arrastra a la nave (marco de referencia comovil) por debajo de esta altura:
/// 2.5 radios, con un minimo en unidades para que los cuerpos pequenos tambien la tengan.
const ALTURA_ARRASTRE: f32 = 2.5;
const ARRASTRE_MINIMO: f32 = 300.;
/// Velocidad maxima permitida al tocar el suelo (km/h).
const VELOCIDAD_ATERRIZAJE: f32 = 800.;
/// Velocidad objetivo al contacto (km/h).
const VELOCIDAD_CONTACTO: f32 = 400.;
/// Frenado constante (unidades/s^2) que el asistente permite al descender: define la
/// velocidad maxima segun la altura, v = sqrt(2 a h), y llega a VELOCIDAD_CONTACTO en el suelo.
const ACEL_ASISTENTE: f32 = 150.;
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
        let mut aviso: Option<(usize, f32, f32)> = None;
        for (i, planeta) in sistema.planetas.iter().enumerate() {
            let radio = planeta.config.radio;
            let altura = (nave.position - planeta.centro).length() - radio;
            let t = (1. - altura / (ALTURA_ARRASTRE * radio).max(ARRASTRE_MINIMO)).clamp(0., 1.);
            nave.position += planeta.delta * (t * t * (3. - 2. * t));

            let rel = nave.position - planeta.centro;
            let distancia = rel.length().max(1e-3);
            let dir = rel / distancia;
            let dir_local = planeta.a_local(dir);
            let contacto = planeta.radio_superficie(dir_local) + holgura;
            let altitud = distancia - contacto;
            let descenso = -nave.forward().dot(dir);

            if altitud < ALTURA_AVISO * radio && aviso.map_or(true, |(_, a, _)| altitud < a) {
                aviso = Some((i, altitud, (1. - altitud / (ALTURA_AVISO * radio)).clamp(0., 1.)));
            }
            if descenso > MIN_DESCENSO && nave.speed_kmh > VELOCIDAD_CONTACTO {
                let metros_a_kmh = Nave::METERS_PER_UNIT * 3.6;
                let tope = VELOCIDAD_CONTACTO
                    + (2. * ACEL_ASISTENTE * altitud.max(0.)).sqrt() * metros_a_kmh;
                if nave.speed_kmh > tope {
                    nave.set_speed(tope);
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
        if self.aviso_colision <= 0. {
            if let Some((planeta, altitud, progreso)) = aviso {
                self.estado = Estado::Aproximando {
                    planeta,
                    altitud,
                    progreso,
                };
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

    // Nave sobre el cuerpo `nombre`, a `altura` unidades de su centro y apuntando a el.
    fn escena(nombre: &str, altura: f32) -> (Nave, SistemaPlanetario, usize) {
        let sistema = SistemaPlanetario::nuevo();
        let i = sistema.indice(nombre).unwrap();
        let mut nave = Nave::new();
        nave.position = sistema.planetas[i].centro + Vector3::new(0., altura, 0.);
        nave.liberar_base(Vector3::new(0., -1., 0.));
        (nave, sistema, i)
    }

    fn descender(
        nave: &mut Nave,
        sistema: &mut SistemaPlanetario,
        gestor: &mut GestorAterrizaje,
        apuntar_a: Option<usize>,
    ) {
        for _ in 0..6000 {
            if let Some(i) = apuntar_a {
                // Cuerpo en movimiento: el piloto corrige el rumbo hacia su posicion actual.
                sistema.actualizar(0.05);
                let hacia = sistema.planetas[i].centro - nave.position;
                nave.liberar_base(hacia.normalized());
            }
            nave.step(0., 0., true, 0.6, 0.05);
            gestor.vuelo(nave, sistema, 13., 0.05);
            if gestor.posada() {
                break;
            }
        }
    }

    #[test]
    fn slow_descent_lands_and_follows_planet() {
        let (mut nave, mut sistema, i) = escena("TERRA", 300.);
        let mut gestor = GestorAterrizaje::nuevo();
        descender(&mut nave, &mut sistema, &mut gestor, None);
        assert!(gestor.posada(), "la nave debia aterrizar");
        let antes = nave.position - sistema.planetas[i].centro;
        for _ in 0..200 {
            sistema.actualizar(0.05);
            gestor.superficie(&mut nave, &sistema, 0., false);
        }
        let despues = nave.position - sistema.planetas[i].centro;
        assert!((antes.length() - despues.length()).abs() < 1e-3);
        assert!((nave.up().dot(despues.normalized()) - 1.).abs() < 1e-3);
    }

    #[test]
    fn lands_on_orbiting_moon() {
        let (mut nave, mut sistema, i) = escena("LUNA", 600.);
        nave.set_speed(40_000.);
        let mut gestor = GestorAterrizaje::nuevo();
        descender(&mut nave, &mut sistema, &mut gestor, Some(i));
        assert!(gestor.posada(), "la nave debia aterrizar en la luna");
        let p = &sistema.planetas[i];
        assert!((nave.position - p.centro).length() < p.config.radio * 1.2);
    }

    #[test]
    fn takeoff_requires_release_after_landing() {
        let (mut nave, mut sistema, _) = escena("TERRA", 300.);
        let mut gestor = GestorAterrizaje::nuevo();
        descender(&mut nave, &mut sistema, &mut gestor, None);
        gestor.superficie(&mut nave, &sistema, 0., true);
        assert!(gestor.posada());
        gestor.superficie(&mut nave, &sistema, 0., false);
        gestor.superficie(&mut nave, &sistema, 0., true);
        assert!(!gestor.posada());
        assert!(nave.base.is_none());
    }

    #[test]
    fn never_ends_inside_body_at_full_speed() {
        for nombre in ["TERRA", "PLUTON", "MERCURIO"] {
            let (mut nave, sistema, i) = escena(nombre, 5000.);
            nave.set_speed(Nave::MAX_SPEED_KMH);
            let mut gestor = GestorAterrizaje::nuevo();
            for _ in 0..4000 {
                nave.step(0., 0., true, 1., 0.05);
                gestor.vuelo(&mut nave, &sistema, 13., 0.05);
                let p = &sistema.planetas[i];
                let dir = (nave.position - p.centro).normalized();
                let superficie = p.radio_superficie(p.a_local(dir));
                assert!(
                    (nave.position - p.centro).length() >= superficie,
                    "la nave atraveso {nombre}"
                );
            }
        }
    }
}
