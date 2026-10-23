mod framebuffer;
mod luz;
mod triangle;
mod thrust;
mod particle_renderer;

use cohete::color::Relleno;
use cohete::controles::Controles;
use cohete::obj::Model;
use framebuffer::Framebuffer;
use particle_renderer::ParticulaPantalla;
use luz::{sombrear, Sombreado};
use raylib::prelude::*;
use triangle::{profundidad_en, triangle};
use thrust::ThrustEngine;

const SCREEN_WIDTH: i32 = 1200;
const SCREEN_HEIGHT: i32 = 800;
const COLOR_SELECCION: Color = Color::new(255, 0, 255, 255);

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

fn proyectar(r: Vector3, view: &View, focal: f32) -> Option<Vector3> {
    let depth = view.distance - r.z;
    if depth <= 0.1 {
        return None;
    }

    let scale = focal / depth;
    Some(Vector3::new(
        SCREEN_WIDTH as f32 / 2.0 + r.x * scale,
        SCREEN_HEIGHT as f32 / 2.0 - r.y * scale,
        depth,
    ))
}

fn project(v: Vector3, view: &View, focal: f32) -> Option<Vector3> {
    proyectar(rotar(v, view), view, focal)
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

fn sombreado_cara_con_propulsor(rotados: &[Vector3; 3], view: &View, thrust_light: Option<(Vector3, f32)>) -> Sombreado {
    let normal = (rotados[1] - rotados[0]).cross(rotados[2] - rotados[0]);
    if normal.length() < 1e-6 {
        return Sombreado::NINGUNO;
    }
    let mut normal = normal.normalized();

    let centro = (rotados[0] + rotados[1] + rotados[2]) / 3.0;
    let hacia_camara = (Vector3::new(0.0, 0.0, view.distance) - centro).normalized();
    if normal.dot(hacia_camara) < 0.0 {
        normal = -normal;
    }
    
    // Calcular iluminación base
    let mut sombra = sombrear(normal, hacia_camara);
    
    // Agregar iluminación del propulsor si está cerca
    if let Some((light_pos, intensidad)) = thrust_light {
        let distancia = (light_pos - centro).length();
        if distancia > 1e-4 && distancia < 9.0 {
            let hacia_luz = (light_pos - centro) / distancia;
            let contrib_luz = normal.dot(hacia_luz).max(0.0);
            let atenuacion = 1.0 / (1.0 + distancia * distancia * 0.05);
            sombra.difuso = (sombra.difuso + contrib_luz * atenuacion * 0.6 * intensidad).min(1.0);
        }
    }
    
    sombra
}

fn render(framebuffer: &mut Framebuffer, model: &Model, view: &View, seleccionado: Option<usize>, thrust_engine: &ThrustEngine) {
    let focal = SCREEN_HEIGHT as f32;
    let resaltado = Relleno::Solido(COLOR_SELECCION);

    type Visible<'a> = ([Vector3; 3], [Vector2; 3], &'a Relleno, Sombreado);
    let mut opacos: Vec<Visible> = Vec::new();
    let mut translucidos: Vec<Visible> = Vec::new();

    // Calcular posición de luz del propulsor (ANTES de renderizar el modelo)
    let light_pos = boquilla(model)
        .map(|_| (rotar(thrust_engine.light_position, view), thrust_engine.intensity));

    for (indice, tri) in model.triangles.iter().enumerate() {
        let rotados = tri.vertices.map(|v| rotar(v - model.center, view));
        let projected: Vec<Vector3> = rotados
            .iter()
            .filter_map(|&r| proyectar(r, view, focal))
            .collect();

        if projected.len() == 3 {
            let pantalla = [projected[0], projected[1], projected[2]];
            let (relleno, sombreado) = if seleccionado == Some(indice) {
                (&resaltado, Sombreado::NINGUNO)
            } else {
                // Usar iluminación del propulsor
                (&tri.relleno, sombreado_cara_con_propulsor(&rotados, view, light_pos))
            };
            if relleno.es_opaco() {
                opacos.push((pantalla, tri.uvs, relleno, sombreado));
            } else {
                translucidos.push((pantalla, tri.uvs, relleno, sombreado));
            }
        }
    }

    let profundidad = |t: &Visible| t.0[0].z + t.0[1].z + t.0[2].z;
    translucidos.sort_by(|a, b| profundidad(b).total_cmp(&profundidad(a)));

    // Las partículas se proyectan una sola vez y se ordenan de atrás hacia adelante
    let particulas = preparar_particulas(thrust_engine, view, focal);

    // Cada hilo dibuja una franja: primero el modelo y luego la nube del propulsor
    framebuffer.render_parallel(|franja| {
        for (pantalla, uvs, relleno, sombreado) in opacos.iter().chain(translucidos.iter()) {
            triangle(franja, *pantalla, *uvs, relleno, *sombreado);
        }
        for p in &particulas {
            particle_renderer::dibujar_particula(franja, p);
        }
    });
}

fn preparar_particulas(thrust_engine: &ThrustEngine, view: &View, focal: f32) -> Vec<ParticulaPantalla> {
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
    let focal = SCREEN_HEIGHT as f32;
    let mut mejor: Option<(usize, f32)> = None;

    for (indice, tri) in model.triangles.iter().enumerate() {
        let projected: Vec<Vector3> = tri
            .vertices
            .iter()
            .filter_map(|&v| project(v - model.center, view, focal))
            .collect();
        if projected.len() != 3 {
            continue;
        }
        let pantalla = [projected[0], projected[1], projected[2]];
        if let Some(depth) = profundidad_en(pantalla, x, y) {
            if mejor.map_or(true, |(_, d)| depth < d) {
                mejor = Some((indice, depth));
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
    framebuffer.set_background_color(Color::new(20, 20, 30, 255));
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
        
        // Controlar intensidad del propulsor (Space/Enter para activar)
        thrust_engine.intensity = if rl.is_key_down(KeyboardKey::KEY_SPACE) || rl.is_key_down(KeyboardKey::KEY_ENTER) {
            1.0  // Máxima potencia
        } else {
            0.2  // Muy débil en idle
        };
        
        if let Some((nozzle_pos, thrust_dir, nozzle_radius)) = boquilla(&model) {
            thrust_engine.update(delta_time, nozzle_pos, thrust_dir, nozzle_radius);
        }
        
        if let Some(clic) = entrada.clic {
            seleccionado = triangulo_en(&model, &view, clic.x, clic.y);
            if let Some(i) = seleccionado {
                println!("[{}]", i);
            }
        }
        view.yaw += entrada.yaw;
        view.pitch = (view.pitch + entrada.pitch).clamp(-1.5, 1.5);
        view.distance = (view.distance - entrada.zoom * model.size * 0.1).max(1.0);

        render(&mut framebuffer, &model, &view, seleccionado, &thrust_engine);

        let mut d = rl.begin_drawing(&thread);
        framebuffer.render_buffer(&mut d);
        d.draw_fps(10, 10);
        d.draw_text("Flechas/WASD/Mouse/Stick: Rotar | Rueda/Q/E: Zoom | R: Reiniciar | SPACE: Encender propulsor | ESC: Salir", 10, 30, 20, Color::WHITE);
        let texto = match seleccionado {
            Some(i) => format!("Triangulo [{}] (magenta) | Clic: seleccionar | C: quitar", i),
            None => "Clic sobre un triangulo para ver su indice".to_string(),
        };
        d.draw_text(&texto, 10, 55, 20, Color::YELLOW);
    }
}
