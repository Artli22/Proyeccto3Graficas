mod aterrizaje;
mod estrellas;
mod fondo_rosa;
mod fragment;
mod framebuffer;
mod hud;
mod laser;
mod luz;
mod particle_renderer;
mod pipeline;
mod planeta;
mod thrust;
mod triangle;
mod vuelo;

use aterrizaje::GestorAterrizaje;
use cohete::color::Relleno;
use cohete::controles::Controles;
use cohete::obj::Model;
use fragment::ShaderParams;
use framebuffer::Framebuffer;
use laser::Laser;
use particle_renderer::ParticulaPantalla;
use pipeline::{ScreenVertex, Uniforms};
use planeta::SistemaPlanetario;
use raylib::prelude::*;
use thrust::ThrustEngine;
use triangle::{profundidad_en, triangle};
use vuelo::{Camara, Nave};

const SCREEN_WIDTH: i32 = 1200;
const SCREEN_HEIGHT: i32 = 800;
const COLOR_SELECCION: Color = Color::new(255, 0, 255, 255);

#[derive(Clone, Copy, PartialEq)]
enum Propulsor {
    Fuego,
    Laser,
}

fn init() -> (RaylibHandle, RaylibThread) {
    let (mut rl, thread) = raylib::init()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Vuelo espacial CPU - tercera persona")
        .build();
    rl.set_target_fps(60);
    (rl, thread)
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
    matrices: &Uniforms,
    camera: &Camara,
    nave: &Nave,
    seleccionado: Option<usize>,
    thrust_engine: &ThrustEngine,
    laser: Option<&Laser>,
    luz_intensidad: f32,
    estrellas: &estrellas::Estrellas,
    datos: &hud::Datos,
    fondo_rosa: &fondo_rosa::FondoRosa,
    sistema: &SistemaPlanetario,
) {
    let focal = SCREEN_HEIGHT as f32;
    let resaltado = Relleno::Solido(COLOR_SELECCION);

    type Visible<'a> = ([ScreenVertex; 3], &'a Relleno, ShaderParams);

    let mut opacos: Vec<Visible> = Vec::new();
    let mut translucidos: Vec<Visible> = Vec::new();

    // Calcular posición de luz del propulsor (ANTES de renderizar el modelo)
    let light_pos = if luz_intensidad > 0. {
        Some((thrust_engine.light_position, luz_intensidad))
    } else {
        None
    };

    for (indice, tri) in model.triangles.iter().enumerate() {
        let transformed = std::array::from_fn::<_, 3, _>(|i| {
            pipeline::vertex_shader(tri.vertices[i], tri.uvs[i], matrices)
        });
        let p = transformed.map(|v| v.world);
        let normal = (p[1] - p[0]).cross(p[2] - p[0]);
        if normal.length() < 1e-6 {
            continue;
        }
        let shader = ShaderParams {
            normal: normal.normalized(),
            eye: camera.position,
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
            let screen = clipped.map(|v| pipeline::viewport(v, matrices));
            if material.es_opaco() {
                opacos.push((screen, material, shader));
            } else {
                translucidos.push((screen, material, shader));
            }
        }
    }

    // Planetas: mismo pipeline (modelo -> vista -> proyeccion -> clipping) con su propia matriz de modelo.
    for planeta in &sistema.planetas {
        let mut uniforms = *matrices;
        uniforms.model = planeta.matriz_modelo();
        for (tri, relleno) in planeta.triangulos.iter().zip(&planeta.rellenos) {
            let transformed = std::array::from_fn::<_, 3, _>(|i| {
                pipeline::vertex_shader(tri[i], Vector2::zero(), &uniforms)
            });
            let p = transformed.map(|v| v.world);
            let normal = (p[1] - p[0]).cross(p[2] - p[0]);
            if normal.length() < 1e-6 {
                continue;
            }
            let normal = normal.normalized();
            // Descarte de caras traseras: la malla es cerrada y sus normales apuntan hacia afuera.
            if normal.dot(camera.position - p[0]) <= 0. {
                continue;
            }
            let shader = ShaderParams {
                normal,
                eye: camera.position,
                engine_light: None,
                unlit: false,
            };
            for clipped in pipeline::clip_triangle(transformed) {
                let screen = clipped.map(|v| pipeline::viewport(v, matrices));
                opacos.push((screen, relleno, shader));
            }
        }
    }

    let profundidad = |t: &Visible| t.0[0].screen.z + t.0[1].screen.z + t.0[2].screen.z;
    translucidos.sort_by(|a, b| profundidad(b).total_cmp(&profundidad(a)));

    // Las partículas se proyectan una sola vez y se ordenan de atrás hacia adelante
    let particulas = preparar_particulas(thrust_engine, matrices, focal);

    // El láser sale de la misma boquilla que el propulsor de fuego
    let haz = laser.and_then(|l| {
        let (pos, dir, _) = boquilla(model)?;
        l.preparar(
            nave.centered_point(pos) + nave.direction(dir) * 0.15,
            nave.direction(dir),
            |v| matrices.effect_point(v),
            0.0,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            focal,
        )
    });

    // Cada hilo dibuja una franja: primero el modelo, luego el láser y la nube del propulsor
    let fondo = estrellas.preparar(matrices, camera.position, model.size.max(1.));
    let logo = fondo_rosa.preparar(matrices, camera.position, model.size.max(1.));
    framebuffer.render_parallel(|franja| {
        estrellas::dibujar(&fondo, franja);
        fondo_rosa.dibujar(franja, &logo, camera.position);
        for (pantalla, relleno, shader) in opacos.iter().chain(translucidos.iter()) {
            triangle(franja, *pantalla, relleno, *shader);
        }
        if let Some(haz) = &haz {
            laser::dibujar_laser(franja, haz);
        }
        for p in &particulas {
            particle_renderer::dibujar_particula(franja, p);
        }
        hud::dibujar(franja, datos, SCREEN_HEIGHT);
    });
}

fn preparar_particulas(
    thrust_engine: &ThrustEngine,
    matrices: &Uniforms,
    focal: f32,
) -> Vec<ParticulaPantalla> {
    let mut particulas: Vec<ParticulaPantalla> = thrust_engine
        .get_particles()
        .iter()
        .filter_map(|p| {
            particle_renderer::proyectar_particula(
                matrices.effect_point(p.position),
                thrust_engine.get_particle_radius(p),
                thrust_engine.get_particle_color(p),
                0.0,
                SCREEN_WIDTH,
                SCREEN_HEIGHT,
                focal,
            )
        })
        .collect();
    particulas.sort_by(|a, b| b.depth.total_cmp(&a.depth));
    particulas
}

fn triangulo_en(model: &Model, matrices: &Uniforms, x: f32, y: f32) -> Option<usize> {
    let mut mejor: Option<(usize, f32)> = None;
    for (indice, tri) in model.triangles.iter().enumerate() {
        // La seleccion utiliza exactamente el mismo clipping del renderizador.
        for screen in pipeline::prepare(tri.vertices, tri.uvs, matrices) {
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
        Ok(m) if !m.triangles.is_empty() => m,
        Ok(_) => {
            eprintln!("El modelo no contiene triangulos");
            return;
        }
        Err(e) => {
            eprintln!("No se pudo leer el OBJ: {e}");
            return;
        }
    };
    let fondo_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/SancorSeguros1.obj");
    let fondo_rosa = match fondo_rosa::FondoRosa::load(fondo_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("No se pudo preparar el fondo rosa: {e}");
            return;
        }
    };
    let size = model.size.max(1.);
    let mut framebuffer = Framebuffer::new(&mut rl, &thread, SCREEN_WIDTH, SCREEN_HEIGHT);
    framebuffer.set_background_color(Color::BLACK);
    let estrellas = estrellas::Estrellas::new();
    let mut nave = Nave::new();
    let mut camera = Camara::new(&nave, size);
    let mut controles = Controles::new();
    let mut selected = None;
    let mut thrust = ThrustEngine::new();
    let mut laser = Laser::new();
    let mut previous_mode = Propulsor::Fuego;
    let mut selected_power = 0.6;
    let mut last = std::time::Instant::now();
    let mut sistema = SistemaPlanetario::nuevo();
    let mut gestor = GestorAterrizaje::nuevo();
    println!("W/S: subir/bajar nariz | A/D: girar izquierda/derecha | ESPACIO: acelerar (soltar: frenar) | 1/2/3: potencia 30/60/100% | laser automatico >1000 km/h | rueda/+/-: distancia de camara | R: reiniciar | clic: seleccionar | C: quitar | ESC: salir");
    println!("ATERRIZAJE: apunta al planeta TERRA y avanza; el asistente reduce la velocidad al descender. Posado: A/D giran, ESPACIO despega.");
    while !rl.window_should_close() {
        let now = std::time::Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.05);
        last = now;
        let input = controles.leer(&rl);
        if input.reiniciar {
            nave = Nave::new();
            camera = Camara::new(&nave, size);
            gestor = GestorAterrizaje::nuevo();
            thrust.apagar();
            laser.apagar();
            selected = None;
            selected_power = 0.6;
            previous_mode = Propulsor::Fuego;
        }
        if input.quitar_seleccion {
            selected = None;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_ONE) {
            selected_power = 0.3;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_TWO) {
            selected_power = 0.6;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_THREE) {
            selected_power = 1.;
        }
        sistema.actualizar(dt);
        let avanzando = input.avanzar && !input.reiniciar;
        if !input.reiniciar {
            if gestor.posada() {
                gestor.superficie(&mut nave, &sistema, input.yaw, input.avanzar);
            } else {
                nave.step(input.yaw, input.pitch, avanzando, selected_power, dt);
                gestor.vuelo(&mut nave, &sistema, size, dt);
            }
        }
        let moving = avanzando && !gestor.posada();
        let propulsor = if nave.laser_mode() {
            Propulsor::Laser
        } else {
            Propulsor::Fuego
        };
        if propulsor != previous_mode {
            thrust.apagar();
            laser.apagar();
            previous_mode = propulsor;
        }
        camera.update(&nave, size, input.zoom, dt);
        // Sin Espacio no hay efectos aunque la nave aun este frenando.
        if moving {
            if let Some((pos, dir, radius)) = boquilla(&model) {
                let world_pos = nave.centered_point(pos);
                let world_dir = nave.direction(dir).normalized();
                thrust.light_position = world_pos + world_dir;
                match propulsor {
                    Propulsor::Fuego => {
                        laser.apagar();
                        thrust.intensity = selected_power;
                        thrust.update(dt, world_pos, world_dir, radius);
                    }
                    Propulsor::Laser => {
                        thrust.apagar();
                        laser.update(dt, selected_power);
                    }
                }
            }
        } else {
            thrust.apagar();
            laser.apagar();
        }
        let matrices = Uniforms::scene(
            nave.model_matrix(model.center),
            camera.view_matrix(),
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
        );
        if let Some(click) = input.clic {
            selected = triangulo_en(&model, &matrices, click.x, click.y);
            if let Some(i) = selected {
                println!("Triangulo [{i}]");
            }
        }
        let (beam, power) = if moving {
            match propulsor {
                Propulsor::Fuego => (None, thrust.intensity),
                Propulsor::Laser => (Some(&laser), laser.potencia * 0.6),
            }
        } else {
            (None, 0.)
        };
        let mut datos = hud::Datos::new(&nave, selected_power, moving);
        let (texto, progreso) = gestor.hud(&sistema);
        datos.landing_status = texto;
        datos.landing_progress = progreso;
        render(
            &mut framebuffer,
            &model,
            &matrices,
            &camera,
            &nave,
            selected,
            &thrust,
            beam,
            power,
            &estrellas,
            &datos,
            &fondo_rosa,
            &sistema,
        );
        let mut d = rl.begin_drawing(&thread);
        framebuffer.render_buffer(&mut d);
    }
}
