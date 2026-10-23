use raylib::prelude::*;

/// Fondo espacial procedural. No necesita imagenes ni bibliotecas adicionales.
/// La camara actual mira siempre en la misma direccion: solo rota el cohete.
pub struct Estrellas {
    shader: Shader,
    viewport: i32,
    screen_size: i32,
}

// GLSL incluido en Rust para mantener esta implementacion en un solo .rs.
const FRAGMENT_SHADER: &str = r#"
#version 330
uniform vec2 viewport;
uniform vec2 screenSize;
out vec4 finalColor;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

vec3 stars(vec2 pixel, float cellSize, float density, float seed) {
    vec2 footprint = fwidth(pixel);
    float aa = max(max(footprint.x, footprint.y), 0.25);
    vec2 cell = floor(pixel / cellSize);
    vec2 local = pixel - cell * cellSize;
    float n = hash(cell + vec2(seed));
    if (n > density) return vec3(0.0);
    // Margen suficiente para que el halo no se corte en los bordes de celda.
    vec2 center = mix(vec2(8.0), vec2(cellSize - 8.0), vec2(
        hash(cell + vec2(seed + 7.3, 19.1)),
        hash(cell + vec2(37.7, seed + 2.9))));
    float radius = mix(0.45, 1.4, hash(cell + vec2(seed + 8.7)));
    float brightness = mix(0.35, 1.0, hash(cell + vec2(seed + 21.3)));
    float distanceToStar = length(local - center);
    float core = 1.0 - smoothstep(max(0.0, radius - aa), radius + aa, distanceToStar);
    float halo = exp(-distanceToStar * distanceToStar / (radius * radius * 7.0)) * 0.08;
    float tint = hash(cell + vec2(seed + 49.0));
    vec3 color = mix(vec3(0.72, 0.82, 1.0), vec3(1.0, 0.91, 0.77), tint);
    return color * brightness * (core + halo);
}

void main() {
    // Coordenadas logicas: mismo aspecto con distintos factores de DPI.
    vec2 pixel = vec2(gl_FragCoord.x, viewport.y - gl_FragCoord.y) * screenSize / viewport;
    vec3 color = stars(pixel, 32.0, 0.12, 11.0)
               + stars(pixel, 64.0, 0.14, 43.0)
               + stars(pixel, 120.0, 0.08, 79.0);
    finalColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
"#;

impl Estrellas {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let shader = rl.load_shader_from_memory(thread, None, Some(FRAGMENT_SHADER));
        assert!(
            shader.id != unsafe { raylib::ffi::rlGetShaderIdDefault() },
            "No se pudo compilar el shader del fondo de estrellas"
        );
        let viewport = shader.get_shader_location("viewport");
        let screen_size = shader.get_shader_location("screenSize");
        assert!(viewport >= 0 && screen_size >= 0);
        Self {
            shader,
            viewport,
            screen_size,
        }
    }

    /// Llamar en modo 2D, antes de dibujar la escena 3D y la interfaz.
    pub fn dibujar(&mut self, d: &mut RaylibDrawHandle) {
        let (width, height, render_width, render_height) = unsafe {
            (
                raylib::ffi::GetScreenWidth(),
                raylib::ffi::GetScreenHeight(),
                raylib::ffi::GetRenderWidth(),
                raylib::ffi::GetRenderHeight(),
            )
        };
        if width <= 0 || height <= 0 || render_width <= 0 || render_height <= 0 {
            return;
        }
        self.shader.set_shader_value(
            self.viewport,
            Vector2::new(render_width as f32, render_height as f32),
        );
        self.shader
            .set_shader_value(self.screen_size, Vector2::new(width as f32, height as f32));
        // El fondo no debe ocultar al cohete ni a sus efectos en el z-buffer.
        unsafe {
            raylib::ffi::rlDrawRenderBatchActive();
            raylib::ffi::rlDisableDepthTest();
            raylib::ffi::rlDisableDepthMask();
        }
        {
            let mut sky = d.begin_shader_mode(&mut self.shader);
            sky.draw_rectangle(0, 0, width, height, Color::WHITE);
        }
        unsafe {
            raylib::ffi::rlDrawRenderBatchActive();
            raylib::ffi::rlEnableDepthMask();
        }
        // BeginMode3D habilitara la prueba de profundidad para la escena.
    }
}
