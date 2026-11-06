
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planeta::SistemaPlanetario;

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
    fn gravity_pulls_idle_ship_and_stronger_for_bigger_planets() {
        let caida = |nombre: &str| {
            let (mut nave, sistema, i) = escena(nombre, 300.);
            let radio = sistema.planetas[i].config.radio;
            nave.position = sistema.planetas[i].centro + Vector3::new(0., radio * 2., 0.);
            let antes = (nave.position - sistema.planetas[i].centro).length();
            let mut gestor = GestorAterrizaje::nuevo();
            for _ in 0..20 {
                gestor.vuelo(&mut nave, &sistema, 13., 0.05);
            }
            antes - (nave.position - sistema.planetas[i].centro).length()
        };
        let (luna, jupiter) = (caida("LUNA"), caida("JUPITER"));
        assert!(luna > 0., "la gravedad debe acercar la nave");
        assert!(jupiter > luna, "un planeta mayor atrae mas");
    }

    #[test]
    fn local_mode_circles_planet_without_diving_into_it() {
        let (mut nave, sistema, i) = escena("TERRA", 300.);
        let mut gestor = GestorAterrizaje::nuevo();
        gestor.alternar_modo_local(&mut nave, &sistema);
        assert_eq!(gestor.titulo(), "MODO LOCAL");
        let p = &sistema.planetas[i];
        let inicio = (nave.position - p.centro).normalized();
        assert!(gestor.vuelo_local(&mut nave, &sistema, 0., INCLINACION_LOCAL, false, 0., 0.05));
        for _ in 0..200 {
            assert!(gestor.vuelo_local(&mut nave, &sistema, 0., 0., true, 0.6, 0.05));
            gestor.vuelo(&mut nave, &sistema, 13., 0.05);
            assert!(!gestor.posada());
            assert!((nave.position - p.centro).length() > p.config.radio);
        }
        let fin = (nave.position - p.centro).normalized();
        assert!(inicio.dot(fin) < 0.95, "la nave debia rodear el planeta");
        gestor.alternar_modo_local(&mut nave, &sistema);
        assert!(!gestor.vuelo_local(&mut nave, &sistema, 0., 0., true, 0.6, 0.05));
    }

    #[test]
    fn local_mode_needs_a_nearby_body() {
        let (mut nave, sistema, _) = escena("PLUTON", 100_000.);
        let mut gestor = GestorAterrizaje::nuevo();
        gestor.alternar_modo_local(&mut nave, &sistema);
        assert!(!gestor.vuelo_local(&mut nave, &sistema, 0., 0., true, 0.6, 0.05));
    }

    #[test]
    fn sun_blocks_the_ship_and_reports_collision() {
        let sistema = SistemaPlanetario::nuevo();
        let mut nave = Nave::new();
        nave.position = sistema.sol.centro + Vector3::new(0., 0., 3000.);
        nave.liberar_base(Vector3::new(0., 0., -1.));
        nave.set_speed(Nave::MAX_SPEED_KMH);
        let mut gestor = GestorAterrizaje::nuevo();
        let mut choco = false;
        for _ in 0..2000 {
            nave.step(0., 0., true, 1., 0.05);
            gestor.vuelo(&mut nave, &sistema, 13., 0.05);
            let d = (nave.position - sistema.sol.centro).length();
            assert!(d >= sistema.sol.radio, "la nave atraveso el sol");
            choco |= gestor.titulo() == "COLISION";
        }
        assert!(choco, "debio registrarse la colision con el sol");
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
