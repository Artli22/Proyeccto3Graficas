mod estrellas;
mod gpu;
mod laser;
mod thrust;

use cohete::controles::Controles;
use cohete::obj::Model;
use gpu::GpuRenderer;
use laser::Laser;
use raylib::prelude::*;
use thrust::ThrustEngine;

const SCREEN_WIDTH: i32 = 1200;
const SCREEN_HEIGHT: i32 = 800;
const COLOR_SELECCION: Color = Color::new(255, 0, 255, 255);

#[derive(Clone, Copy, PartialEq)]
enum Propulsor {
    Fuego,
    Laser,
}

#[derive(Clone, Copy)]
struct View {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

fn init() -> (RaylibHandle, RaylibThread) {
    let mut flags = raylib::ffi::ConfigFlags::FLAG_WINDOW_HIGHDPI as u32;
    if std::env::args().any(|a| a == "--verificar-gpu") {
        flags |= raylib::ffi::ConfigFlags::FLAG_WINDOW_HIDDEN as u32;
    }
    unsafe { raylib::ffi::SetConfigFlags(flags); }
    let (mut rl, thread) = raylib::init()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("CargaModelo - GPU / raylib")
        .build();
    rl.set_target_fps(0); // F: alternar entre 60 FPS y sin limite para comparar.
    (rl, thread)
}

fn rotar(v: Vector3, view: &View) -> Vector3 {
    let (sin_y, cos_y) = view.yaw.sin_cos();
    let (sin_p, cos_p) = view.pitch.sin_cos();

    let x = v.x * cos_y + v.z * sin_y;
    let z = -v.x * sin_y + v.z * cos_y;
    let y = v.y * cos_p - z * sin_p;
    let z = v.y * sin_p + z * cos_p;
    Vector3::new(x, y, z)
}

// Boquilla del propulsor en el espacio del modelo (centrado): punto medio de los triángulos 42 y 120,
// dirección de salida (normal hacia afuera del modelo) y radio de emisión.
fn boquilla(model: &Model) -> Option<(Vector3, Vector3, f32)> {
    if model.triangles.len() <= 120 {
        return None;
    }
    let a = &model.triangles[42].vertices;
    let b = &model.triangles[120].vertices;
    let centro = |t: &[Vector3; 3]| (t[0] + t[1] + t[2]) / 3.0 - model.center;
    let pos = (centro(a) + centro(b)) / 2.0;

    let normal = (a[1] - a[0]).cross(a[2] - a[0]);
    if normal.length() < 1e-6 {
        return None;
    }
    let mut dir = normal.normalized();
    if dir.dot(pos) < 0.0 {
        dir = -dir;
    }

    let mut radio = 0.0_f32;
    for v in a.iter().chain(b.iter()) {
        radio = radio.max((*v - model.center - pos).length());
    }
    Some((pos, dir, radio * 0.6))
}

fn main() {
    let (mut rl, thread) = init();

    let obj_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/Intento1Lab3.obj");
    let model = match Model::load_obj(obj_path) {
        Ok(model) if !model.triangles.is_empty() => model,
        Ok(_) => {
            eprintln!("El archivo no contiene triángulos: {}", obj_path);
            return;
        }
        Err(e) => {
            eprintln!("No se pudo leer {}: {}", obj_path, e);
            return;
        }
    };
    println!("Modelo cargado: {} triángulos", model.triangles.len());

    let mut renderer = GpuRenderer::new(&mut rl, &thread);
    if std::env::args().any(|a| a == "--verificar-gpu") {
        renderer.verificar(&mut rl, &thread, &model);
        return;
    }
    let mut limite_fps = false;
    let vista_inicial = View {
        yaw: 0.8,
        pitch: 0.4,
        distance: model.size.max(1.0) * 2.0,
    };
    let mut view = vista_inicial;
    let mut controles = Controles::new();
    let mut seleccionado: Option<usize> = None;

    // Crear el motor de propulsión
    let mut thrust_engine = ThrustEngine::new();
    let mut laser = Laser::new();
    let mut propulsor = Propulsor::Fuego;
    let mut last_time = std::time::Instant::now();

    while !rl.window_should_close() {
        // Calcular delta time
        let now = std::time::Instant::now();
        let delta_time = now.duration_since(last_time).as_secs_f32();
        last_time = now;

        let entrada = controles.leer(&rl);
        if entrada.reiniciar {
            view = vista_inicial;
        }
        if entrada.quitar_seleccion {
            seleccionado = None;
        }

        if rl.is_key_pressed(KeyboardKey::KEY_E) {
            propulsor = match propulsor {
                Propulsor::Fuego => Propulsor::Laser,
                Propulsor::Laser => Propulsor::Fuego,
            };
        }

        // Space/Enter para máxima potencia
        let encendido =
            rl.is_key_down(KeyboardKey::KEY_SPACE) || rl.is_key_down(KeyboardKey::KEY_ENTER);

        // Al cambiar de propulsor, la nube de fuego deja de emitir y se disipa sola
        thrust_engine.intensity = match propulsor {
            Propulsor::Fuego => {
                if encendido {
                    1.0
                } else {
                    0.2
                }
            }
            Propulsor::Laser => 0.0,
        };
        laser.update(delta_time, if encendido { 1.0 } else { 0.7 });

        if let Some((nozzle_pos, thrust_dir, nozzle_radius)) = boquilla(&model) {
            thrust_engine.update(delta_time, nozzle_pos, thrust_dir, nozzle_radius);
        }

        view.yaw += entrada.yaw;
        view.pitch = (view.pitch + entrada.pitch).clamp(-1.5, 1.5);
        view.distance = (view.distance - entrada.zoom * model.size * 0.1).max(0.11);
        if rl.is_key_pressed(KeyboardKey::KEY_F) {
            limite_fps = !limite_fps;
            rl.set_target_fps(if limite_fps { 60 } else { 0 });
        }
        if let Some(clic) = entrada.clic {
            seleccionado = gpu::seleccionar(&rl, &model, &view, clic);
            if let Some(i) = seleccionado {
                println!("[{}]", i);
            }
        }

        let (haz_laser, luz_intensidad) = match propulsor {
            Propulsor::Fuego => (None, thrust_engine.intensity),
            Propulsor::Laser => (Some(&laser), laser.potencia * 0.6),
        };
        renderer.actualizar(&view, &thrust_engine, luz_intensidad);

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        renderer.dibujar_fondo(&mut d);
        renderer.dibujar(
            &mut d,
            &model,
            &view,
            seleccionado,
            &thrust_engine,
            haz_laser,
        );
        d.draw_fps(10, 10);
        d.draw_text(
            if limite_fps {
                "GPU | F: quitar limite 60 FPS"
            } else {
                "GPU | F: limitar a 60 FPS"
            },
            180,
            10,
            18,
            Color::GREEN,
        );
        d.draw_text(
            "Flechas/WASD/Mouse/Stick: Rotar | Rueda/Q/+/-: Zoom | R: Reiniciar | ESC: Salir",
            10,
            30,
            20,
            Color::WHITE,
        );
        let texto = match seleccionado {
            Some(i) => format!(
                "Triangulo [{}] (magenta) | Clic: seleccionar | C: quitar",
                i
            ),
            None => "Clic sobre un triangulo para ver su indice".to_string(),
        };
        d.draw_text(&texto, 10, 55, 20, Color::YELLOW);
        let nombre = match propulsor {
            Propulsor::Fuego => "Fuego",
            Propulsor::Laser => "Laser rojo",
        };
        d.draw_text(
            &format!(
                "Propulsor: {} | E: cambiar propulsor | SPACE: maxima potencia",
                nombre
            ),
            10,
            80,
            20,
            Color::ORANGE,
        );
    }
}
