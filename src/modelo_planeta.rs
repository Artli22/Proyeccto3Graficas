
use crate::planeta::Malla;
use cohete::color::Relleno;
use raylib::prelude::{Color, Vector3};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const COLOR_DEFECTO: [u8; 3] = [128, 128, 128];

const PALETA: &[(&str, [u8; 3])] = &[
    ("violeta", [110, 60, 170]),
    ("crema", [238, 222, 184]),
    ("lavanda", [180, 160, 225]),
    ("verde", [70, 170, 110]),
    ("panel_oscuro", [46, 48, 58]),
    ("panel_claro", [120, 124, 138]),
    ("acero", [150, 158, 170]),
    ("oro_claro", [240, 208, 110]),
    ("oro_oscuro", [150, 110, 30]),
    ("oro", [212, 170, 52]),
];

fn leer_mtl(ruta: &Path) -> HashMap<String, [u8; 3]> {
    let mut colores = HashMap::new();
    let Ok(contenido) = fs::read_to_string(ruta) else {
        return colores;
    };
    let mut actual: Option<String> = None;
    for linea in contenido.lines() {
        let partes: Vec<&str> = linea.split_whitespace().collect();
        match partes.first().copied() {
            Some("newmtl") if partes.len() >= 2 => actual = Some(partes[1].to_string()),
            Some("Kd") if partes.len() >= 4 => {
                let canal = |i: usize| {
                    (partes[i].parse::<f32>().unwrap_or(0.5).clamp(0., 1.) * 255.).round() as u8
                };
                if let Some(nombre) = &actual {
                    colores.insert(nombre.clone(), [canal(1), canal(2), canal(3)]);
                }
            }
            _ => {}
        }
    }
    colores
}

fn color_paleta(material: &str) -> [u8; 3] {
    // "Oro.001" comparte color con "Oro".
    let base = material.split('.').next().unwrap_or(material).to_lowercase();
    PALETA
        .iter()
        .find(|(nombre, _)| *nombre == base)
        .map_or(COLOR_DEFECTO, |&(_, c)| c)
}

/// Carga `ruta` centrado en el centroide del objeto `nucleo` y escalado para que ese objeto
/// (la esfera del cuerpo) tenga radio `radio`. Devuelve la malla y la distancia maxima al
/// centro en unidades de mundo (para el descarte por frustum).
pub fn cargar(ruta: &str, nucleo: &str, radio: f32) -> std::io::Result<(Malla, f32)> {
    let contenido = fs::read_to_string(ruta)?;
    let mut vertices: Vec<Vector3> = Vec::new();
    let mut objetos: Vec<(String, usize)> = Vec::new();
    let mut materiales: HashMap<String, [u8; 3]> = HashMap::new();
    let mut material = String::new();
    let mut caras: Vec<(Vec<usize>, [u8; 3])> = Vec::new();

    for linea in contenido.lines() {
        let partes: Vec<&str> = linea.split_whitespace().collect();
        match partes.first().copied() {
            Some("mtllib") if partes.len() >= 2 => {
                let mtl = Path::new(ruta).with_file_name(partes[1]);
                materiales.extend(leer_mtl(&mtl));
            }
            Some("o") if partes.len() >= 2 => objetos.push((partes[1].to_string(), vertices.len())),
            Some("usemtl") if partes.len() >= 2 => material = partes[1].to_string(),
            Some("v") if partes.len() >= 4 => {
                let c = |i: usize| partes[i].parse::<f32>().unwrap_or(0.);
                vertices.push(Vector3::new(c(1), c(2), c(3)));
            }
            Some("f") => {
                let indices: Vec<usize> = partes[1..]
                    .iter()
                    .filter_map(|p| p.split('/').next()?.parse::<usize>().ok())
                    .filter(|&i| i > 0 && i <= vertices.len())
                    .map(|i| i - 1)
                    .collect();
                let color = materiales
                    .get(&material)
                    .copied()
                    .unwrap_or_else(|| color_paleta(&material));
                caras.push((indices, color));
            }
            _ => {}
        }
    }

    let invalido = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
    let posicion = objetos
        .iter()
        .position(|(n, _)| n == nucleo)
        .ok_or_else(|| invalido("el OBJ no contiene el objeto del nucleo"))?;
    let inicio = objetos[posicion].1;
    let fin = objetos.get(posicion + 1).map_or(vertices.len(), |o| o.1);
    let nucleo_v = &vertices[inicio..fin];
    if nucleo_v.is_empty() {
        return Err(invalido("el objeto del nucleo no tiene vertices"));
    }
    let centro = nucleo_v.iter().fold(Vector3::zero(), |a, &v| a + v) / nucleo_v.len() as f32;
    let radio_nucleo =
        nucleo_v.iter().map(|&v| (v - centro).length()).sum::<f32>() / nucleo_v.len() as f32;
    if radio_nucleo < 1e-6 {
        return Err(invalido("el nucleo tiene radio cero"));
    }
    let escala = radio / radio_nucleo;
    let local: Vec<Vector3> = vertices.iter().map(|&v| (v - centro) * escala).collect();
    let extension = local.iter().map(|v| v.length()).fold(0., f32::max);

    let mut malla = Malla {
        triangulos: Vec::new(),
        normales: Vec::new(),
        rellenos: Vec::new(),
    };
    for (indices, [r, g, b]) in caras {
        for i in 1..indices.len().saturating_sub(1) {
            let tri = [local[indices[0]], local[indices[i]], local[indices[i + 1]]];
            let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            if normal.length() < 1e-9 {
                continue;
            }
            malla.triangulos.push(tri);
            malla.normales.push(normal.normalized());
            malla.rellenos.push(Relleno::Solido(Color::new(r, g, b, 255)));
        }
    }
    Ok((malla, extension))
}

pub fn simplificar(malla: &Malla, extension: f32, celdas: u32) -> Malla {
    let lado = (2. * extension / celdas.max(1) as f32).max(1e-6);
    let celda = |v: Vector3| {
        (
            (v.x / lado).floor() as i32,
            (v.y / lado).floor() as i32,
            (v.z / lado).floor() as i32,
        )
    };
    let mut acumulado: HashMap<(i32, i32, i32), (Vector3, u32)> = HashMap::new();
    for v in malla.triangulos.iter().flatten() {
        let e = acumulado.entry(celda(*v)).or_insert((Vector3::zero(), 0));
        e.0 += *v;
        e.1 += 1;
    }
    let representante = |v: Vector3| {
        let (suma, n) = acumulado[&celda(v)];
        suma / n as f32
    };
    let mut salida = Malla {
        triangulos: Vec::new(),
        normales: Vec::new(),
        rellenos: Vec::new(),
    };
    for (tri, relleno) in malla.triangulos.iter().zip(&malla.rellenos) {
        let c = tri.map(celda);
        if c[0] == c[1] || c[1] == c[2] || c[0] == c[2] {
            continue;
        }
        let nuevo = tri.map(representante);
        let normal = (nuevo[1] - nuevo[0]).cross(nuevo[2] - nuevo[0]);
        if normal.length() < 1e-9 {
            continue;
        }
        salida.triangulos.push(nuevo);
        salida.normales.push(normal.normalized());
        salida.rellenos.push(relleno.clone());
    }
    salida
}

/// Color promedio de la malla, para el nivel de detalle lejano.
pub fn color_medio(malla: &Malla) -> Color {
    let mut suma = [0u64; 3];
    for r in &malla.rellenos {
        if let Relleno::Solido(c) = r {
            suma[0] += c.r as u64;
            suma[1] += c.g as u64;
            suma[2] += c.b as u64;
        }
    }
    let n = malla.rellenos.len().max(1) as u64;
    Color::new((suma[0] / n) as u8, (suma[1] / n) as u8, (suma[2] / n) as u8, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruta(nombre: &str) -> String {
        format!("{}/src/assets/{nombre}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn models_load_scaled_to_the_requested_core_radius() {
        for (archivo, nucleo) in [("Mogo.obj", "Planeta"), ("Unicron.obj", "Esfera_nucleo")] {
            let (malla, extension) = cargar(&ruta(archivo), nucleo, 100.).unwrap();
            assert!(malla.triangulos.len() > 10_000, "{archivo}");
            assert_eq!(malla.triangulos.len(), malla.rellenos.len());
            assert!(extension > 100., "{archivo}: la geometria sobrepasa el nucleo");
            assert!(malla.triangulos.iter().flatten().all(|v| v.length() <= extension + 1e-2));
        }
    }

    #[test]
    fn simplified_levels_have_fewer_triangles_and_stay_in_bounds() {
        let (malla, extension) = cargar(&ruta("Unicron.obj"), "Esfera_nucleo", 100.).unwrap();
        let mut anterior = malla.triangulos.len();
        for celdas in [64, 32, 14] {
            let s = simplificar(&malla, extension, celdas);
            assert!(!s.triangulos.is_empty());
            assert!(s.triangulos.len() < anterior, "{celdas} celdas");
            assert_eq!(s.triangulos.len(), s.rellenos.len());
            assert_eq!(s.triangulos.len(), s.normales.len());
            assert!(s.triangulos.iter().flatten().all(|v| v.length() <= extension + 1e-2));
            anterior = s.triangulos.len();
        }
    }

    #[test]
    fn material_names_map_to_distinct_colors_with_suffixes_ignored() {
        assert_eq!(color_paleta("Oro.001"), color_paleta("Oro"));
        assert_ne!(color_paleta("Oro"), color_paleta("Oro_claro"));
        assert_ne!(color_paleta("Violeta"), color_paleta("Verde"));
        assert_eq!(color_paleta("Desconocido"), COLOR_DEFECTO);
    }
}
