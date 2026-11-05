//! Velocimetro digital dibujado pixel a pixel en CPU; no usa fuentes ni dibujo raylib.
use crate::framebuffer::Band;
use crate::vuelo::Nave;
use raylib::prelude::Color;

pub struct Datos {
    speed: String,
    power: String,
    motor: &'static str,
    status: &'static str,
    ratio: f32,
    laser: bool,
    active: bool,
    pub landing_status: String,
    pub landing_progress: f32,
}
impl Datos {
    pub fn new(nave: &Nave, power: f32, active: bool) -> Self {
        let n = nave.speed_kmh.clamp(0., Nave::MAX_SPEED_KMH).round() as u32;
        let speed = if n >= 1000 {
            format!("{},{:03}", n / 1000, n % 1000)
        } else {
            n.to_string()
        };
        Self {
            speed,
            power: format!("POTENCIA {}%", (power * 100.).round() as u32),
            motor: if !active {
                "MOTOR APAGADO"
            } else if nave.laser_mode() {
                "MOTOR LASER"
            } else {
                "MOTOR NORMAL"
            },
            status: if active && nave.speed_kmh >= Nave::MAX_SPEED_KMH {
                "LIMITE"
            } else if active {
                "ACELERANDO"
            } else if nave.speed_kmh > 0. {
                "FRENANDO"
            } else {
                "DETENIDA"
            },
            ratio: nave.speed_kmh / Nave::MAX_SPEED_KMH,
            laser: nave.laser_mode(),
            active,
            landing_status: String::new(),
            landing_progress: 0.0,
        }
    }
}
fn rect(b: &mut Band, x: i32, y: i32, w: i32, h: i32, c: Color) {
    for py in y.max(b.y0)..(y + h).min(b.y1) {
        for px in x.max(0)..(x + w).min(b.width) {
            b.background_pixel(px, py, c);
        }
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 2, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        '%' => [25, 25, 2, 4, 8, 19, 19],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        ',' => [0, 0, 0, 0, 4, 4, 8],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}
fn text(b: &mut Band, x: i32, y: i32, s: &str, scale: i32, c: Color) {
    for (i, ch) in s.chars().enumerate() {
        for (row, mask) in glyph(ch).iter().enumerate() {
            for col in 0..5 {
                if mask & (1 << (4 - col)) != 0 {
                    rect(
                        b,
                        x + i as i32 * 6 * scale + col * scale,
                        y + row as i32 * scale,
                        scale,
                        scale,
                        c,
                    );
                }
            }
        }
    }
}
pub fn dibujar(b: &mut Band, hud: &Datos, height: i32) {
    dibujar_aterrizaje(b, hud, height);
    let (x, y, w, h) = (18, height - 174, 360, 156);
    if y + h <= b.y0 || y >= b.y1 {
        return;
    }
    let edge = Color::new(48, 94, 118, 255);
    let muted = Color::new(142, 164, 178, 255);
    let color = if !hud.active {
        Color::new(170, 190, 200, 255)
    } else if hud.laser {
        Color::new(70, 225, 255, 255)
    } else {
        Color::new(255, 165, 66, 255)
    };
    rect(b, x, y, w, h, Color::new(6, 14, 23, 255));
    rect(b, x, y, w, 1, edge);
    rect(b, x, y + h - 1, w, 1, edge);
    rect(b, x, y, 1, h, edge);
    rect(b, x + w - 1, y, 1, h, edge);
    text(b, x + 14, y + 10, "VELOCIDAD", 1, muted);
    text(b, x + 14, y + 27, &hud.speed, 4, Color::WHITE);
    text(b, x + 190, y + 38, "KM/H", 2, muted);
    text(b, x + 14, y + 65, &hud.power, 1, color);
    text(b, x + 196, y + 65, hud.status, 1, muted);
    text(b, x + 14, y + 82, hud.motor, 1, color);
    rect(b, x + 14, y + 101, w - 28, 8, Color::new(22, 38, 52, 255));
    rect(
        b,
        x + 14,
        y + 101,
        ((w - 28) as f32 * hud.ratio.clamp(0., 1.)) as i32,
        8,
        color,
    );
    let threshold =
        x + 14 + ((w - 28) as f32 * Nave::LASER_THRESHOLD_KMH / Nave::MAX_SPEED_KMH) as i32;
    rect(b, threshold, y + 98, 1, 14, Color::new(255, 190, 80, 255));
    text(b, x + 14, y + 116, "0", 1, muted);
    text(b, x + w - 74, y + 116, "28,000", 1, muted);
    text(
        b,
        x + 14,
        y + 137,
        "1/2/3: POTENCIA   ESPACIO: ACELERAR",
        1,
        muted,
    );
}

fn dibujar_aterrizaje(b: &mut Band, hud: &Datos, height: i32) {
    if hud.landing_status.is_empty() {
        return;
    }
    let (x, y, w) = (18, height - 174 - 44, 360);
    let color = if hud.landing_status.starts_with("COLISION") {
        Color::new(255, 80, 80, 255)
    } else if hud.landing_status.starts_with("ATERRIZADO") {
        Color::new(80, 255, 130, 255)
    } else {
        Color::new(255, 200, 60, 255)
    };
    rect(b, x, y, w, 34, Color::new(6, 14, 23, 255));
    text(b, x + 14, y + 6, &hud.landing_status, 1, color);
    rect(b, x + 14, y + 22, w - 28, 6, Color::new(22, 38, 52, 255));
    rect(
        b,
        x + 14,
        y + 22,
        ((w - 28) as f32 * hud.landing_progress.clamp(0., 1.)) as i32,
        6,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speed_format_and_motor_status() {
        let mut ship = Nave::new();
        ship.speed_kmh = 28_000.;
        let active = Datos::new(&ship, 0.6, true);
        assert_eq!(active.speed, "28,000");
        assert_eq!(active.motor, "MOTOR LASER");
        let off = Datos::new(&ship, 0.6, false);
        assert_eq!(off.motor, "MOTOR APAGADO");
        assert_eq!(off.status, "FRENANDO");
    }
}
