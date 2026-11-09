use cohete::color::mezclar;
use raylib::prelude::*;
use std::sync::Mutex;

pub struct Framebuffer {
    pub width: i32,
    pub height: i32,
    pixels: Vec<u8>,
    zbuffer: Vec<f32>,
    background_color: Color,
    texture: Texture2D,
}

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

    pub fn background_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || x >= self.width || y < self.y0 || y >= self.y1 {
            return;
        }
        let index = ((y - self.y0) * self.width + x) as usize * 4;
        self.pixels[index..index + 4].copy_from_slice(&[color.r, color.g, color.b, 255]);
    }

    pub fn overlay_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || x >= self.width || y < self.y0 || y >= self.y1 || color.a == 0 {
            return;
        }
        let index = ((y - self.y0) * self.width + x) as usize * 4;
        let pixel = &mut self.pixels[index..index + 4];
        if color.a == 255 {
            pixel.copy_from_slice(&[color.r, color.g, color.b, 255]);
            return;
        }
        let alpha = color.a as u32;
        let inv = 255 - alpha;
        let rgb = [color.r, color.g, color.b];
        for i in 0..3 {
            pixel[i] = ((rgb[i] as u32 * alpha + pixel[i] as u32 * inv + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }

    /// Filtro rojizo de toda la franja: pasa la imagen a monocromo y la tiñe de rojo.
    pub fn filtro_rojizo(&mut self) {
        for px in self.pixels.chunks_exact_mut(4) {
            let luz = (px[0] as u32 * 77 + px[1] as u32 * 150 + px[2] as u32 * 29) >> 8;
            let r = (px[0] as u32 + luz * 2 + 60) / 2;
            px[0] = ((px[0] as u32 * 3 + r * 5) / 8).min(255) as u8;
            px[1] = ((px[1] as u32 * 2 + luz * 2 / 3) / 8) as u8;
            px[2] = ((px[2] as u32 * 2 + luz * 2 / 3) / 8) as u8;
        }
    }

    pub fn visible(&self, x: i32, y: i32, depth: f32) -> bool {
        if x < 0 || x >= self.width || y < self.y0 || y >= self.y1 || !depth.is_finite() {
            return false;
        }
        let index = ((y - self.y0) * self.width + x) as usize;
        depth < self.zbuffer[index]
    }

    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

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
/// Alto en filas de cada franja; el renderizador lo usa para repartir triangulos por franja.
pub const ALTO_FRANJA: i32 = FILAS_POR_FRANJA as i32;

impl Framebuffer {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread, width: i32, height: i32) -> Self {
        let image = Image::gen_image_color(width, height, Color::BLACK);
        let texture = rl
            .load_texture_from_image(thread, &image)
            .expect("No se pudo crear la textura del framebuffer");
        texture.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        let size = (width * height) as usize;

        Framebuffer {
            width,
            height,
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
        let fondo = [
            self.background_color.r,
            self.background_color.g,
            self.background_color.b,
            255,
        ];
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

        let hilos = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(franjas.len());
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
        let destino = rect_destino(self.width, self.height, d.get_screen_width(), d.get_screen_height());
        d.clear_background(Color::BLACK);
        d.draw_texture_pro(
            &self.texture,
            Rectangle::new(0., 0., self.width as f32, self.height as f32),
            destino,
            Vector2::zero(),
            0.,
            Color::WHITE,
        );
    }

    /// Convierte un punto de la ventana a coordenadas del framebuffer; `None` si cae en las
    /// barras negras.
    pub fn a_framebuffer(&self, punto: Vector2, ventana: (i32, i32)) -> Option<Vector2> {
        let r = rect_destino(self.width, self.height, ventana.0, ventana.1);
        if punto.x < r.x || punto.y < r.y || punto.x >= r.x + r.width || punto.y >= r.y + r.height {
            return None;
        }
        Some(Vector2::new(
            (punto.x - r.x) * self.width as f32 / r.width,
            (punto.y - r.y) * self.height as f32 / r.height,
        ))
    }
}

/// Mayor rectangulo centrado dentro de la ventana que conserva la proporcion del framebuffer.
fn rect_destino(ancho: i32, alto: i32, ventana_ancho: i32, ventana_alto: i32) -> Rectangle {
    let escala = (ventana_ancho as f32 / ancho as f32).min(ventana_alto as f32 / alto as f32);
    let (w, h) = (ancho as f32 * escala, alto as f32 * escala);
    Rectangle::new(
        (ventana_ancho as f32 - w) * 0.5,
        (ventana_alto as f32 - h) * 0.5,
        w,
        h,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destination_keeps_aspect_and_centers() {
        let r = rect_destino(1200, 800, 1920, 1080);
        assert!((r.width / r.height - 1.5).abs() < 1e-4);
        assert!((r.height - 1080.).abs() < 1e-3);
        assert!((r.x * 2. + r.width - 1920.).abs() < 1e-3);
        let exacto = rect_destino(1200, 800, 1200, 800);
        assert_eq!((exacto.x, exacto.y, exacto.width, exacto.height), (0., 0., 1200., 800.));
    }

    #[test]
    fn opaque_depth_and_background() {
        let mut pixels = vec![0; 4];
        let mut z = vec![f32::INFINITY];
        let mut b = Band {
            width: 1,
            y0: 0,
            y1: 1,
            pixels: &mut pixels,
            zbuffer: &mut z,
            current_color: Color::WHITE,
        };
        b.background_pixel(0, 0, Color::WHITE);
        assert!(b.visible(0, 0, 100.)); // El cielo no escribe profundidad.
        b.set_current_color(Color::new(255, 0, 0, 255));
        b.point(0, 0, 2.);
        b.set_current_color(Color::new(0, 0, 255, 255));
        b.point(0, 0, 3.);
        assert_eq!(&b.pixels[..4], &[255, 0, 0, 255]);
        b.point(0, 0, 1.);
        assert_eq!(&b.pixels[..4], &[0, 0, 255, 255]);
        assert_eq!(b.zbuffer[0], 1.);
    }
    #[test]
    fn transparency_tests_depth_without_writing_it() {
        let mut pixels = vec![0, 0, 0, 255];
        let mut z = vec![2.];
        let mut b = Band {
            width: 1,
            y0: 0,
            y1: 1,
            pixels: &mut pixels,
            zbuffer: &mut z,
            current_color: Color::new(255, 0, 0, 128),
        };
        b.point(0, 0, 3.);
        assert_eq!(b.pixels[0], 0);
        b.point(0, 0, 1.);
        assert!(b.pixels[0] > 0);
        assert_eq!(b.zbuffer[0], 2.);
    }
}
