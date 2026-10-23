use crate::color::mezclar;
use raylib::prelude::*;

pub struct Framebuffer {
    pub width: i32,
    pub height: i32,
    pixels: Vec<u8>,
    zbuffer: Vec<f32>,
    background_color: Color,
    current_color: Color,
    texture: Texture2D,
}

impl Framebuffer {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread, width: i32, height: i32) -> Self {
        let image = Image::gen_image_color(width, height, Color::BLACK);
        let texture = rl
            .load_texture_from_image(thread, &image)
            .expect("No se pudo crear la textura del framebuffer");
        let size = (width * height) as usize;

        Framebuffer {
            width,
            height,
            pixels: vec![0; size * 4],
            zbuffer: vec![f32::INFINITY; size],
            background_color: Color::BLACK,
            current_color: Color::WHITE,
            texture,
        }
    }

    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    pub fn clear(&mut self) {
        let bg = self.background_color;
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&[bg.r, bg.g, bg.b, 255]);
        }
        self.zbuffer.fill(f32::INFINITY);
    }

    // Pinta un píxel con el color actual. Si es opaco, escribe el z-buffer; si es translúcido,
    // se mezcla con lo ya dibujado y no escribe profundidad (se dibuja después de los opacos).
    pub fn point(&mut self, x: i32, y: i32, depth: f32) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let c = self.current_color;
        if c.a == 0 {
            return;
        }
        let index = (y * self.width + x) as usize;
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

    pub fn render_buffer(&mut self, d: &mut RaylibDrawHandle) {
        self.texture
            .update_texture(&self.pixels)
            .expect("No se pudo actualizar la textura del framebuffer");
        d.draw_texture(&self.texture, 0, 0, Color::WHITE);
    }
}
