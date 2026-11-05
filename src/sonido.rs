
use raylib::prelude::*;

const SUBIDA: f32 = 6.; 
const BAJADA: f32 = 4.; 
const RETRASO_FRENADO: f32 = 0.5;
const VOLUMEN_FRENADO: f32 = 0.8;

#[derive(Clone, Copy, PartialEq, Debug)]
enum AccionFrenado {
    Nada,
    Reproducir,
    Cancelar,
}

// El frenado se dispara una sola vez tras RETRASO_FRENADO segundos sin avanzar y solo si la nave
// aun se mueve; si se vuelve a avanzar se cancela.
#[derive(Default)]
struct TemporizadorFrenado {
    sin_avanzar: f32,
    disparado: bool,
}

impl TemporizadorFrenado {
    fn paso(&mut self, dt: f32, activo: bool, velocidad_kmh: f32) -> AccionFrenado {
        if activo {
            self.sin_avanzar = 0.;
            if std::mem::take(&mut self.disparado) {
                return AccionFrenado::Cancelar;
            }
            return AccionFrenado::Nada;
        }
        self.sin_avanzar += dt;
        if !self.disparado && self.sin_avanzar >= RETRASO_FRENADO && velocidad_kmh > 0. {
            self.disparado = true;
            return AccionFrenado::Reproducir;
        }
        AccionFrenado::Nada
    }
}

fn objetivos(activo: bool, laser: bool, potencia: f32) -> (f32, f32) {
    if !activo {
        return (0., 0.);
    }
    let volumen = 0.35 + 0.65 * potencia.clamp(0., 1.);
    if laser {
        (0., volumen)
    } else {
        (volumen, 0.)
    }
}

fn acercar(actual: f32, objetivo: f32, dt: f32) -> f32 {
    if objetivo > actual {
        (actual + SUBIDA * dt).min(objetivo)
    } else {
        (actual - BAJADA * dt).max(objetivo)
    }
}

struct Pista<'a> {
    musica: Music<'a>,
    volumen: f32,
}

impl<'a> Pista<'a> {
    fn cargar(audio: &'a RaylibAudio, ruta: &str) -> Option<Self> {
        match audio.new_music(ruta) {
            Ok(musica) => Some(Self {
                musica,
                volumen: 0.,
            }),
            Err(e) => {
                eprintln!("No se pudo cargar el sonido {ruta}: {e}");
                None
            }
        }
    }

    fn actualizar(&mut self, objetivo: f32, dt: f32) {
        self.volumen = acercar(self.volumen, objetivo, dt);
        if self.volumen > 0. {
            if !self.musica.is_stream_playing() {
                self.musica.play_stream();
            }
            self.musica.set_volume(self.volumen);
            self.musica.update_stream();
        } else if self.musica.is_stream_playing() {
            self.musica.stop_stream();
        }
    }
}

pub struct SonidoNave<'a> {
    normal: Pista<'a>,
    laser: Pista<'a>,
    frenado: Sound<'a>,
    temporizador: TemporizadorFrenado,
    volumen_frenado: f32,
    desvaneciendo: bool,
}

impl<'a> SonidoNave<'a> {
    pub fn cargar(audio: &'a RaylibAudio) -> Option<Self> {
        let carpeta = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/soundeffects/");
        let ruta_frenado = format!("{carpeta}bajandoVelocidad.mp3");
        let frenado = match audio.new_sound(&ruta_frenado) {
            Ok(sonido) => sonido,
            Err(e) => {
                eprintln!("No se pudo cargar el sonido {ruta_frenado}: {e}");
                return None;
            }
        };
        Some(Self {
            normal: Pista::cargar(audio, &format!("{carpeta}PropulsionNormal.mp3"))?,
            laser: Pista::cargar(audio, &format!("{carpeta}propulsionLazer.mp3"))?,
            frenado,
            temporizador: TemporizadorFrenado::default(),
            volumen_frenado: 0.,
            desvaneciendo: false,
        })
    }

    pub fn actualizar(
        &mut self,
        dt: f32,
        activo: bool,
        laser: bool,
        potencia: f32,
        velocidad_kmh: f32,
    ) {
        let (normal, laser) = objetivos(activo, laser, potencia);
        self.normal.actualizar(normal, dt);
        self.laser.actualizar(laser, dt);

        match self.temporizador.paso(dt, activo, velocidad_kmh) {
            AccionFrenado::Reproducir => {
                self.desvaneciendo = false;
                self.volumen_frenado = VOLUMEN_FRENADO;
                self.frenado.set_volume(self.volumen_frenado);
                self.frenado.play();
            }
            AccionFrenado::Cancelar => self.desvaneciendo = true,
            AccionFrenado::Nada => {}
        }
        if self.desvaneciendo {
            self.volumen_frenado = (self.volumen_frenado - BAJADA * dt).max(0.);
            self.frenado.set_volume(self.volumen_frenado);
            if self.volumen_frenado <= 0. {
                self.frenado.stop();
                self.desvaneciendo = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_active_engine_has_volume() {
        assert_eq!(objetivos(false, false, 1.), (0., 0.));
        assert_eq!(objetivos(false, true, 1.), (0., 0.));
        let (n, l) = objetivos(true, false, 0.6);
        assert!(n > 0. && l == 0.);
        let (n, l) = objetivos(true, true, 0.6);
        assert!(n == 0. && l > 0.);
    }

    #[test]
    fn power_raises_volume_within_range() {
        let v = |p| objetivos(true, false, p).0;
        assert!(v(0.3) < v(0.6) && v(0.6) < v(1.));
        assert!(v(1.) <= 1.);
    }

    #[test]
    fn braking_sound_fires_once_after_delay_without_thrust() {
        let mut t = TemporizadorFrenado::default();
        assert_eq!(t.paso(0.016, true, 1000.), AccionFrenado::Nada);
        let mut disparos = 0;
        for _ in 0..400 {
            if t.paso(0.016, false, 1000.) == AccionFrenado::Reproducir {
                disparos += 1;
                assert!(t.sin_avanzar >= RETRASO_FRENADO);
            }
        }
        assert_eq!(disparos, 1);
    }

    #[test]
    fn braking_sound_requires_motion_and_cancels_on_thrust() {
        let mut quieta = TemporizadorFrenado::default();
        for _ in 0..400 {
            assert_eq!(quieta.paso(0.016, false, 0.), AccionFrenado::Nada);
        }
        let mut t = TemporizadorFrenado::default();
        for _ in 0..250 {
            t.paso(0.016, false, 500.);
        }
        assert!(t.disparado);
        assert_eq!(t.paso(0.016, true, 500.), AccionFrenado::Cancelar);
        assert_eq!(t.paso(0.016, true, 500.), AccionFrenado::Nada);
        assert_eq!(t.sin_avanzar, 0.);
    }

    #[test]
    fn fade_reaches_target_without_overshoot() {
        let mut v = 0.;
        for _ in 0..100 {
            v = acercar(v, 0.8, 0.016);
            assert!(v <= 0.8);
        }
        assert_eq!(v, 0.8);
        for _ in 0..200 {
            v = acercar(v, 0., 0.016);
            assert!(v >= 0.);
        }
        assert_eq!(v, 0.);
    }
}
