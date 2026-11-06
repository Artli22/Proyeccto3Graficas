//! Mapa del HUD: la imagen de la pista se reduce a una mascara de transparencia y su linea
//! central se traza en CPU (adelgazamiento, poda y recorrido) para mover el marcador sobre ella.
use raylib::prelude::Image;

const PUNTOS_PISTA: usize = 256;
// Luminancia por debajo de la cual un pixel se considera asfalto (las marcas son mas claras).
const UMBRAL_PISTA: u32 = 85;

pub struct Mapa {
    pub ancho: i32,
    pub alto: i32,
    alfa: Vec<u8>,
    pista: Vec<(f32, f32)>,
}

impl Mapa {
    pub fn cargar(ruta: &str, ancho_destino: i32) -> Result<Self, String> {
        let imagen = Image::load_image(ruta).map_err(|e| format!("{ruta}: {e}"))?;
        let (w, h) = (imagen.width as usize, imagen.height as usize);
        let luma: Vec<u8> = imagen
            .get_image_data()
            .iter()
            .map(|c| ((c.r as u32 * 299 + c.g as u32 * 587 + c.b as u32 * 114) / 1000) as u8)
            .collect();
        Self::desde_luminancia(&luma, w, h, ancho_destino)
    }

    fn desde_luminancia(luma: &[u8], w: usize, h: usize, ancho: i32) -> Result<Self, String> {
        let mascara: Vec<bool> = luma.iter().map(|&l| (l as u32) < UMBRAL_PISTA).collect();
        let linea = extraer_pista(&mascara, w, h);
        if linea.len() < 16 {
            return Err("no se pudo trazar la pista del mapa".to_string());
        }
        let escala = ancho as f32 / w as f32;
        let alto = (h as f32 * escala).round() as i32;
        Ok(Self {
            ancho,
            alto,
            alfa: reducir(luma, w, h, ancho as usize, alto as usize),
            pista: linea.iter().map(|&(x, y)| (x * escala, y * escala)).collect(),
        })
    }

    pub fn alfa(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.ancho || y >= self.alto {
            return 0;
        }
        self.alfa[(y * self.ancho + x) as usize]
    }

    /// Posicion y direccion de avance sobre la pista para `t` en vueltas (se envuelve en [0, 1)).
    pub fn punto(&self, t: f32) -> ((f32, f32), (f32, f32)) {
        let n = self.pista.len();
        let pos = t.rem_euclid(1.) * n as f32;
        let i = (pos.floor() as usize) % n;
        let f = pos - pos.floor();
        let (a, b) = (self.pista[i], self.pista[(i + 1) % n]);
        let (p, q) = (self.pista[(i + n - 1) % n], self.pista[(i + 2) % n]);
        let largo = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
        let dir = if largo > 1e-4 {
            ((q.0 - p.0) / largo, (q.1 - p.1) / largo)
        } else {
            (1., 0.)
        };
        ((a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f), dir)
    }
}

// Promedio de oscuridad por celda; el fondo casi blanco se descarta para dejar solo el trazo.
fn reducir(luma: &[u8], w: usize, h: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut alfa = vec![0u8; dw * dh];
    for dy in 0..dh {
        let (y0, y1) = (dy * h / dh, ((dy + 1) * h / dh).max(dy * h / dh + 1).min(h));
        for dx in 0..dw {
            let (x0, x1) = (dx * w / dw, ((dx + 1) * w / dw).max(dx * w / dw + 1).min(w));
            let mut suma = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    suma += 255 - luma[y * w + x] as u32;
                }
            }
            let media = suma as f32 / ((y1 - y0) * (x1 - x0)) as f32;
            alfa[dy * dw + dx] = ((media - 12.).max(0.) * 2.2).min(255.) as u8;
        }
    }
    alfa
}

fn adelgazar(m: &mut [bool], w: usize, h: usize) {
    let mut borrar: Vec<usize> = Vec::new();
    loop {
        let mut cambio = false;
        for paso in 0..2 {
            borrar.clear();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if !m[i] {
                        continue;
                    }
                    // N, NE, E, SE, S, SO, O, NO
                    let p = [
                        m[i - w],
                        m[i - w + 1],
                        m[i + 1],
                        m[i + w + 1],
                        m[i + w],
                        m[i + w - 1],
                        m[i - 1],
                        m[i - w - 1],
                    ];
                    let vecinos = p.iter().filter(|&&v| v).count();
                    if !(2..=6).contains(&vecinos) {
                        continue;
                    }
                    let transiciones = (0..8).filter(|&k| !p[k] && p[(k + 1) % 8]).count();
                    if transiciones != 1 {
                        continue;
                    }
                    let (n, e, s, o) = (p[0], p[2], p[4], p[6]);
                    let quitar = if paso == 0 {
                        !(n && e && s) && !(e && s && o)
                    } else {
                        !(n && e && o) && !(n && s && o)
                    };
                    if quitar {
                        borrar.push(i);
                    }
                }
            }
            if !borrar.is_empty() {
                cambio = true;
                for &i in &borrar {
                    m[i] = false;
                }
            }
        }
        if !cambio {
            break;
        }
    }
}

fn grado(m: &[bool], w: usize, i: usize) -> usize {
    [
        i - w,
        i - w + 1,
        i + 1,
        i + w + 1,
        i + w,
        i + w - 1,
        i - 1,
        i - w - 1,
    ]
    .iter()
    .filter(|&&j| m[j])
    .count()
}

// Quita las ramas sueltas hasta que solo quedan los trazos cerrados.
fn podar(m: &mut [bool], w: usize) {
    let mut pixeles: Vec<usize> = (0..m.len()).filter(|&i| m[i]).collect();
    loop {
        let puntas: Vec<usize> = pixeles
            .iter()
            .copied()
            .filter(|&i| grado(m, w, i) <= 1)
            .collect();
        if puntas.is_empty() {
            break;
        }
        for &i in &puntas {
            m[i] = false;
        }
        pixeles.retain(|&i| m[i]);
    }
}

fn vecinos(i: usize, w: usize) -> [(i32, i32, usize); 8] {
    let w = w as i64;
    let i = i as i64;
    let d = |dx: i32, dy: i32| (dx, dy, (i + dy as i64 * w + dx as i64) as usize);
    [
        d(0, -1),
        d(1, 0),
        d(0, 1),
        d(-1, 0),
        d(1, -1),
        d(1, 1),
        d(-1, 1),
        d(-1, -1),
    ]
}

// Componente conexa (8 vecinos) con mas pixeles.
fn componente_mayor(m: &[bool], w: usize) -> Vec<usize> {
    let mut visto = vec![false; m.len()];
    let mut mejor: Vec<usize> = Vec::new();
    for inicio in 0..m.len() {
        if !m[inicio] || visto[inicio] {
            continue;
        }
        let mut comp = vec![inicio];
        visto[inicio] = true;
        let mut k = 0;
        while k < comp.len() {
            for (_, _, j) in vecinos(comp[k], w) {
                if m[j] && !visto[j] {
                    visto[j] = true;
                    comp.push(j);
                }
            }
            k += 1;
        }
        if comp.len() > mejor.len() {
            mejor = comp;
        }
    }
    mejor
}

// Recorre la linea de un pixel de ancho conservando el rumbo, para no saltar entre tramos cercanos.
fn recorrer(m: &[bool], w: usize, comp: &[usize]) -> Vec<(f32, f32)> {
    let mut visitado = vec![false; m.len()];
    let inicio = *comp.iter().min().unwrap();
    let mut camino = vec![inicio];
    visitado[inicio] = true;
    loop {
        let actual = *camino.last().unwrap();
        let atras = camino[camino.len().saturating_sub(9)];
        let rumbo = (
            (actual % w) as f32 - (atras % w) as f32,
            (actual / w) as f32 - (atras / w) as f32,
        );
        let rumbo_largo = (rumbo.0 * rumbo.0 + rumbo.1 * rumbo.1).sqrt();
        let mut mejor: Option<(f32, usize)> = None;
        for (dx, dy, j) in vecinos(actual, w) {
            if !m[j] || visitado[j] {
                continue;
            }
            let paso = ((dx * dx + dy * dy) as f32).sqrt();
            let alineacion = if rumbo_largo > 0. {
                (rumbo.0 * dx as f32 + rumbo.1 * dy as f32) / (rumbo_largo * paso)
            } else {
                0.
            };
            let puntaje = alineacion - 0.01 * (paso - 1.);
            if mejor.map_or(true, |(p, _)| puntaje > p) {
                mejor = Some((puntaje, j));
            }
        }
        match mejor {
            Some((_, j)) => {
                visitado[j] = true;
                camino.push(j);
            }
            None => break,
        }
    }
    camino
        .iter()
        .map(|&i| ((i % w) as f32, (i / w) as f32))
        .collect()
}

// Puntos equidistantes sobre la curva cerrada, con un suavizado ligero del escalonado de pixeles.
fn remuestrear(camino: &[(f32, f32)], n: usize) -> Vec<(f32, f32)> {
    let m = camino.len();
    let dist = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let mut acumulado = vec![0.0f32; m + 1];
    for i in 0..m {
        acumulado[i + 1] = acumulado[i] + dist(camino[i], camino[(i + 1) % m]);
    }
    let total = acumulado[m];
    let mut puntos = Vec::with_capacity(n);
    let mut seg = 0;
    for k in 0..n {
        let s = total * k as f32 / n as f32;
        while seg + 1 < m && acumulado[seg + 1] < s {
            seg += 1;
        }
        let largo = (acumulado[seg + 1] - acumulado[seg]).max(1e-6);
        let f = ((s - acumulado[seg]) / largo).clamp(0., 1.);
        let (a, b) = (camino[seg], camino[(seg + 1) % m]);
        puntos.push((a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f));
    }
    (0..n)
        .map(|i| {
            let (p, c, q) = (puntos[(i + n - 1) % n], puntos[i], puntos[(i + 1) % n]);
            ((p.0 + 2. * c.0 + q.0) / 4., (p.1 + 2. * c.1 + q.1) / 4.)
        })
        .collect()
}

fn extraer_pista(mascara: &[bool], w: usize, h: usize) -> Vec<(f32, f32)> {
    let mut m = mascara.to_vec();
    for x in 0..w {
        m[x] = false;
        m[(h - 1) * w + x] = false;
    }
    for y in 0..h {
        m[y * w] = false;
        m[y * w + w - 1] = false;
    }
    adelgazar(&mut m, w, h);
    podar(&mut m, w);
    let comp = componente_mayor(&m, w);
    if comp.len() < PUNTOS_PISTA / 2 {
        return Vec::new();
    }
    remuestrear(&recorrer(&m, w, &comp), PUNTOS_PISTA)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anillo(w: usize, h: usize, radio: f32, grosor: f32) -> Vec<bool> {
        let (cx, cy) = (w as f32 / 2., h as f32 / 2.);
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32 - cx, (i / w) as f32 - cy);
                ((x * x + y * y).sqrt() - radio).abs() <= grosor / 2.
            })
            .collect()
    }

    #[test]
    fn centerline_of_a_ring_is_closed_ordered_and_centered() {
        let (w, h, radio) = (200, 200, 70.);
        let linea = extraer_pista(&anillo(w, h, radio, 9.), w, h);
        assert_eq!(linea.len(), PUNTOS_PISTA);
        let mut acumulado = 0.;
        for i in 0..linea.len() {
            let (x, y) = (linea[i].0 - 100., linea[i].1 - 100.);
            assert!(((x * x + y * y).sqrt() - radio).abs() < 2.5);
            let (a, b) = (linea[i], linea[(i + 1) % linea.len()]);
            acumulado += ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        }
        // Recorrido completo, una sola vuelta, a ritmo uniforme.
        let esperado = std::f32::consts::TAU * radio;
        assert!((acumulado - esperado).abs() < esperado * 0.05);
    }

    #[test]
    fn short_spurs_are_pruned() {
        let (w, h) = (200, 200);
        let mut m = anillo(w, h, 70., 9.);
        for y in 20..45 {
            for x in 99..102 {
                m[y * w + x] = true;
            }
        }
        let linea = extraer_pista(&m, w, h);
        for &(x, y) in &linea {
            let (dx, dy) = (x - 100., y - 100.);
            assert!(((dx * dx + dy * dy).sqrt() - 70.).abs() < 3.5);
        }
    }

    #[test]
    fn marker_wraps_and_follows_the_path() {
        let luma: Vec<u8> = anillo(200, 200, 70., 9.)
            .iter()
            .map(|&d| if d { 40 } else { 250 })
            .collect();
        let mapa = Mapa::desde_luminancia(&luma, 200, 200, 100).unwrap();
        let (a, _) = mapa.punto(0.25);
        let (b, _) = mapa.punto(1.25);
        assert!((a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3);
        let (centro, dir) = mapa.punto(0.5);
        assert!(((centro.0 - 50.).powi(2) + (centro.1 - 50.).powi(2)).sqrt() > 30.);
        assert!(((dir.0 * dir.0 + dir.1 * dir.1).sqrt() - 1.).abs() < 1e-3);
        assert!(mapa.alfa(50 + 35, 50) > 100 && mapa.alfa(50, 50) == 0);
    }

    #[test]
    fn real_track_image_yields_a_closed_loop() {
        let ruta = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/hud/mapa.png");
        let mapa = Mapa::cargar(ruta, 270).expect("el mapa debe cargarse");
        assert_eq!(mapa.pista.len(), PUNTOS_PISTA);
        let largo = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        let paso: Vec<f32> = (0..PUNTOS_PISTA)
            .map(|i| largo(mapa.pista[i], mapa.pista[(i + 1) % PUNTOS_PISTA]))
            .collect();
        let total: f32 = paso.iter().sum();
        let medio = total / PUNTOS_PISTA as f32;
        // Un circuito cerrado y recorrido por completo: sin saltos grandes entre muestras.
        assert!(total > 500., "recorrido demasiado corto: {total}");
        assert!(paso.iter().all(|&p| p < medio * 4.), "hay un salto entre tramos");
    }
}
