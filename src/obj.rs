use crate::color::{asignar_relleno, Relleno};
use raylib::prelude::{Vector2, Vector3};
use std::fs;

pub struct Triangle {
    pub vertices: [Vector3; 3],
    pub uvs: [Vector2; 3],
    pub relleno: Relleno,
}

pub struct Model {
    pub triangles: Vec<Triangle>,
    pub center: Vector3,
    pub size: f32,
}

impl Model {
    pub fn load_obj(path: &str) -> std::io::Result<Model> {
        let content = fs::read_to_string(path)?;
        let mut vertices: Vec<Vector3> = Vec::new();
        let mut uvs: Vec<Vector2> = Vec::new();
        let mut triangles: Vec<Triangle> = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.first().copied() {
                Some("v") if parts.len() >= 4 => {
                    let coord = |i: usize| parts[i].parse::<f32>().unwrap_or(0.0);
                    vertices.push(Vector3::new(coord(1), coord(2), coord(3)));
                }
                Some("vt") if parts.len() >= 3 => {
                    let coord = |i: usize| parts[i].parse::<f32>().unwrap_or(0.0);
                    uvs.push(Vector2::new(coord(1), coord(2)));
                }
                Some("f") => {
                    let corners: Vec<(Vector3, Vector2)> = parts[1..]
                        .iter()
                        .filter_map(|p| {
                            let mut fields = p.split('/');
                            let v = fields.next()?.parse::<usize>().ok()?;
                            let vt = fields.next().and_then(|s| s.parse::<usize>().ok());
                            if v == 0 || v > vertices.len() {
                                return None;
                            }
                            let uv = vt
                                .filter(|&i| i > 0 && i <= uvs.len())
                                .map_or(Vector2::zero(), |i| uvs[i - 1]);
                            Some((vertices[v - 1], uv))
                        })
                        .collect();

                    for i in 1..corners.len().saturating_sub(1) {
                        triangles.push(Triangle {
                            vertices: [corners[0].0, corners[i].0, corners[i + 1].0],
                            uvs: [corners[0].1, corners[i].1, corners[i + 1].1],
                            relleno: asignar_relleno(triangles.len()),
                        });
                    }
                }
                _ => {}
            }
        }

        let (center, size) = bounds(&triangles);
        Ok(Model { triangles, center, size })
    }
}

fn bounds(triangles: &[Triangle]) -> (Vector3, f32) {
    let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);

    for v in triangles.iter().flat_map(|t| t.vertices.iter()) {
        min = Vector3::new(min.x.min(v.x), min.y.min(v.y), min.z.min(v.z));
        max = Vector3::new(max.x.max(v.x), max.y.max(v.y), max.z.max(v.z));
    }

    let center = (min + max) * 0.5;
    let size = (max.x - min.x).max(max.y - min.y).max(max.z - min.z);
    (center, size)
}
