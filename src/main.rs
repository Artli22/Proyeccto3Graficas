mod estrellas;
mod fragment;
mod framebuffer;
mod laser;
mod luz;
mod particle_renderer;
mod pipeline;
mod thrust;
mod triangle;

use cohete::color::Relleno;
use cohete::controles::Controles;
use cohete::obj::Model;
use fragment::ShaderParams;
use framebuffer::Framebuffer;
use laser::Laser;
use particle_renderer::ParticulaPantalla;
use pipeline::{ScreenVertex, Uniforms};
use raylib::prelude::*;
use thrust::ThrustEngine;
use triangle::{profundidad_en, triangle};

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
    let (mut rl, thread) = raylib::init()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Software Renderer")
        .build();
    rl.set_target_fps(60);
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

fn render(
    framebuffer: &mut Framebuffer,
    model: &Model,
    view: &View,
    seleccionado: Option<usize>,
    thrust_engine: &ThrustEngine,
    laser: Option<&Laser>,
    luz_intensidad: f32,
    estrellas: &estrellas::Estrellas,
) {
    let focal = SCREEN_HEIGHT as f32;
    let resaltado = Relleno::Solido(COLOR_SELECCION);

    type Visible<'a> = ([ScreenVertex; 3], &'a Relleno, ShaderParams);
    let matrices = Uniforms::new(
        model.center,
        view.yaw,
        view.pitch,
        view.distance,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
    );
    let mut opacos: Vec<Visible> = Vec::new();
    let mut translucidos: Vec<Visible> = Vec::new();

    // Calcular posición de luz del propulsor (ANTES de renderizar el modelo)
    let light_pos =
        boquilla(model).map(|_| (rotar(thrust_engine.light_position, view), luz_intensidad));

    for (indice, tri) in model.triangles.iter().enumerate() {
        let transformed = std::array::from_fn::<_, 3, _>(|i| {
            pipeline::vertex_shader(tri.vertices[i], tri.uvs[i], &matrices)
        });
        let p = transformed.map(|v| v.world);
        let normal = (p[1] - p[0]).cross(p[2] - p[0]);
        if normal.length() < 1e-6 {
            continue;
        }
        let shader = ShaderParams {
            normal: normal.normalized(),
            eye: Vector3::new(0., 0., view.distance),
            engine_light: light_pos,
            unlit: seleccionado == Some(indice),
        };
        let material = if shader.unlit {
            &resaltado
        } else {
            &tri.relleno
        };
        // Ensamblado -> clipping homogeneo -> division de perspectiva -> viewport.
        for clipped in pipeline::clip_triangle(transformed) {
            let screen = clipped.map(|v| pipeline::viewport(v, &matrices));
            if material.es_opaco() {
                opacos.push((screen, material, shader));
            } else {
                translucidos.push((screen, material, shader));
            }
        }
    }

    let profundidad = |t: &Visible| t.0[0].screen.z + t.0[1].screen.z + t.0[2].screen.z;
    translucidos.sort_by(|a, b| profundidad(b).total_cmp(&profundidad(a)));

    // Las partículas se proyectan una sola vez y se ordenan de atrás hacia adelante
    let particulas = preparar_particulas(thrust_engine, view, focal);

    // El láser sale de la misma boquilla que el propulsor de fuego
    let haz = laser.and_then(|l| {
        let (pos, dir, _) = boquilla(model)?;
        l.preparar(
            pos + dir * 0.15,
            dir,
            |v| rotar(v, view),
            view.distance,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            focal,
        )
    });

    // Cada hilo dibuja una franja: primero el modelo, luego el láser y la nube del propulsor
    framebuffer.render_parallel(|franja| {
        estrellas.dibujar(franja);
        for (pantalla, relleno, shader) in opacos.iter().chain(translucidos.iter()) {
            triangle(franja, *pantalla, relleno, *shader);
        }
        if let Some(haz) = &haz {
            laser::dibujar_laser(franja, haz);
        }
        for p in &particulas {
            particle_renderer::dibujar_particula(franja, p);
        }
    });
}

fn preparar_particulas(
    thrust_engine: &ThrustEngine,
    view: &View,
    focal: f32,
) -> Vec<ParticulaPantalla> {
    let mut particulas: Vec<ParticulaPantalla> = thrust_engine
        .get_particles()
        .iter()
        .filter_map(|p| {
            particle_renderer::proyectar_particula(
                rotar(p.position, view),
                thrust_engine.get_particle_radius(p),
                thrust_engine.get_particle_color(p),
                view.distance,
                SCREEN_WIDTH,
                SCREEN_HEIGHT,
                focal,
            )
        })
        .collect();
    particulas.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    particulas
}

fn triangulo_en(model: &Model, view: &View, x: f32, y: f32) -> Option<usize> {
    let matrices = Uniforms::new(
        model.center,
        view.yaw,
        view.pitch,
        view.distance,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
    );
    let mut mejor: Option<(usize, f32)> = None;
    for (indice, tri) in model.triangles.iter().enumerate() {
        // La seleccion utiliza exactamente el mismo clipping del renderizador.
        for screen in pipeline::prepare(tri.vertices, tri.uvs, &matrices) {
            if let Some(depth) = profundidad_en(screen.map(|v| v.screen), x, y) {
                if mejor.map_or(true, |(_, d)| depth < d) {
                    mejor = Some((indice, depth));
                }
            }
        }
    }
    mejor.map(|(indice, _)| indice)
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

    let mut framebuffer = Framebuffer::new(&mut rl, &thread, SCREEN_WIDTH, SCREEN_HEIGHT);
    framebuffer.set_background_color(Color::BLACK);
    let estrellas = estrellas::Estrellas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    println!("Controles: WASD/flechas/arrastrar: rotar | rueda/Q/+/-: zoom | R: reiniciar | clic: seleccionar | C: quitar | E: fuego/laser | espacio/Enter: potencia | Escape: salir");
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
        if let Some(clic) = entrada.clic {
            seleccionado = triangulo_en(&model, &view, clic.x, clic.y);
            if let Some(i) = seleccionado {
                println!("[{}]", i);
            }
        }

        let (haz_laser, luz_intensidad) = match propulsor {
            Propulsor::Fuego => (None, thrust_engine.intensity),
            Propulsor::Laser => (Some(&laser), laser.potencia * 0.6),
        };
        render(
            &mut framebuffer,
            &model,
            &view,
            seleccionado,
            &thrust_engine,
            haz_laser,
            luz_intensidad,
            &estrellas,
        );

        let mut d = rl.begin_drawing(&thread);
        framebuffer.render_buffer(&mut d);
        // Unica operacion de dibujo raylib: presentar la imagen calculada en CPU.
    }
}
