
use crate::planeta::SistemaPlanetario;
use crate::vuelo::{Base, Nave};
use raylib::prelude::Vector3;


const ALTURA_AVISO: f32 = 3.0;
const ALTURA_ARRASTRE: f32 = 2.5;
const ARRASTRE_MINIMO: f32 = 300.;
const VELOCIDAD_ATERRIZAJE: f32 = 800.;
const VELOCIDAD_CONTACTO: f32 = 400.;
const ACEL_ASISTENTE: f32 = 150.;
pub const HOLGURA: f32 = 0.2;
const MIN_DESCENSO: f32 = 0.05;
const AMORTIGUACION_DERIVA: f32 = 0.5;
const INCLINACION_LOCAL: f32 = 1.2;
const HISTERESIS_LOCAL: f32 = 1.5;

/// Distancia sobre la superficie dentro de la cual puede activarse el modo local.
fn alcance_local(radio: f32) -> f32 {
    (ALTURA_ARRASTRE * radio).max(ARRASTRE_MINIMO)
}

/// Modo local: los controles son relativos al cuerpo. A/D giran el rumbo sobre el horizonte
/// local y W/S inclinan la nariz respecto a el, asi que se rodea el planeta sin apuntarle.
#[derive(Clone, Copy)]
struct Orbita {
    planeta: usize,
    rumbo: Vector3, 
    inclinacion: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Estado {
    EnVuelo,
    Aproximando,
    Posada,
    Colision,
}

#[derive(Clone, Copy)]
struct Anclaje {
    planeta: usize,
    normal: Vector3,   
    adelante: Vector3, 
    radio: f32,        
}

pub struct GestorAterrizaje {
    estado: Estado,
    anclaje: Option<Anclaje>,
    aviso_colision: f32,
    esperar_soltar: bool,
    orbita: Option<Orbita>,
}

impl GestorAterrizaje {
    pub fn nuevo() -> Self {
        Self {
            estado: Estado::EnVuelo,
            anclaje: None,
            aviso_colision: 0.,
            esperar_soltar: false,
            orbita: None,
        }
    }

    pub fn posada(&self) -> bool {
        self.anclaje.is_some()
    }

    /// Muestra el aviso de colision durante un segundo (choque contra un resto, por ejemplo).
    pub fn avisar_colision(&mut self) {
        self.aviso_colision = 1.;
        if self.anclaje.is_none() {
            self.estado = Estado::Colision;
        }
    }

    pub fn titulo(&self) -> &'static str {
        match self.estado {
            Estado::EnVuelo | Estado::Aproximando if self.orbita.is_some() => "MODO LOCAL",
            Estado::EnVuelo => "VUELO LIBRE",
            Estado::Aproximando => "APROXIMANDO",
            Estado::Posada => "ATERRIZADO",
            Estado::Colision => "COLISION",
        }
    }

    /// Activa o desactiva el modo local. Solo se activa a menos de `alcance_local` de un cuerpo y nunca estando posada.
    pub fn alternar_modo_local(&mut self, nave: &mut Nave, sistema: &SistemaPlanetario) {
        if self.anclaje.is_some() {
            return;
        }
        if self.orbita.take().is_some() {
            let adelante = nave.forward();
            nave.liberar_base(adelante);
            return;
        }
        let mut mejor: Option<(usize, f32)> = None;
        for (i, p) in sistema.planetas.iter().enumerate() {
            let altura = (nave.position - p.centro).length() - p.config.radio;
            if altura < alcance_local(p.config.radio) && mejor.map_or(true, |(_, a)| altura < a) {
                mejor = Some((i, altura));
            }
        }
        let Some((planeta, _)) = mejor else { return };
        let arriba = (nave.position - sistema.planetas[planeta].centro).normalized();
        let adelante = nave.forward();
        let mut rumbo = adelante - arriba * adelante.dot(arriba);
        if rumbo.length() < 1e-3 {
            let derecha = nave.right();
            rumbo = derecha - arriba * derecha.dot(arriba);
        }
        let orbita = Orbita {
            planeta,
            rumbo: rumbo.normalized(),
            inclinacion: adelante
                .dot(arriba)
                .clamp(-1., 1.)
                .asin()
                .clamp(-INCLINACION_LOCAL, INCLINACION_LOCAL),
        };
        Self::poner_base(nave, arriba, orbita.rumbo, orbita.inclinacion);
        self.orbita = Some(orbita);
    }

    fn poner_base(nave: &mut Nave, arriba: Vector3, rumbo: Vector3, inclinacion: f32) {
        let (s, c) = inclinacion.sin_cos();
        let forward = rumbo * c + arriba * s;
        let right = forward.cross(arriba).normalized();
        nave.base = Some(Base {
            right,
            up: right.cross(forward).normalized(),
            forward,
        });
    }

    pub fn vuelo_local(
        &mut self,
        nave: &mut Nave,
        sistema: &SistemaPlanetario,
        yaw: f32,
        pitch: f32,
        avanzar: bool,
        potencia: f32,
        dt: f32,
    ) -> bool {
        let Some(mut o) = self.orbita else { return false };
        let p = &sistema.planetas[o.planeta];
        let rel = nave.position - p.centro;
        let distancia = rel.length().max(1e-3);
        if distancia - p.config.radio > alcance_local(p.config.radio) * HISTERESIS_LOCAL {
            self.orbita = None;
            let adelante = nave.forward();
            nave.liberar_base(adelante);
            return false;
        }
        let arriba = rel / distancia;
        let mut rumbo = o.rumbo - arriba * o.rumbo.dot(arriba);
        if rumbo.length() < 1e-3 {
            let derecha = nave.right();
            rumbo = derecha - arriba * derecha.dot(arriba);
        }
        let (s, c) = (-yaw).sin_cos();
        let rumbo = rumbo.normalized();
        o.rumbo = (rumbo * c + arriba.cross(rumbo) * s).normalized();
        o.inclinacion = (o.inclinacion + pitch).clamp(-INCLINACION_LOCAL, INCLINACION_LOCAL);
        self.orbita = Some(o);
        Self::poner_base(nave, arriba, o.rumbo, o.inclinacion);
        nave.avanzar_en_base(avanzar, potencia, dt, Nave::LASER_THRESHOLD_KMH);
        true
    }

    pub fn vuelo(&mut self, nave: &mut Nave, sistema: &SistemaPlanetario, size: f32, dt: f32) {
        self.aviso_colision = (self.aviso_colision - dt).max(0.);
        self.estado = if self.aviso_colision > 0. {
            Estado::Colision
        } else {
            Estado::EnVuelo
        };
        let holgura = size * HOLGURA;
        let rel_sol = nave.position - sistema.sol.centro;
        let limite_sol = sistema.sol.radio + holgura;
        let distancia_sol = rel_sol.length();
        if distancia_sol < limite_sol {
            nave.position = sistema.sol.centro + rel_sol / distancia_sol.max(1e-3) * limite_sol;
            nave.deriva = Vector3::zero();
            nave.set_speed(0.);
            self.aviso_colision = 2.;
            self.estado = Estado::Colision;
        }
        let mut aproximando = false;
        self.aplicar_gravedad(nave, sistema, dt);
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

            if altitud < ALTURA_AVISO * radio {
                aproximando = true;
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
                nave.deriva = Vector3::zero();
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
        if self.aviso_colision <= 0. && aproximando {
            self.estado = Estado::Aproximando;
        }
    }

    fn aplicar_gravedad(&self, nave: &mut Nave, sistema: &SistemaPlanetario, dt: f32) {
        let mut aceleracion = Vector3::zero();
        for planeta in &sistema.planetas {
            aceleracion += planeta.aceleracion_gravedad(nave.position);
        }
        nave.deriva += aceleracion * dt;
        nave.deriva *= (-AMORTIGUACION_DERIVA * dt).exp();
        nave.position += nave.deriva * dt;
    }

    fn posar(
        &mut self,
        nave: &mut Nave,
        planeta: usize,
        sistema: &SistemaPlanetario,
        normal_local: Vector3,
        radio: f32,
    ) {
        self.orbita = None;
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
        self.estado = Estado::Posada;
    }

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
        let (s, c) = (-yaw).sin_cos();
        a.adelante = (a.adelante * c + a.normal.cross(a.adelante) * s).normalized();
        self.anclaje = Some(a);
        self.aplicar(nave, sistema);
    }
}
