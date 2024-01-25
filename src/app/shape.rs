use super::layout::{Action, Room, Vec2};
use geo::BooleanOps;
use serde::{Deserialize, Serialize};

impl Room {
    pub fn vertices(&self) -> Vec<Vec2> {
        let mut vertices = self.shape.vertices(self.pos, self.size);
        for operation in &self.operations {
            let operation_vertices = operation.shape.vertices(operation.pos, operation.size);
            let self_vertices_corrected = vertices
                .iter()
                .map(|v| geo_types::Coord {
                    x: v.x as f64,
                    y: v.y as f64,
                })
                .collect::<Vec<_>>();
            let operation_vertices_corrected = operation_vertices
                .iter()
                .map(|v| geo_types::Coord {
                    x: v.x as f64,
                    y: v.y as f64,
                })
                .collect::<Vec<_>>();
            let poly1 = geo::Polygon::new(geo::LineString::from(self_vertices_corrected), vec![]);
            let poly2 = geo::Polygon::new(geo::LineString::from(operation_vertices_corrected), vec![]);
            let operated: geo_types::MultiPolygon = match operation.action {
                Action::Add => poly1.union(&poly2),
                Action::Subtract => poly1.difference(&poly2),
            };
            vertices = operated
                .0
                .first()
                .expect("Expected at least one polygon")
                .exterior()
                .points()
                .map(|c: geo_types::Point| Vec2 {
                    x: c.x() as f32,
                    y: c.y() as f32,
                })
                .collect::<Vec<_>>();
        }
        vertices
    }

    pub fn triangles(&self) -> (Vec<Vec2>, Vec<[usize; 3]>) {
        let vertices = self.vertices();
        let mut triangles = Vec::new();

        // Convert vertices to a mutable Vec of indices
        let mut indices: Vec<usize> = (0..vertices.len()).collect();

        // Ear clipping triangulation
        while indices.len() > 3 {
            let mut ear_found = false;

            for i in 0..indices.len() {
                let prev = if i == 0 { indices.len() - 1 } else { i - 1 };
                let next = if i == indices.len() - 1 { 0 } else { i + 1 };

                let a = vertices[indices[prev]];
                let b = vertices[indices[i]];
                let c = vertices[indices[next]];

                if is_ear(a, b, c, &vertices, &indices) {
                    // Found an ear
                    triangles.push([indices[prev], indices[i], indices[next]]);
                    indices.remove(i);
                    ear_found = true;
                    break;
                }
            }

            // Break if no ear is found to prevent an infinite loop
            if !ear_found {
                break;
            }
        }

        // Add the remaining triangle
        if indices.len() == 3 {
            triangles.push([indices[0], indices[1], indices[2]]);
        }

        (vertices, triangles)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Shape {
    Rectangle,
    Circle,
}

impl Shape {
    pub fn vertices(&self, pos: Vec2, size: Vec2) -> Vec<Vec2> {
        match self {
            Self::Rectangle => {
                let size_half = Vec2 {
                    x: size.x / 2.0,
                    y: size.y / 2.0,
                };
                vec![
                    Vec2 {
                        x: pos.x - size_half.x,
                        y: pos.y - size_half.y,
                    },
                    Vec2 {
                        x: pos.x + size_half.x,
                        y: pos.y - size_half.y,
                    },
                    Vec2 {
                        x: pos.x + size_half.x,
                        y: pos.y + size_half.y,
                    },
                    Vec2 {
                        x: pos.x - size_half.x,
                        y: pos.y + size_half.y,
                    },
                ]
            }
            Self::Circle => {
                let radius = size.x / 2.0;
                let quality = 90;
                let mut vertices = Vec::new();
                for i in 0..quality {
                    let angle = (i as f32 / quality as f32) * std::f32::consts::PI * 2.0;
                    vertices.push(Vec2 {
                        x: pos.x + angle.cos() * radius,
                        y: pos.y + angle.sin() * radius,
                    });
                }
                vertices
            }
        }
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

fn is_ear(a: Vec2, b: Vec2, c: Vec2, vertices: &[Vec2], indices: &[usize]) -> bool {
    // Check if triangle ABC is convex and no other vertices lie inside it
    if is_convex(a, b, c) {
        for &i in indices {
            let p = vertices[i];
            if p != a && p != b && p != c && point_in_triangle(p, a, b, c) {
                return false;
            }
        }
        true
    } else {
        false
    }
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    // Calculate vectors from the point to the vertices of the triangle
    let pa = a - p;
    let pb = b - p;
    let pc = c - p;

    // Calculate cross products to get z components of 3D vectors
    let cross_pa_pb = pa.x * pb.y - pa.y * pb.x;
    let cross_pb_pc = pb.x * pc.y - pb.y * pc.x;
    let cross_pc_pa = pc.x * pa.y - pc.y * pa.x;

    // Check if point is inside triangle by comparing the signs of the z components
    cross_pa_pb.signum() == cross_pb_pc.signum() && cross_pb_pc.signum() == cross_pc_pa.signum()
}

fn is_convex(a: Vec2, b: Vec2, c: Vec2) -> bool {
    let ab = b - a;
    let bc = c - b;
    let cross_product_z = ab.x * bc.y - ab.y * bc.x;
    cross_product_z > 0.0
}
