use cohete::color::mezclar;
use raylib::prelude::*;
use std::sync::Mutex;

pub struct Framebuffer {
    pub width: i32,
    pixels: Vec<u8>,
    zbuffer: Vec<f32>,
    background_color: Color,
    texture: Texture2D,
}

// Franja horizontal del framebuffer. Cada hilo trabaja sobre una franja distinta,
// por lo que no hay datos compartidos ni bloqueos al dibujar.
pub struct Band<'a> {
    pub width: i32,
    pub y0: i32,
    pub y1: i32,
    pixels: &'a mut [u8],
    zbuffer: &'a mut [f32],
    current_color: Color,
}

impl Band<'_> {
    fn limpiar(&mut self, fondo: [u8; 4]) {
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&fondo);
        }
        self.zbuffer.fill(f32::INFINITY);
    }

    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    // Mezcla alfa entera para partículas translúcidas (sin escribir profundidad).
    pub fn blend(&mut self, x: i32, y: i32, depth: f32, rgb: [u8; 3], alpha: u32) {
        if x < 0 || x >= self.width || y < self.y0 || y >= self.y1 {
            return;
        }
        let index = ((y - self.y0) * self.width + x) as usize;
        if depth >= self.zbuffer[index] {
            return;
        }
        let pixel = &mut self.pixels[index * 4..index * 4 + 4];
        let inv = 255 - alpha;
        for i in 0..3 {
            pixel[i] = ((rgb[i] as u32 * alpha + pixel[i] as u32 * inv + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }

    // Pinta un píxel con el color actual. Si es opaco, escribe el z-buffer; si es translúcido,
    // se mezcla con lo ya dibujado y no escribe profundidad (se dibuja después de los opacos).
    pub fn point(&mut self, x: i32, y: i32, depth: f32) {
        if x < 0 || x >= self.width || y < self.y0 || y >= self.y1 {
            return;
        }
        let c = self.current_color;
        if c.a == 0 {
            return;
        }
        let index = ((y - self.y0) * self.width + x) as usize;
        if depth >= self.zbuffer[index] {
            return;
        }
        let pixel = &mut self.pixels[index * 4..index * 4 + 4];
        if c.a == 255 {
            self.zbuffer[index] = depth;
            pixel.copy_from_slice(&[c.r, c.g, c.b, 255]);
        } else {
            let destino = [pixel[0], pixel[1], pixel[2], pixel[3]];
            pixel.copy_from_slice(&mezclar(c, destino));
        }
    }
}

const FILAS_POR_FRANJA: usize = 16;

impl Framebuffer {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread, width: i32, height: i32) -> Self {
        let image = Image::gen_image_color(width, height, Color::BLACK);
        let texture = rl
            .load_texture_from_image(thread, &image)
            .expect("No se pudo crear la textura del framebuffer");
        let size = (width * height) as usize;

        Framebuffer {
            width,
            pixels: vec![0; size * 4],
            zbuffer: vec![f32::INFINITY; size],
            background_color: Color::BLACK,
            texture,
        }
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    // Reparte el framebuffer en franjas y las dibuja en paralelo (std::thread, sin librerías externas).
    // Cada franja se limpia antes de dibujar. Los hilos toman franjas de una cola compartida
    // para equilibrar la carga.
    pub fn render_parallel<F>(&mut self, dibujar: F)
    where
        F: Fn(&mut Band) + Sync,
    {
        let fondo = [self.background_color.r, self.background_color.g, self.background_color.b, 255];
        let ancho = self.width as usize;
        let franjas: Vec<Band> = self
            .pixels
            .chunks_mut(ancho * 4 * FILAS_POR_FRANJA)
            .zip(self.zbuffer.chunks_mut(ancho * FILAS_POR_FRANJA))
            .enumerate()
            .map(|(i, (pixels, zbuffer))| {
                let y0 = (i * FILAS_POR_FRANJA) as i32;
                Band {
                    width: self.width,
                    y0,
                    y1: y0 + (zbuffer.len() / ancho) as i32,
                    pixels,
                    zbuffer,
                    current_color: Color::WHITE,
                }
            })
            .collect();

        let hilos = std::thread::available_parallelism().map_or(4, |n| n.get()).min(franjas.len());
        let cola = Mutex::new(franjas);
        let dibujar = &dibujar;
        let cola = &cola;

        std::thread::scope(|s| {
            for _ in 0..hilos {
                s.spawn(move || loop {
                    let franja = cola.lock().unwrap().pop();
                    match franja {
                        Some(mut franja) => {
                            franja.limpiar(fondo);
                            dibujar(&mut franja)
                        }
                        None => break,
                    }
                });
            }
        });
    }

    pub fn render_buffer(&mut self, d: &mut RaylibDrawHandle) {
        self.texture
            .update_texture(&self.pixels)
            .expect("No se pudo actualizar la textura del framebuffer");
        d.draw_texture(&self.texture, 0, 0, Color::WHITE);
    }
}
