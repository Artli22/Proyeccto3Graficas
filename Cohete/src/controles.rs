use raylib::prelude::*;

const GAMEPAD: i32 = 0;
const ZONA_MUERTA: f32 = 0.15;
const VELOCIDAD_ROTACION: f32 = 0.03;
const SENSIBILIDAD_MOUSE: f32 = 0.005;
const UMBRAL_CLIC: f32 = 4.0;

#[derive(Default)]
pub struct Entrada {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub reiniciar: bool,
    pub clic: Option<Vector2>,
    pub quitar_seleccion: bool,
}

pub struct Controles {
    gamepad_conectado: bool,
    inicio_clic: Option<Vector2>,
}

impl Controles {
    pub fn new() -> Self {
        Controles { gamepad_conectado: false, inicio_clic: None }
    }

    pub fn leer(&mut self, rl: &RaylibHandle) -> Entrada {
        let mut entrada = Entrada::default();
        self.leer_teclado(rl, &mut entrada);
        self.leer_mouse(rl, &mut entrada);
        self.leer_gamepad(rl, &mut entrada);
        entrada
    }

    fn leer_teclado(&self, rl: &RaylibHandle, entrada: &mut Entrada) {
        let eje = |menos: &[KeyboardKey], mas: &[KeyboardKey]| -> f32 {
            let activo = |teclas: &[KeyboardKey]| teclas.iter().any(|&k| rl.is_key_down(k));
            (activo(mas) as i32 - activo(menos) as i32) as f32
        };

        entrada.yaw += eje(
            &[KeyboardKey::KEY_LEFT, KeyboardKey::KEY_A],
            &[KeyboardKey::KEY_RIGHT, KeyboardKey::KEY_D],
        ) * VELOCIDAD_ROTACION;
        entrada.pitch += eje(
            &[KeyboardKey::KEY_DOWN, KeyboardKey::KEY_S],
            &[KeyboardKey::KEY_UP, KeyboardKey::KEY_W],
        ) * VELOCIDAD_ROTACION;
        entrada.zoom += eje(&[KeyboardKey::KEY_MINUS, KeyboardKey::KEY_Q], &[KeyboardKey::KEY_EQUAL]) * 0.2;

        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            entrada.reiniciar = true;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_C) {
            entrada.quitar_seleccion = true;
        }
    }

    fn leer_mouse(&mut self, rl: &RaylibHandle, entrada: &mut Entrada) {
        entrada.zoom += rl.get_mouse_wheel_move();

        if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
            self.inicio_clic = Some(rl.get_mouse_position());
        }
        if rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT) {
            let posicion = rl.get_mouse_position();
            if let Some(inicio) = self.inicio_clic.take() {
                if (posicion - inicio).length() < UMBRAL_CLIC {
                    entrada.clic = Some(posicion);
                }
            }
        }
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let delta = rl.get_mouse_delta();
            entrada.yaw += delta.x * SENSIBILIDAD_MOUSE;
            entrada.pitch += delta.y * SENSIBILIDAD_MOUSE;
        }
    }

    fn leer_gamepad(&mut self, rl: &RaylibHandle, entrada: &mut Entrada) {
        let disponible = rl.is_gamepad_available(GAMEPAD);
        if disponible != self.gamepad_conectado {
            self.gamepad_conectado = disponible;
            match rl.get_gamepad_name(GAMEPAD) {
                Some(nombre) if disponible => println!("Control conectado: {}", nombre),
                _ => println!("Control desconectado"),
            }
        }
        if !disponible {
            return;
        }

        let eje = |a: GamepadAxis| {
            let valor = rl.get_gamepad_axis_movement(GAMEPAD, a);
            if valor.abs() < ZONA_MUERTA { 0.0 } else { valor }
        };

        entrada.yaw += eje(GamepadAxis::GAMEPAD_AXIS_LEFT_X) * VELOCIDAD_ROTACION;
        entrada.pitch -= eje(GamepadAxis::GAMEPAD_AXIS_LEFT_Y) * VELOCIDAD_ROTACION;
        entrada.zoom -= eje(GamepadAxis::GAMEPAD_AXIS_RIGHT_Y) * 0.3;
        if rl.is_gamepad_button_down(GAMEPAD, GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1) {
            entrada.zoom += 0.2;
        }
        if rl.is_gamepad_button_down(GAMEPAD, GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1) {
            entrada.zoom -= 0.2;
        }

        if rl.is_gamepad_button_pressed(GAMEPAD, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN) {
            entrada.reiniciar = true;
        }
    }
}
