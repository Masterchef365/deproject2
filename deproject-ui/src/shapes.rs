use crate::Vertex;

/// In each unit length used in graphics, there are this many thou.
/// That is, one unit length is exactly one inch.
const DOWNSCALE: f32 = 1000.;

const PLANE_SIZE: f32 = 1000.0;

pub fn default_grid() -> Vec<Vertex> {
    grid(
        50 * 12,
        12,
        1.,
        |x, y| [x, 0., y],
        [0.2; 3],
        [0.1; 3],
    )
}

pub fn grid(
    size: i32,
    div: i32,
    scale: f32,
    map_3d: fn(f32, f32) -> [f32; 3],
    primary_color: [f32; 3],
    secondary_color: [f32; 3],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    for i in -size..=size {
        let color = if i.abs() % div == 0 {
            primary_color
        } else {
            secondary_color
        };
        let i = i as f32 * scale;
        let length = div as f32 * scale;

        let subgrid = size / div;
        for j in -subgrid..subgrid {
            vertices.push(Vertex::new(map_3d((j + 1) as f32 * length, i), color));
            vertices.push(Vertex::new(map_3d(j as f32 * length, i), color));
            vertices.push(Vertex::new(map_3d(i, (j + 1) as f32 * length), color));
            vertices.push(Vertex::new(map_3d(i, j as f32 * length), color));
        }
    }
    vertices
}
