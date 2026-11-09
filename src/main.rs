mod aterrizaje;
mod estrellas;
mod fondo_rosa;
mod fragment;
mod framebuffer;
mod hud;
mod laser;
#[allow(non_snake_case)]
mod lineaPetrova;
mod luz;
mod modelo_planeta;
mod particle_renderer;
mod pipeline;
mod planeta;
mod resplandor;
mod restos;
mod sol;
mod sonido;
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
use lineaPetrova::LineaPetrova;
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
        .resizable()
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

type Visible<'a> = ([ScreenVertex; 3], &'a Relleno, ShaderParams);

/// Transforma y recorta los triangulos de una malla. Con `eye_local` se descartan las caras
/// traseras en espacio local antes de transformar (solo mallas cerradas con normales).
/// `plantilla` aporta los parametros del shader; la normal se calcula por triangulo.
fn agregar_malla<'a>(
    malla: &'a planeta::Malla,
    uniforms: &Uniforms,
    plantilla: ShaderParams,
    eye_local: Option<Vector3>,
    salida: &mut Vec<Visible<'a>>,
) {
    let mut recortados = Vec::with_capacity(4);
    for (i, (tri, relleno)) in malla.triangulos.iter().zip(&malla.rellenos).enumerate() {
        if let Some(eye_local) = eye_local {
            if malla.normales[i].dot(eye_local - tri[0]) <= 0. {
                continue;
            }
        }
        let transformed = std::array::from_fn::<_, 3, _>(|k| {
            pipeline::vertex_shader(tri[k], Vector2::zero(), uniforms)
        });
        // El recorte descarta primero lo que queda fuera de la vista: lo mas barato va antes.
        pipeline::clip_triangle_into(transformed, &mut recortados);
        if recortados.is_empty() {
            continue;
        }
        let p = transformed.map(|v| v.world);
        let normal = (p[1] - p[0]).cross(p[2] - p[0]);
        if normal.length() < 1e-9 {
            recortados.clear();
            continue;
        }
        let shader = ShaderParams {
            normal: normal.normalized(),
            ..plantilla
        };
        for clipped in recortados.drain(..) {
            salida.push((clipped.map(|v| pipeline::viewport(v, uniforms)), relleno, shader));
        }
    }
}

fn render(
    framebuffer: &mut Framebuffer,
    model: &Model,
    matrices: &Uniforms,
    camera: &Camara,
    nave: &Nave,
    seleccionado: Option<usize>,
    thrust_engine: &ThrustEngine,
    linea_petrova: &LineaPetrova,
    laser: Option<&Laser>,
    luz_intensidad: f32,
    estrellas: &estrellas::Estrellas,
    datos: &hud::Datos,
    fondo_rosa: &fondo_rosa::FondoRosa,
    sistema: &SistemaPlanetario,
    restos: &restos::Restos,
    vision_ultra_roja: bool,
) {
    let focal = SCREEN_HEIGHT as f32;
    let resaltado = Relleno::Solido(COLOR_SELECCION);

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
            sol: Some(sistema.sol.centro),
            emisivo: None,
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

    // Cuerpos celestes: mismo pipeline (modelo -> vista -> proyeccion -> clipping) con la matriz
    // de cada uno. Se descartan los que quedan fuera de la vista y el detalle baja con la distancia.
    // El sol se dibuja con su shader emisivo; planetas y anillos reciben luz desde el sol.
    let base = ShaderParams {
        normal: Vector3::zero(),
        eye: camera.position,
        engine_light: None,
        unlit: false,
        sol: Some(sistema.sol.centro),
        emisivo: None,
    };
    if let Some(radio_px) = sistema.sol.proyeccion(matrices) {
        let mut uniforms = *matrices;
        uniforms.model = sistema.sol.matriz_modelo();
        let plantilla = ShaderParams {
            sol: None,
            emisivo: Some((sistema.sol.centro, sistema.tiempo)),
            ..base
        };
        let eye = camera.position - sistema.sol.centro;
        agregar_malla(sistema.sol.malla_lod(radio_px), &uniforms, plantilla, Some(eye), &mut opacos);
    }
    for planeta in &sistema.planetas {
        let Some(radio_px) = planeta.proyeccion(matrices) else {
            continue;
        };
        let mut uniforms = *matrices;
        uniforms.model = planeta.matriz_modelo();
        let eye = planeta
            .malla_cerrada()
            .then(|| planeta.a_local(camera.position - planeta.centro));
        agregar_malla(planeta.malla_lod(radio_px), &uniforms, base, eye, &mut opacos);
        if let Some(anillo) = &planeta.anillo {
            agregar_malla(anillo, &uniforms, base, None, &mut opacos);
        }
    }
    // Restos de naves: se descartan fuera de la vista y el detalle baja con la distancia.
    for resto in &restos.restos {
        let Some(radio_px) =
            planeta::proyeccion_esfera(matrices, resto.centro, resto.radio_limite, resto.radio_colision)
        else {
            continue;
        };
        let Some(malla) = resto.malla_lod(radio_px) else {
            continue;
        };
        let mut uniforms = *matrices;
        uniforms.model = resto.matriz_modelo();
        agregar_malla(malla, &uniforms, base, None, &mut opacos);
    }
    let profundidad = |t: &Visible| t.0[0].screen.z + t.0[1].screen.z + t.0[2].screen.z;
    translucidos.sort_by(|a, b| profundidad(b).total_cmp(&profundidad(a)));

    // Las partículas se proyectan una sola vez y se ordenan de atrás hacia adelante
    let particulas = preparar_particulas(thrust_engine, linea_petrova, matrices, focal);

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

    // Cada hilo dibuja una franja: primero el modelo, luego el laser y la nube del propulsor.
    // Los triangulos se reparten por franja una sola vez para que cada hilo solo recorra los
    // que tocan sus filas (conservando el orden: opacos y luego translucidos).
    let fondo = estrellas.preparar(matrices, camera.position, model.size.max(1.));
    let logo = fondo_rosa.preparar(matrices, camera.position, model.size.max(1.));
    let triangulos: Vec<&Visible> = opacos.iter().chain(translucidos.iter()).collect();
    let franjas = (SCREEN_HEIGHT + framebuffer::ALTO_FRANJA - 1) / framebuffer::ALTO_FRANJA;
    let mut por_franja: Vec<Vec<u32>> = vec![Vec::new(); franjas as usize];
    for (i, (pantalla, _, _)) in triangulos.iter().enumerate() {
        let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
        for v in pantalla {
            min_y = min_y.min(v.screen.y);
            max_y = max_y.max(v.screen.y);
        }
        if !(min_y.is_finite() && max_y.is_finite()) {
            continue;
        }
        let primera = ((min_y.floor() as i32).max(0) / framebuffer::ALTO_FRANJA).min(franjas - 1);
        let ultima = ((max_y.ceil() as i32).max(0) / framebuffer::ALTO_FRANJA).min(franjas - 1);
        if max_y < 0. || min_y >= SCREEN_HEIGHT as f32 {
            continue;
        }
        for f in primera..=ultima {
            por_franja[f as usize].push(i as u32);
        }
    }
    let resplandor = resplandor::Resplandor::preparar(matrices, sistema.sol.centro, sistema.sol.radio);
    framebuffer.render_parallel(|franja| {
        estrellas::dibujar(&fondo, franja);
        fondo_rosa.dibujar(franja, &logo, camera.position);
        if let Some(r) = &resplandor {
            r.dibujar(franja);
        }
        for &i in &por_franja[(franja.y0 / framebuffer::ALTO_FRANJA) as usize] {
            let (pantalla, relleno, shader) = triangulos[i as usize];
            triangle(franja, *pantalla, relleno, *shader);
        }
        // El filtro tine la escena (no el laser, las particulas ni el HUD) para que la linea resalte.
        if vision_ultra_roja {
            franja.filtro_rojizo();
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
    linea_petrova: &LineaPetrova,
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
    particulas.extend(linea_petrova.puntos().iter().filter_map(|p| {
        particle_renderer::proyectar_particula(
            matrices.effect_point(p.position),
            p.radius,
            p.color,
            0.0,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            focal,
        )
    }));
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

fn nave_en_tierra(sistema: &SistemaPlanetario) -> Nave {
    let mut nave = Nave::new();
    if let Some(tierra) = sistema.planetas.iter().find(|p| p.config.nombre == "TERRA") {
        let radio = tierra.config.radio;
        nave.position = tierra.centro + Vector3::new(0., radio * 0.6, radio * 3.);
        nave.liberar_base((tierra.centro - nave.position).normalized());
    }
    nave
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
    let mut sistema = SistemaPlanetario::nuevo();
    let mut linea_petrova = LineaPetrova::nueva(&sistema);
    let mut vision_ultra_roja = false;
    let mut nave = nave_en_tierra(&sistema);
    let mut camera = Camara::new(&nave, size);
    let mut controles = Controles::new();
    let mut selected = None;
    let mut thrust = ThrustEngine::new();
    let mut laser = Laser::new();
    let mut previous_mode = Propulsor::Fuego;
    let mut selected_power = 0.6;
    let mut last = std::time::Instant::now();
    let mut gestor = GestorAterrizaje::nuevo();
    let mut restos = {
        let ruta = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/Nave_destruida.obj");
        restos::Restos::nuevo(ruta, size).unwrap_or_else(|e| {
            eprintln!("Sin restos de naves: {e}");
            restos::Restos::vacio()
        })
    };
    let mut recorrido_km = 0f64;
    // El dispositivo de audio debe declararse antes que el sonido que lo usa.
    let audio = match RaylibAudio::init_audio_device() {
        Ok(audio) => Some(audio),
        Err(e) => {
            eprintln!("Sin audio: {e}");
            None
        }
    };
    let mut sonido = audio.as_ref().and_then(sonido::SonidoNave::cargar);
    println!("W/S: subir/bajar nariz | A/D: girar izquierda/derecha | ESPACIO: acelerar (soltar: frenar) | 1/2/3: potencia 30/60/100% | laser automatico >36500 km/h | rueda/+/-: distancia de camara | R: vision ultra roja (linea Petrova) | T: reiniciar | clic: seleccionar | C: quitar | F11: pantalla completa | ESC: salir");
    println!("SISTEMA: Mercurio, Venus, Terra (+Luna), Marte, Jupiter, Saturno, Urano, Neptuno y Pluton. La nave aparece cerca de TERRA.");
    println!("ATERRIZAJE: apunta a un cuerpo y avanza; el asistente reduce la velocidad al descender. Posado: A/D giran, ESPACIO despega.");
    println!("MODO LOCAL: cerca de un cuerpo, O lo activa/desactiva. A/D giran el rumbo sobre el horizonte del planeta, W/S inclinan la nariz respecto a el; la gravedad sigue actuando.");
    while !rl.window_should_close() {
        let now = std::time::Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.05);
        last = now;
        let input = controles.leer(&rl);
        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            rl.toggle_fullscreen();
        }
        if input.reiniciar {
            nave = nave_en_tierra(&sistema);
            camera = Camara::new(&nave, size);
            gestor = GestorAterrizaje::nuevo();
            restos.olvidar();
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
        if input.vision_ultra_roja {
            vision_ultra_roja = !vision_ultra_roja;
        }
        if vision_ultra_roja {
            linea_petrova.actualizar(dt, &sistema);
        } else {
            linea_petrova.apagar();
        }
        restos.actualizar(dt, &sistema);
        if input.modo_local && !input.reiniciar {
            gestor.alternar_modo_local(&mut nave, &sistema);
        }
        let avanzando = input.avanzar && !input.reiniciar;
        if !input.reiniciar {
            if gestor.posada() {
                gestor.superficie(&mut nave, &sistema, input.yaw, input.avanzar);
            } else {
                let en_local = gestor.vuelo_local(
                    &mut nave,
                    &sistema,
                    input.yaw,
                    input.pitch,
                    avanzando,
                    selected_power,
                    dt,
                );
                if !en_local {
                    nave.step(input.yaw, input.pitch, avanzando, selected_power, dt);
                }
                gestor.vuelo(&mut nave, &sistema, size, dt);
            }
            if restos.colisionar_nave(&mut nave, size * 0.5, gestor.posada()) {
                gestor.avisar_colision();
            }
        }
        let moving = avanzando && !gestor.posada();
        recorrido_km +=  nave.speed_kmh as f64 * dt as f64 / 3600.;
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
        if let Some(sonido) = sonido.as_mut() {
            sonido.actualizar(
                dt,
                moving,
                propulsor == Propulsor::Laser,
                selected_power,
                nave.speed_kmh,
            );
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
        if let Some(click) = input
            .clic
            .and_then(|c| framebuffer.a_framebuffer(c, (rl.get_screen_width(), rl.get_screen_height())))
        {
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
        datos.estado_vuelo = gestor.titulo();
        datos.posada = gestor.posada();
        datos.vision_ultra_roja = vision_ultra_roja;
        datos.con_sistema(&nave, &sistema, size * aterrizaje::HOLGURA, recorrido_km);
        render(
            &mut framebuffer,
            &model,
            &matrices,
            &camera,
            &nave,
            selected,
            &thrust,
            &linea_petrova,
            beam,
            power,
            &estrellas,
            &datos,
            &fondo_rosa,
            &sistema,
            &restos,
            vision_ultra_roja,
        );
        let mut d = rl.begin_drawing(&thread);
        framebuffer.render_buffer(&mut d);
    }
}
