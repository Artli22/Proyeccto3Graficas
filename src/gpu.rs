use crate::laser::Laser;
use crate::thrust::ThrustEngine;
use crate::{boquilla, rotar, View, COLOR_SELECCION};
use cohete::obj::Model;
use raylib::prelude::*;

pub const NEAR: f32 = 0.1;
pub const FAR: f32 = 10000.0;

pub fn camera(view: &View) -> Camera3D {
    // Misma focal vertical que el software original: focal = alto de pantalla.
    let fovy = (2.0 * (0.5_f32).atan()).to_degrees();
    Camera3D::perspective(
        Vector3::new(0.0, 0.0, view.distance),
        Vector3::zero(),
        Vector3::new(0.0, 1.0, 0.0),
        fovy,
    )
}

pub struct GpuRenderer {
    estrellas: crate::estrellas::Estrellas,
    shader: Shader,
    laser_shader: Shader,
    smoke: Texture2D,
    angles: i32,
    eye: i32,
    light: i32,
    power: i32,
}

// Una sola cola para TODAS las superficies transparentes.
enum Transparent {
    Face(usize),
    Particle(Vector3, f32, Color),
    Beam,
}

impl GpuRenderer {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let estrellas = crate::estrellas::Estrellas::new(rl, thread);
        let shader = rl.load_shader_from_memory(
            thread,
            Some(include_str!("shaders/model.vs")),
            Some(include_str!("shaders/model.fs")),
        );
        // raylib puede devolver el shader por defecto si GLSL falla: no ocultar el fallo.
        assert!(
            shader.id != unsafe { raylib::ffi::rlGetShaderIdDefault() },
            "No se pudo compilar el shader GLSL 330. Revise el log de raylib."
        );
        let angles = shader.get_shader_location("angles");
        let eye = shader.get_shader_location("eye");
        let light = shader.get_shader_location("engineLight");
        let power = shader.get_shader_location("enginePower");
        assert!([angles, eye, light, power].iter().all(|&x| x >= 0));
        let laser_shader =
            rl.load_shader_from_memory(thread, None, Some(include_str!("shaders/laser.fs")));
        assert!(
            laser_shader.id != unsafe { raylib::ffi::rlGetShaderIdDefault() },
            "No se pudo compilar el shader del laser"
        );
        // Textura radial generada una sola vez; rasterizacion y alfa en GPU.
        let mut image = Image::gen_image_color(64, 64, Color::BLANK);
        for y in 0..64 {
            for x in 0..64 {
                let dx = (x as f32 + 0.5 - 32.0) / 32.0;
                let dy = (y as f32 + 0.5 - 32.0) / 32.0;
                let a = ((1.0 - dx * dx - dy * dy).max(0.0).powf(0.75) * 255.0) as u8;
                image.draw_pixel(x, y, Color::new(255, 255, 255, a));
            }
        }
        let smoke = rl
            .load_texture_from_image(thread, &image)
            .expect("Textura de particulas");
        // Funciones rlgl incluidas en raylib; no son una dependencia adicional.
        unsafe {
            raylib::ffi::rlSetClipPlanes(NEAR as f64, FAR as f64);
        }
        Self {
            estrellas,
            shader,
            laser_shader,
            smoke,
            angles,
            eye,
            light,
            power,
        }
    }

    pub fn dibujar_fondo(&mut self, d: &mut RaylibDrawHandle) {
        self.estrellas.dibujar(d);
    }

    pub fn actualizar(&mut self, view: &View, thrust: &ThrustEngine, power: f32) {
        self.shader
            .set_shader_value(self.angles, Vector2::new(view.yaw, view.pitch));
        self.shader
            .set_shader_value(self.eye, Vector3::new(0.0, 0.0, view.distance));
        self.shader
            .set_shader_value(self.light, rotar(thrust.light_position, view));
        self.shader.set_shader_value(self.power, power);
    }

    pub fn dibujar(
        &mut self,
        d: &mut RaylibDrawHandle,
        model: &Model,
        view: &View,
        selected: Option<usize>,
        thrust: &ThrustEngine,
        laser: Option<&Laser>,
    ) {
        let cam = camera(view);
        let mut scene = d.begin_mode3D(cam);
        // El original mostraba ambas caras. Mantenerlo sin duplicar triangulos.
        unsafe {
            raylib::ffi::rlDisableBackfaceCulling();
        }
        let face_color = |i: usize| {
            if selected == Some(i) {
                COLOR_SELECCION
            } else {
                model.triangles[i].relleno.color_en(Vector2::zero())
            }
        };
        let mut transparent: Vec<(f32, Transparent)> = Vec::new();
        {
            let mut shaded = scene.begin_shader_mode(&mut self.shader);
            for (i, tri) in model.triangles.iter().enumerate() {
                let color = face_color(i);
                if color.a == 255 {
                    let p = tri.vertices.map(|v| v - model.center);
                    shaded.draw_triangle3D(p[0], p[1], p[2], color);
                } else {
                    let center =
                        (tri.vertices[0] + tri.vertices[1] + tri.vertices[2]) / 3.0 - model.center;
                    transparent.push((view.distance - rotar(center, view).z, Transparent::Face(i)));
                }
            }
        }
        for p in thrust.get_particles() {
            let center = rotar(p.position, view);
            transparent.push((
                view.distance - center.z,
                Transparent::Particle(
                    center,
                    thrust.get_particle_radius(p) * 2.0,
                    thrust.get_particle_color(p),
                ),
            ));
        }
        if let (Some(laser), Some((origin, dir, _))) = (laser, boquilla(model)) {
            let a = rotar(origin + dir * 0.15, view);
            // Igual longitud y origen final que el laser original.
            let b = rotar(origin + dir * 0.15 + dir * 40.0, view);
            let (da, db) = (view.distance - a.z, view.distance - b.z);
            // Mismo recorte cercano del haz original (0.5 unidades).
            if da >= 0.5 || db >= 0.5 {
                let t0 = if da < 0.5 {
                    (0.5 - da) / (db - da)
                } else {
                    0.0
                };
                let t1 = if db < 0.5 {
                    (0.5 - da) / (db - da)
                } else {
                    1.0
                };
                let p = a + (b - a) * t0;
                let q = a + (b - a) * t1;
                let (d0, d1) = (view.distance - p.z, view.distance - q.z);
                let f = crate::SCREEN_HEIGHT as f32;
                let cx = crate::SCREEN_WIDTH as f32 * 0.5;
                let cy = crate::SCREEN_HEIGHT as f32 * 0.5;
                let endpoints = Vector4::new(
                    cx + p.x * f / d0,
                    cy - p.y * f / d0,
                    cx + q.x * f / d1,
                    cy - q.y * f / d1,
                );
                let viewport = unsafe {
                    Vector2::new(
                        raylib::ffi::GetRenderWidth() as f32,
                        raylib::ffi::GetRenderHeight() as f32,
                    )
                };
                for (name, value) in [
                    ("power", laser.brillo()),
                    ("nearPlane", NEAR),
                    ("farPlane", FAR),
                ] {
                    let loc = self.laser_shader.get_shader_location(name);
                    self.laser_shader.set_shader_value(loc, value);
                }
                let loc = self.laser_shader.get_shader_location("endpoints");
                self.laser_shader.set_shader_value(loc, endpoints);
                for (name, value) in [
                    ("inverseDepth", Vector2::new(1.0 / d0, 1.0 / d1)),
                    ("viewport", viewport),
                    (
                        "screenSize",
                        Vector2::new(crate::SCREEN_WIDTH as f32, crate::SCREEN_HEIGHT as f32),
                    ),
                ] {
                    let loc = self.laser_shader.get_shader_location(name);
                    self.laser_shader.set_shader_value(loc, value);
                }
                transparent.push(((d0 + d1) * 0.5, Transparent::Beam));
            }
            // Destello original: halo rojo y centro claro con radios dependientes de potencia.
            let power = laser.brillo();
            for (radius, color) in [
                (
                    0.9 * power + 0.2,
                    Color::new(255, 40, 30, (190.0 * power) as u8),
                ),
                (
                    0.3 * power + 0.08,
                    Color::new(255, 225, 215, (235.0 * power) as u8),
                ),
            ] {
                transparent.push((
                    view.distance - a.z,
                    Transparent::Particle(a, 2.0 * radius, color),
                ));
            }
        }
        transparent.sort_by(|a, b| b.0.total_cmp(&a.0));
        // Vaciar ANTES de cambiar profundidad: el estado afecta al batch al enviarse.
        unsafe {
            raylib::ffi::rlDrawRenderBatchActive();
            raylib::ffi::rlDisableDepthMask();
        }
        for (_, item) in transparent {
            match item {
                Transparent::Face(i) => {
                    let p = model.triangles[i].vertices.map(|v| v - model.center);
                    let mut shaded = scene.begin_shader_mode(&mut self.shader);
                    shaded.draw_triangle3D(p[0], p[1], p[2], face_color(i));
                }
                Transparent::Particle(p, size, color) => {
                    scene.draw_billboard(cam, &self.smoke, p, size, color)
                }
                Transparent::Beam => {
                    // Cuadrilatero de pantalla; el fragment shader reconstruye el perfil
                    // original y escribe la profundidad real del haz en gl_FragDepth.
                    let half_y = 0.5;
                    let half_x = half_y * crate::SCREEN_WIDTH as f32 / crate::SCREEN_HEIGHT as f32;
                    let z = view.distance - 1.0;
                    let p = [
                        Vector3::new(-half_x, -half_y, z),
                        Vector3::new(half_x, -half_y, z),
                        Vector3::new(half_x, half_y, z),
                        Vector3::new(-half_x, half_y, z),
                    ];
                    let mut glow = scene.begin_shader_mode(&mut self.laser_shader);
                    glow.draw_triangle3D(p[0], p[1], p[2], Color::WHITE);
                    glow.draw_triangle3D(p[0], p[2], p[3], Color::WHITE);
                }
            }
        }
        unsafe {
            raylib::ffi::rlDrawRenderBatchActive();
            raylib::ffi::rlEnableDepthMask();
            raylib::ffi::rlEnableBackfaceCulling();
        }
        // Al terminar Mode3D se restaura el modo 2D para la interfaz.
    }
}

pub fn seleccionar(rl: &RaylibHandle, model: &Model, view: &View, mouse: Vector2) -> Option<usize> {
    let ray = rl.get_screen_to_world_ray(mouse, camera(view));
    let mut best: Option<(usize, f32)> = None;
    for (i, t) in model.triangles.iter().enumerate() {
        let p = t.vertices.map(|v| rotar(v - model.center, view));
        let hit = raylib::core::collision::get_ray_collision_triangle(ray, p[0], p[1], p[2]);
        if hit.hit {
            let depth = view.distance - hit.point.z;
            // Mismos planos que el render: se selecciona la interseccion visible,
            // aunque otro vertice del triangulo este detras del plano cercano.
            if (NEAR..=FAR).contains(&depth)
                && best.map_or(true, |(_, distance)| hit.distance < distance)
            {
                best = Some((i, hit.distance));
            }
        }
    }
    best.map(|(i, _)| i)
}

impl GpuRenderer {
    /// Prueba real del pipeline, con contexto OpenGL oculto y lectura del framebuffer.
    pub fn verificar(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, model: &Model) {
        use cohete::color::Relleno;
        use cohete::obj::Triangle;
        let view = View {
            yaw: 0.0,
            pitch: 0.0,
            distance: 1.0,
        };
        let thrust = ThrustEngine::new();
        self.actualizar(&view, &thrust, 0.0);
        let cases = [
            (
                "cruza_near",
                [
                    Vector3::new(-0.01, -0.01, 1.05),
                    Vector3::new(0.3, -0.2, 0.0),
                    Vector3::new(-0.3, 0.3, 0.0),
                ],
                true,
            ),
            (
                "fuera_near",
                [
                    Vector3::new(-0.1, -0.1, 1.05),
                    Vector3::new(0.1, -0.1, 1.05),
                    Vector3::new(0.0, 0.1, 1.05),
                ],
                false,
            ),
            (
                "cruza_lateral",
                [
                    Vector3::new(-0.2, -0.2, 0.0),
                    Vector3::new(10.0, -0.2, 0.0),
                    Vector3::new(0.0, 0.3, 0.0),
                ],
                true,
            ),
            (
                "fuera_far",
                [
                    Vector3::new(-0.1, -0.1, -FAR - 1.0),
                    Vector3::new(0.1, -0.1, -FAR - 1.0),
                    Vector3::new(0.0, 0.1, -FAR - 1.0),
                ],
                false,
            ),
        ];
        for (name, vertices, visible) in cases {
            let test_model = Model {
                triangles: vec![Triangle {
                    vertices,
                    uvs: [Vector2::zero(); 3],
                    relleno: Relleno::Solido(COLOR_SELECCION),
                }],
                center: Vector3::zero(),
                size: 1.0,
            };
            {
                let mut d = rl.begin_drawing(thread);
                d.clear_background(Color::BLACK);
                self.dibujar(&mut d, &test_model, &view, None, &thrust, None);
                // Leer antes del intercambio de buffers, despues de vaciar el batch.
                unsafe {
                    raylib::ffi::rlDrawRenderBatchActive();
                }
                let image = unsafe { Image::from_raw(raylib::ffi::LoadImageFromScreen()) };
                let pixels = image.get_image_data();
                let count = pixels.iter().filter(|c| c.r > 100 && c.b > 100).count();
                assert_eq!(count > 0, visible, "Clipping {name}: {count} pixeles");
                image.export_image(&format!("verificacion_{name}.png"));
                println!("PASS {name}: {count} pixeles");
            }
        }
        // Capturas de la escena completa, fuego y laser.
        let view = View {
            yaw: 0.8,
            pitch: 0.4,
            distance: model.size.max(1.0) * 2.0,
        };
        let mut thrust = ThrustEngine::new();
        thrust.intensity = 1.0;
        if let Some((p, dir, r)) = boquilla(model) {
            for _ in 0..60 {
                thrust.update(1.0 / 60.0, p, dir, r);
            }
        }
        let mut laser = Laser::new();
        laser.update(1.0, 1.0);
        for (name, beam) in [("fuego", None), ("laser", Some(&laser))] {
            self.actualizar(&view, &thrust, 1.0);
            let mut d = rl.begin_drawing(thread);
            d.clear_background(Color::BLACK);
            self.dibujar_fondo(&mut d);
            self.dibujar(&mut d, model, &view, None, &thrust, beam);
            unsafe {
                raylib::ffi::rlDrawRenderBatchActive();
            }
            let image = unsafe { Image::from_raw(raylib::ffi::LoadImageFromScreen()) };
            image.export_image(&format!("verificacion_{name}.png"));
        }
        println!("PASS verificacion GPU completa");
    }
}
