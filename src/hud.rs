
use crate::framebuffer::Band;
use crate::planeta::SistemaPlanetario;
use crate::vuelo::Nave;
use raylib::prelude::{Color, Vector3};
use std::f32::consts::{PI, TAU};

const BLANCO: Color = Color::new(240, 244, 250, 255);
const APAGADO: Color = Color::new(142, 164, 178, 255);
const FONDO: Color = Color::new(14, 22, 38, 205);
const BORDE: Color = Color::new(120, 150, 180, 150);
const NARANJA: Color = Color::new(255, 120, 50, 255);
const CIAN: Color = Color::new(70, 225, 255, 255);
const VERDE: Color = Color::new(70, 255, 120, 255);
const AMARILLO: Color = Color::new(255, 232, 60, 255);
const ROJO: Color = Color::new(235, 60, 50, 255);

const SEGMENTOS_BARRA: i32 = 44;
const PX_POR_PASO: f32 = 30.;

struct Cercano {
    nombre: &'static str,
    distancia: String,
    color: Color,
}

pub struct Datos {
    speed: String,
    power: String,
    motor: &'static str,
    status: &'static str,
    ratio: f32,
    laser: bool,
    active: bool,
    potencia: f32,
    pub estado_vuelo: &'static str,
    pub posada: bool,
    tiempo: String,
    odometro: String,
    cercanos: Vec<Cercano>,
    altitud: f32,
    unidad: &'static str,
    cuerpo: &'static str,
    actitud: (f32, f32, f32),
    nave_mapa: Option<NaveMapa>,
    cuerpos_mapa: Vec<MarcaMapa>,
    orbitas_mapa: Vec<f32>,
}


const LADO_MAPA: i32 = 220;
const RADIO_MAPA: f32 = 104.;
const ESTRELLAS_MAPA: u32 = 70;

struct MarcaMapa {
    x: f32,
    y: f32,
    color: Color,
    radio: f32,
    cercano: bool,
}

struct NaveMapa {
    x: f32,
    y: f32,
    dir: Option<(f32, f32)>,
}

fn proyectar_mapa(p: (f32, f32), alcance: f32) -> (f32, f32) {
    let d = (p.0 * p.0 + p.1 * p.1).sqrt();
    if d < 1e-3 {
        return (0., 0.);
    }
    let r = (d / alcance).min(1.).sqrt();
    (p.0 / d * r, p.1 / d * r)
}


fn direccion_en_mapa(pos: (f32, f32), v: (f32, f32), alcance: f32) -> Option<(f32, f32)> {
    let n = (v.0 * v.0 + v.1 * v.1).sqrt();
    if n < 1e-6 {
        return None;
    }
    let (vx, vy) = (v.0 / n, v.1 / n);
    let d = (pos.0 * pos.0 + pos.1 * pos.1).sqrt();
    if d < 1e-3 {
        return Some((vx, vy));
    }
    let dc = d.min(alcance);
    let u = (pos.0 / d, pos.1 / d);
    let t = (-u.1, u.0);
    let (vr, vt) = (vx * u.0 + vy * u.1, vx * t.0 + vy * t.1);
    let radial = 0.5 / (dc * alcance).sqrt();
    let tangencial = (dc / alcance).sqrt() / dc;
    let m = (
        u.0 * radial * vr + t.0 * tangencial * vt,
        u.1 * radial * vr + t.1 * tangencial * vt,
    );
    let l = (m.0 * m.0 + m.1 * m.1).sqrt();
    (l > 1e-9).then(|| (m.0 / l, m.1 / l))
}

fn hash01(n: u32) -> f32 {
    let mut h = n.wrapping_mul(0x9E37_79B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.
}

fn miles(n: u32) -> String {
    if n >= 1000 {
        format!("{},{:03}", n / 1000, n % 1000)
    } else {
        n.to_string()
    }
}

fn tiempo_texto(segundos: f32) -> String {
    let ms = (segundos.max(0.) * 1000.).round() as u64;
    format!("{}:{:02}.{:03}", ms / 60_000, (ms / 1000) % 60, ms % 1000)
}

fn odometro_texto(km: f64) -> String {
    let decimas = (km.max(0.) * 10.).round() as u64;
    let enteros = decimas / 10;
    format!("{:03},{:03}.{}", (enteros / 1000) % 1000, enteros % 1000, decimas % 10)
}

// Metros por encima de 10,000 se muestran en kilometros.
fn medida(metros: f32) -> (f32, &'static str) {
    if metros >= 10_000. {
        (metros / 1000., "KM")
    } else {
        (metros, "M")
    }
}

fn distancia_texto(metros: f32) -> String {
    let (valor, unidad) = medida(metros);
    format!("{} {}", miles(valor.round() as u32), unidad)
}

// Separacion entre marcas de la cinta de altitud: 1, 2, 20, 200... segun la magnitud.
fn paso_cinta(valor: f32) -> f32 {
    let base = 10f32.powf(valor.max(1.).log10().floor());
    (base / 5.).max(1.)
}

// Altura en pixeles sobre la linea del horizonte para un pixel (dx, dy) del instrumento (dx a la derecha, dy hacia arriba). `k` son pixeles por radian de cabeceo.
fn sobre_horizonte(dx: f32, dy: f32, actitud: (f32, f32, f32), k: f32) -> f32 {
    let (sp, sr, cu) = actitud;
    let n = (sr * sr + cu * cu).sqrt();
    let (sx, sy) = if n > 1e-4 { (sr / n, cu / n) } else { (0., 1.) };
    dx * sx + dy * sy + sp.clamp(-1., 1.).asin() * k
}

impl Datos {
    pub fn new(nave: &Nave, power: f32, active: bool) -> Self {
        let n = nave.speed_kmh.clamp(0., Nave::MAX_SPEED_KMH).round() as u32;
        let speed = miles(n);
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
            potencia: power,
            estado_vuelo: "VUELO LIBRE",
            posada: false,
            tiempo: tiempo_texto(0.),
            odometro: odometro_texto(0.),
            cercanos: Vec::new(),
            altitud: 0.,
            unidad: "M",
            cuerpo: "",
            actitud: (0., 0., 1.),
            nave_mapa: None,
            cuerpos_mapa: Vec::new(),
            orbitas_mapa: Vec::new(),
        }
    }

    fn actualizar_mapa(&mut self, nave: &Nave, sistema: &SistemaPlanetario) {
        let origen = sistema.sol.centro;
        let plano = |v: Vector3| ((v - origen).x, (v - origen).z);
        let alcance = sistema
            .planetas
            .iter()
            .filter(|p| p.config.padre.is_none())
            .map(|p| p.config.orbita_radio)
            .fold(1., f32::max)
            * 1.08;
        self.orbitas_mapa = sistema
            .planetas
            .iter()
            .filter(|p| p.config.padre.is_none())
            .map(|p| (p.config.orbita_radio / alcance).sqrt())
            .collect();
        let cercano = self.cuerpo;
        self.cuerpos_mapa = sistema
            .planetas
            .iter()
            .map(|p| {
                let (x, y) = match p.config.padre {
                    None => proyectar_mapa(plano(p.centro), alcance),
                    // La luna se dibuja junto a su planeta, en la direccion real de su orbita.
                    Some(i) => {
                        let padre = &sistema.planetas[i];
                        let (px, py) = proyectar_mapa(plano(padre.centro), alcance);
                        let (dx, dz) = (p.centro.x - padre.centro.x, p.centro.z - padre.centro.z);
                        let n = (dx * dx + dz * dz).sqrt().max(1e-3);
                        (px + dx / n * 8. / RADIO_MAPA, py + dz / n * 8. / RADIO_MAPA)
                    }
                };
                MarcaMapa {
                    x,
                    y,
                    color: p.config.aspecto.color(),
                    radio: if p.config.padre.is_some() {
                        1.6
                    } else {
                        2.2 + 0.1 * p.config.radio.sqrt()
                    },
                    cercano: p.config.nombre == cercano,
                }
            })
            .collect();
        let pos = plano(nave.position);
        let (x, y) = proyectar_mapa(pos, alcance);
        let f = nave.forward();
        let dir = if (f.x * f.x + f.z * f.z).sqrt() > 0.15 {
            direccion_en_mapa(pos, (f.x, f.z), alcance)
        } else {
            None
        };
        self.nave_mapa = Some(NaveMapa { x, y, dir });
    }

    /// Datos que dependen del sistema: cuerpos cercanos, radar de altitud, horizonte y mapa.
    /// `holgura` es la distancia centro-nave a suelo al estar posada (altitud 0).
    pub fn con_sistema(
        &mut self,
        nave: &Nave,
        sistema: &SistemaPlanetario,
        holgura: f32,
        tiempo: f32,
        recorrido_km: f64,
    ) {
        self.tiempo = tiempo_texto(tiempo);
        self.odometro = odometro_texto(recorrido_km);
        // Cada cuerpo (el sol incluido): nombre, color, altitud en metros y direccion desde su centro.
        let mut medidas: Vec<(&'static str, Color, f32, Vector3)> = sistema
            .planetas
            .iter()
            .map(|p| {
                let rel = nave.position - p.centro;
                let distancia = rel.length().max(1e-3);
                let dir = rel / distancia;
                let superficie = p.radio_superficie(p.a_local(dir));
                let altitud = (distancia - superficie - holgura).max(0.);
                (
                    p.config.nombre,
                    p.config.aspecto.color(),
                    altitud * Nave::METERS_PER_UNIT,
                    dir,
                )
            })
            .collect();
        let rel_sol = nave.position - sistema.sol.centro;
        let distancia_sol = rel_sol.length().max(1e-3);
        medidas.push((
            "SOL",
            Color::new(255, 200, 60, 255),
            (distancia_sol - sistema.sol.radio - holgura).max(0.) * Nave::METERS_PER_UNIT,
            rel_sol / distancia_sol,
        ));
        medidas.sort_by(|a, b| a.2.total_cmp(&b.2));
        self.cercanos = medidas
            .iter()
            .take(5)
            .map(|&(nombre, color, metros, _)| Cercano {
                nombre,
                distancia: distancia_texto(metros),
                color,
            })
            .collect();
        if let Some(&(nombre, _, metros, dir)) = medidas.first() {
            let (valor, unidad) = medida(metros);
            self.altitud = valor;
            self.unidad = unidad;
            self.cuerpo = nombre;
            self.actitud = (nave.forward().dot(dir), nave.right().dot(dir), nave.up().dot(dir));
        }
        self.actualizar_mapa(nave, sistema);
    }
}

// ---------------------------------------------------------------- primitivas

fn rect(b: &mut Band, x: i32, y: i32, w: i32, h: i32, c: Color) {
    for py in y.max(b.y0)..(y + h).min(b.y1) {
        for px in x.max(0)..(x + w).min(b.width) {
            b.overlay_pixel(px, py, c);
        }
    }
}

fn borde(b: &mut Band, x: i32, y: i32, w: i32, h: i32, c: Color) {
    rect(b, x, y, w, 1, c);
    rect(b, x, y + h - 1, w, 1, c);
    rect(b, x, y, 1, h, c);
    rect(b, x + w - 1, y, 1, h, c);
}

fn linea(b: &mut Band, x0: f32, y0: f32, x1: f32, y1: f32, grosor: i32, c: Color) {
    let (min_y, max_y) = (y0.min(y1) as i32 - grosor, y0.max(y1) as i32 + grosor);
    if max_y < b.y0 || min_y >= b.y1 {
        return;
    }
    let pasos = (x1 - x0).abs().max((y1 - y0).abs()).ceil().max(1.) as i32;
    for k in 0..=pasos {
        let t = k as f32 / pasos as f32;
        let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
        rect(
            b,
            x.round() as i32 - grosor / 2,
            y.round() as i32 - grosor / 2,
            grosor,
            grosor,
            c,
        );
    }
}

fn disco(b: &mut Band, cx: f32, cy: f32, r: f32, c: Color) {
    let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
    for y in y0.max(b.y0)..=y1.min(b.y1 - 1) {
        for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy <= r * r {
                b.overlay_pixel(x, y, c);
            }
        }
    }
}

fn anillo(b: &mut Band, cx: f32, cy: f32, r_ext: f32, r_int: f32, c: Color) {
    let (y0, y1) = ((cy - r_ext).floor() as i32, (cy + r_ext).ceil() as i32);
    for y in y0.max(b.y0)..=y1.min(b.y1 - 1) {
        for x in (cx - r_ext).floor() as i32..=(cx + r_ext).ceil() as i32 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d2 = dx * dx + dy * dy;
            if d2 <= r_ext * r_ext && d2 >= r_int * r_int {
                b.overlay_pixel(x, y, c);
            }
        }
    }
}

fn triangulo(b: &mut Band, p: [(f32, f32); 3], c: Color) {
    let min_y = p.iter().map(|v| v.1).fold(f32::MAX, f32::min).floor() as i32;
    let max_y = p.iter().map(|v| v.1).fold(f32::MIN, f32::max).ceil() as i32;
    let min_x = p.iter().map(|v| v.0).fold(f32::MAX, f32::min).floor() as i32;
    let max_x = p.iter().map(|v| v.0).fold(f32::MIN, f32::max).ceil() as i32;
    let borde = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| {
        (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
    };
    for y in min_y.max(b.y0)..=max_y.min(b.y1 - 1) {
        for x in min_x..=max_x {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let e = [
                borde(p[0], p[1], fx, fy),
                borde(p[1], p[2], fx, fy),
                borde(p[2], p[0], fx, fy),
            ];
            if e.iter().all(|&v| v >= 0.) || e.iter().all(|&v| v <= 0.) {
                b.overlay_pixel(x, y, c);
            }
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
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}

fn ancho_texto(s: &str, scale: i32) -> i32 {
    (s.chars().count() as i32 * 6 - 1).max(0) * scale
}

fn text(b: &mut Band, x: i32, y: i32, s: &str, scale: i32, c: Color) {
    if y + 7 * scale <= b.y0 || y >= b.y1 {
        return;
    }
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

fn texto_centrado(b: &mut Band, cx: i32, y: i32, s: &str, scale: i32, c: Color) {
    text(b, cx - ancho_texto(s, scale) / 2, y, s, scale, c);
}

fn texto_derecha(b: &mut Band, x_der: i32, y: i32, s: &str, scale: i32, c: Color) {
    text(b, x_der - ancho_texto(s, scale), y, s, scale, c);
}


fn lista_cercanos(b: &mut Band, hud: &Datos, x: i32, y: i32) {
    if b.y1 <= y || b.y0 >= y + 230 {
        return;
    }
    rect(b, x, y, 56, 56, FONDO);
    borde(b, x, y, 56, 56, BORDE);
    if let Some(c) = hud.cercanos.first() {
        disco(b, x as f32 + 28., y as f32 + 28., 17., c.color);
        anillo(b, x as f32 + 28., y as f32 + 28., 20., 18., BLANCO);
        text(b, x + 68, y + 6, "CUERPO MAS CERCANO", 1, APAGADO);
        text(b, x + 68, y + 24, c.nombre, 3, BLANCO);
    }
    for (i, c) in hud.cercanos.iter().enumerate() {
        let fy = y + 68 + i as i32 * 30;
        let primero = i == 0;
        rect(b, x, fy, 30, 28, Color::new(10, 16, 28, 230));
        texto_centrado(b, x + 15, fy + 7, &(i + 1).to_string(), 2, BLANCO);
        let (fondo, tinta) = if primero {
            (Color::new(214, 220, 232, 235), Color::new(16, 24, 40, 255))
        } else {
            (FONDO, BLANCO)
        };
        rect(b, x + 32, fy, 170, 28, fondo);
        text(b, x + 40, fy + 7, c.nombre, 2, tinta);
        texto_derecha(b, x + 194, fy + 10, &c.distancia, 1, tinta);
    }
}

fn temporizador(b: &mut Band, hud: &Datos, cx: i32, y: i32) {
    if b.y1 <= y || b.y0 >= y + 30 {
        return;
    }
    rect(b, cx - 85, y, 170, 30, FONDO);
    borde(b, cx - 85, y, 170, 30, BORDE);
    texto_centrado(b, cx, y + 8, &hud.tiempo, 2, BLANCO);
}

fn dibujar_mapa(b: &mut Band, hud: &Datos, x: i32, y: i32) {
    if b.y1 <= y - 8 || b.y0 >= y + LADO_MAPA + 30 {
        return;
    }
    rect(b, x - 8, y - 8, LADO_MAPA + 16, LADO_MAPA + 16, Color::new(6, 10, 22, 170));
    borde(b, x - 8, y - 8, LADO_MAPA + 16, LADO_MAPA + 16, BORDE);
    let c = LADO_MAPA as f32 * 0.5;
    let (cx, cy) = (x as f32 + c, y as f32 + c);
    for i in 0..ESTRELLAS_MAPA {
        let (angulo, radio) = (hash01(i * 3) * TAU, hash01(i * 3 + 1).sqrt() * (RADIO_MAPA + 4.));
        let brillo = (50. + hash01(i * 3 + 2) * 130.) as u8;
        rect(
            b,
            (cx + angulo.cos() * radio) as i32,
            (cy + angulo.sin() * radio) as i32,
            1,
            1,
            Color::new(220, 230, 255, brillo),
        );
    }
    anillo(b, cx, cy, RADIO_MAPA + 4., RADIO_MAPA + 3., BORDE);
    for &r in &hud.orbitas_mapa {
        anillo(b, cx, cy, r * RADIO_MAPA, r * RADIO_MAPA - 1., Color::new(120, 160, 200, 55));
    }
    disco(b, cx, cy, 8., Color::new(255, 170, 40, 70));
    disco(b, cx, cy, 4.6, Color::new(255, 205, 70, 255));
    for m in &hud.cuerpos_mapa {
        let (px, py) = (cx + m.x * RADIO_MAPA, cy + m.y * RADIO_MAPA);
        disco(b, px, py, m.radio + 1.4, Color::new(8, 12, 22, 230));
        disco(b, px, py, m.radio, m.color);
        if m.cercano {
            anillo(b, px, py, m.radio + 5., m.radio + 3.6, BLANCO);
        }
    }
    if let Some(n) = &hud.nave_mapa {
        let (px, py) = (cx + n.x * RADIO_MAPA, cy + n.y * RADIO_MAPA);
        match n.dir {
            Some((dx, dy)) => {
                let (nx, ny) = (-dy, dx);
                let flecha = |largo: f32, ancho: f32| {
                    [
                        (px + dx * largo, py + dy * largo),
                        (px - dx * largo * 0.6 + nx * ancho, py - dy * largo * 0.6 + ny * ancho),
                        (px - dx * largo * 0.6 - nx * ancho, py - dy * largo * 0.6 - ny * ancho),
                    ]
                };
                triangulo(b, flecha(9., 5.5), BLANCO);
                triangulo(b, flecha(7., 3.8), ROJO);
            }
            None => {
                disco(b, px, py, 5., BLANCO);
                disco(b, px, py, 3.4, ROJO);
            }
        }
    }
    texto_derecha(b, x + LADO_MAPA, y + LADO_MAPA + 14, "MAPA ESTELAR", 1, APAGADO);
}

// Semicirculo de potencia: arco lleno hasta `frac`, marcas en los tres niveles y aguja.
fn medidor(b: &mut Band, cx: i32, cy: i32, r: i32, frac: f32, color: Color) {
    if b.y1 <= cy - r - 10 || b.y0 >= cy + 20 {
        return;
    }
    let apagado = Color::new(40, 52, 72, 255);
    for y in (cy - r - 2).max(b.y0)..=(cy + 1).min(b.y1 - 1) {
        for x in cx - r - 2..=cx + r + 2 {
            let (dx, dy) = ((x - cx) as f32, (cy - y) as f32);
            let d = (dx * dx + dy * dy).sqrt();
            if d > r as f32 + 0.5 || d < r as f32 - 5. {
                continue;
            }
            let f = 1. - dy.max(0.).atan2(dx) / PI;
            b.overlay_pixel(x, y, if f <= frac { color } else { apagado });
        }
    }
    for nivel in [0.3_f32, 0.6, 1.0] {
        let a = PI * (1. - nivel);
        let (ca, sa) = (a.cos(), a.sin());
        let r = r as f32;
        linea(
            b,
            cx as f32 + ca * (r + 2.),
            cy as f32 - sa * (r + 2.),
            cx as f32 + ca * (r + 7.),
            cy as f32 - sa * (r + 7.),
            2,
            BLANCO,
        );
    }
    let a = PI * (1. - frac.clamp(0., 1.));
    linea(
        b,
        cx as f32,
        cy as f32,
        cx as f32 + a.cos() * (r - 8) as f32,
        cy as f32 - a.sin() * (r - 8) as f32,
        2,
        BLANCO,
    );
    disco(b, cx as f32, cy as f32, 3.5, BLANCO);
}

fn indicador(b: &mut Band, cx: i32, cy: i32, etiqueta: &str, encendido: bool, color: Color) {
    if b.y1 <= cy - 16 || b.y0 >= cy + 16 {
        return;
    }
    disco(b, cx as f32, cy as f32, 15., Color::new(14, 22, 38, 215));
    let (aro, tinta) = if encendido {
        (color, color)
    } else {
        (Color::new(64, 76, 98, 255), Color::new(90, 104, 128, 255))
    };
    anillo(b, cx as f32, cy as f32, 15., 13., aro);
    texto_centrado(b, cx, cy - 3, etiqueta, 1, tinta);
}

fn grupo_izquierdo(b: &mut Band, hud: &Datos, altura: i32) {
    if b.y1 <= altura - 130 {
        return;
    }
    let (cx, cy, r) = (84, altura - 62, 42);
    let color = if hud.laser { CIAN } else { NARANJA };
    medidor(b, cx, cy, r, hud.potencia, color);
    texto_centrado(b, cx, cy + 8, &format!("{}%", (hud.potencia * 100.).round() as u32), 1, BLANCO);
    text(b, cx - r - 2, cy + 8, "0", 1, APAGADO);
    texto_derecha(b, cx + r + 4, cy + 8, "MAX", 1, APAGADO);
    text(b, 28, altura - 30, &hud.odometro, 2, BLANCO);
    text(b, 28 + ancho_texto(&hud.odometro, 2) + 8, altura - 24, "KM", 1, APAGADO);
    let activo = hud.active;
    indicador(b, 172, altura - 96, "NRM", activo && !hud.laser, NARANJA);
    indicador(b, 208, altura - 96, "LSR", activo && hud.laser, CIAN);
    indicador(b, 172, altura - 60, "AST", hud.estado_vuelo == "APROXIMANDO", AMARILLO);
    indicador(b, 208, altura - 60, "LND", hud.posada, VERDE);
}

fn velocimetro(b: &mut Band, hud: &Datos, cx: i32, altura: i32) {
    let (w, h) = (440, 88);
    let (x0, y0) = (cx - w / 2, altura - h - 16);
    if b.y1 <= y0 || b.y0 >= altura {
        return;
    }
    rect(b, x0, y0, w, h, Color::new(8, 14, 24, 190));
    borde(b, x0, y0, w, h, BORDE);
    let umbral = (Nave::LASER_THRESHOLD_KMH / Nave::MAX_SPEED_KMH * SEGMENTOS_BARRA as f32) as i32;
    let llenos = (hud.ratio.clamp(0., 1.) * SEGMENTOS_BARRA as f32).ceil() as i32;
    for i in 0..SEGMENTOS_BARRA {
        let color = if i >= llenos {
            Color::new(40, 48, 64, 255)
        } else if i >= umbral {
            CIAN
        } else {
            ROJO
        };
        rect(b, x0 + 14 + i * 9, y0 + 8, 8, 10, color);
    }
    rect(b, x0 + 14 + umbral * 9 - 1, y0 + 5, 1, 16, BLANCO);
    text(b, x0 + 14, y0 + 28, &hud.speed, 5, BLANCO);
    text(b, x0 + 14, y0 + 68, "KM/H", 2, APAGADO);
    rect(b, x0 + 262, y0 + 28, 2, 54, BORDE);
    let (letra, color) = if hud.laser { ("L", CIAN) } else { ("N", NARANJA) };
    text(b, x0 + 280, y0 + 28, letra, 5, color);
    text(b, x0 + 322, y0 + 30, hud.status, 1, APAGADO);
    text(b, x0 + 322, y0 + 46, &hud.power, 1, color);
    text(b, x0 + 280, y0 + 70, hud.motor, 1, color);
    texto_centrado(
        b,
        cx,
        altura - 10,
        "1/2/3 POTENCIA   ESPACIO ACELERAR   W/S A/D ORIENTAR   R REINICIAR",
        1,
        APAGADO,
    );
}

fn horizonte(b: &mut Band, actitud: (f32, f32, f32), cx: i32, cy: i32, r: i32) {
    if b.y1 <= cy - r - 4 || b.y0 >= cy + r + 4 {
        return;
    }
    let k = r as f32 * 1.5;
    let cielo = Color::new(46, 170, 236, 255);
    let suelo = Color::new(150, 92, 42, 255);
    let aro = Color::new(150, 165, 190, 255);
    for y in (cy - r - 3).max(b.y0)..=(cy + r + 3).min(b.y1 - 1) {
        for x in cx - r - 3..=cx + r + 3 {
            let (dx, dy) = ((x - cx) as f32, (cy - y) as f32);
            let d2 = dx * dx + dy * dy;
            if d2 > ((r + 3) * (r + 3)) as f32 {
                continue;
            }
            if d2 > (r * r) as f32 {
                b.overlay_pixel(x, y, aro);
                continue;
            }
            let sobre = sobre_horizonte(dx, dy, actitud, k);
            let (sr, cu) = (actitud.1, actitud.2);
            let n = (sr * sr + cu * cu).sqrt().max(1e-4);
            let along = (dx * cu - dy * sr) / n;
            let mut color = if sobre >= 0. { cielo } else { suelo };
            if sobre.abs() <= 1. {
                color = BLANCO;
            } else {
                for grados in [-20_f32, -10., 10., 20.] {
                    let mitad = if grados.abs() > 15. { 26. } else { 18. };
                    if (sobre - grados.to_radians() * k).abs() <= 0.9 && along.abs() <= mitad {
                        color = BLANCO;
                    }
                }
            }
            b.overlay_pixel(x, y, color);
        }
    }
    rect(b, cx - 34, cy - 1, 22, 3, AMARILLO);
    rect(b, cx + 12, cy - 1, 22, 3, AMARILLO);
    rect(b, cx - 34, cy - 1, 3, 9, AMARILLO);
    rect(b, cx + 31, cy - 1, 3, 9, AMARILLO);
    disco(b, cx as f32, cy as f32, 2.5, AMARILLO);
}

fn cinta_altitud(b: &mut Band, hud: &Datos, x0: i32, y0: i32, w: i32, h: i32) {
    if b.y1 <= y0 - 16 || b.y0 >= y0 + h + 22 {
        return;
    }
    text(b, x0, y0 - 14, "RADAR ALT", 1, VERDE);
    rect(b, x0, y0, w, h, Color::new(46, 54, 76, 225));
    borde(b, x0, y0, w, h, BORDE);
    let cy = y0 + h / 2;
    let valor = hud.altitud;
    let paso = paso_cinta(valor);
    let y_cero = cy as f32 + valor / paso * PX_POR_PASO;
    if y_cero < (y0 + h) as f32 {
        let desde = y_cero.max(y0 as f32) as i32;
        rect(b, x0, desde, 8, y0 + h - desde, ROJO);
    }
    let k0 = ((valor - paso * 3.) / paso).floor() as i32;
    let k1 = ((valor + paso * 3.) / paso).ceil() as i32;
    for k in k0..=k1 {
        let v = k as f32 * paso;
        for (extra, largo) in [(0., 10), (0.5, 5)] {
            let marca = v + extra * paso;
            if marca < 0. {
                continue;
            }
            let y = (cy as f32 - (marca - valor) / paso * PX_POR_PASO).round() as i32;
            if y < y0 + 4 || y > y0 + h - 4 {
                continue;
            }
            rect(b, x0, y, largo, 2, BLANCO);
            if extra == 0. {
                text(b, x0 + 14, y - 3, &miles(marca as u32), 1, BLANCO);
            }
        }
    }
    let caja_w = w - 18;
    rect(b, x0 + 18, cy - 12, caja_w, 24, Color::new(0, 0, 0, 255));
    borde(b, x0 + 18, cy - 12, caja_w, 24, AMARILLO);
    rect(b, x0, cy - 1, 18, 2, AMARILLO);
    let valor_txt = miles(valor.round() as u32);
    let escala = if ancho_texto(&valor_txt, 2) <= caja_w - 8 { 2 } else { 1 };
    texto_centrado(b, x0 + 18 + caja_w / 2, cy - 3 * escala - 1, &valor_txt, escala, AMARILLO);
    text(b, x0, y0 + h + 6, &format!("{} {}", hud.unidad, hud.cuerpo), 1, VERDE);
}

fn instrumentos(b: &mut Band, hud: &Datos, ancho: i32, altura: i32) {
    let (cx, cy, r) = (ancho - 270, altura - 100, 64);
    horizonte(b, hud.actitud, cx, cy, r);
    texto_centrado(b, cx, cy - r - 16, "ACTITUD", 1, VERDE);
    let (sp, sr, cu) = hud.actitud;
    let cabeceo = sp.clamp(-1., 1.).asin().to_degrees().round() as i32;
    let alabeo = (-sr).atan2(cu).to_degrees().round() as i32;
    texto_centrado(b, cx, cy + r + 8, &format!("C{:+} A{:+}", cabeceo, alabeo), 1, VERDE);
    cinta_altitud(b, hud, ancho - 130, altura - 190, 96, 150);
}

pub fn dibujar(b: &mut Band, hud: &Datos, altura: i32) {
    let ancho = b.width;
    lista_cercanos(b, hud, 28, 24);
    temporizador(b, hud, ancho / 2, 22);
    dibujar_mapa(b, hud, ancho - LADO_MAPA - 36, 30);
    grupo_izquierdo(b, hud, altura);
    velocimetro(b, hud, ancho / 2, altura);
    instrumentos(b, hud, ancho, altura);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planeta::SistemaPlanetario;

    #[test]
    fn speed_format_and_motor_status() {
        let mut ship = Nave::new();
        ship.speed_kmh = Nave::MAX_SPEED_KMH;
        let active = Datos::new(&ship, 0.6, true);
        assert_eq!(active.speed, "164,000");
        assert_eq!(active.motor, "MOTOR LASER");
        let off = Datos::new(&ship, 0.6, false);
        assert_eq!(off.motor, "MOTOR APAGADO");
        assert_eq!(off.status, "FRENANDO");
    }

    #[test]
    fn clock_and_odometer_formats() {
        assert_eq!(tiempo_texto(26.666), "0:26.666");
        assert_eq!(tiempo_texto(125.5), "2:05.500");
        assert_eq!(odometro_texto(31_884.1), "031,884.1");
        assert_eq!(odometro_texto(0.), "000,000.0");
    }

    #[test]
    fn distances_switch_to_kilometers() {
        assert_eq!(distancia_texto(850.), "850 M");
        assert_eq!(distancia_texto(15_900.), "16 KM");
        assert_eq!(distancia_texto(1_950_000.), "1,950 KM");
    }

    #[test]
    fn altitude_tape_step_follows_magnitude() {
        let casi = |a: f32, b: f32| (a - b).abs() < 1e-2;
        assert!(casi(paso_cinta(0.), 1.));
        assert!(casi(paso_cinta(7.), 1.));
        assert!(casi(paso_cinta(15.9), 2.));
        assert!(casi(paso_cinta(850.), 20.));
        assert!(casi(paso_cinta(15_000.), 2_000.));
    }

    #[test]
    fn horizon_responds_to_pitch_and_roll() {
        let nivel = (0., 0., 1.);
        assert!(sobre_horizonte(0., 10., nivel, 96.) > 0.);
        assert!(sobre_horizonte(0., -10., nivel, 96.) < 0.);
        // Nariz arriba: el horizonte baja respecto al centro del instrumento.
        assert!(sobre_horizonte(0., 0., (0.5, 0., 0.866), 96.) > 0.);
        assert!(sobre_horizonte(0., 0., (-0.5, 0., 0.866), 96.) < 0.);
        // Alabeo a la derecha: el lado derecho del horizonte sube, el centro queda en cielo
        // y un punto a la derecha del centro queda en suelo.
        let derecha = (0., -0.5, 0.866);
        assert!(sobre_horizonte(40., 0., derecha, 96.) < 0.);
        assert!(sobre_horizonte(-40., 0., derecha, 96.) > 0.);
    }

    #[test]
    fn spawn_reports_terra_as_nearest_body() {
        let sistema = SistemaPlanetario::nuevo();
        let tierra = &sistema.planetas[sistema.indice("TERRA").unwrap()];
        let mut nave = Nave::new();
        nave.position = tierra.centro + Vector3::new(0., 90., 450.);
        let mut datos = Datos::new(&nave, 0.6, false);
        datos.con_sistema(&nave, &sistema, 2.6, 12.5, 100.);
        assert_eq!(datos.cercanos.len(), 5);
        assert_eq!(datos.cercanos[0].nombre, "TERRA");
        assert_eq!(datos.cuerpo, "TERRA");
        assert!(datos.altitud > 0.);
        assert_eq!(datos.tiempo, "0:12.500");
        // Mapa: una marca por cuerpo (la Luna incluida), con el mas cercano resaltado.
        assert_eq!(datos.cuerpos_mapa.len(), sistema.planetas.len());
        assert_eq!(datos.cuerpos_mapa.iter().filter(|m| m.cercano).count(), 1);
        assert!(datos.nave_mapa.is_some());
        assert!(datos.orbitas_mapa.iter().all(|&r| r > 0. && r < 1.));
    }

    #[test]
    fn map_compresses_distance_and_keeps_bearing() {
        let alcance = 1000.;
        let (cerca, lejos) = (proyectar_mapa((100., 0.), alcance), proyectar_mapa((900., 0.), alcance));
        assert!(cerca.0 > 0. && cerca.0 < lejos.0 && lejos.0 < 1.);
        let fuera = proyectar_mapa((0., 5000.), alcance);
        assert!((fuera.1 - 1.).abs() < 1e-5 && fuera.0.abs() < 1e-5);
        assert_eq!(proyectar_mapa((0., 0.), alcance), (0., 0.));
    }

    #[test]
    fn map_heading_follows_radial_and_tangential_motion() {
        let pos = (400., 0.);
        let radial = direccion_en_mapa(pos, (1., 0.), 1000.).unwrap();
        assert!((radial.0 - 1.).abs() < 1e-5 && radial.1.abs() < 1e-5);
        let tangencial = direccion_en_mapa(pos, (0., -1.), 1000.).unwrap();
        assert!(tangencial.0.abs() < 1e-5 && (tangencial.1 + 1.).abs() < 1e-5);
        assert!(direccion_en_mapa(pos, (0., 0.), 1000.).is_none());
    }

    #[test]
    fn altitude_is_zero_when_landed_on_the_surface() {
        let sistema = SistemaPlanetario::nuevo();
        let p = &sistema.planetas[sistema.indice("TERRA").unwrap()];
        let dir = Vector3::new(0., 1., 0.);
        let holgura = 2.6;
        let mut nave = Nave::new();
        nave.position = p.centro + dir * (p.radio_superficie(p.a_local(dir)) + holgura);
        let mut datos = Datos::new(&nave, 0.6, false);
        datos.con_sistema(&nave, &sistema, holgura, 0., 0.);
        assert!(datos.altitud < 1.0);
    }
}
