use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::mesh::{Indices, MeshAabb},
};

pub(super) struct SplinePlugin;

impl Plugin for SplinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_spline_mesh);
        app.register_type::<SplineMesh>();
    }
}

#[derive(Component, Clone, Copy, Reflect, PartialEq)]
pub struct Spline {
    pub controls: [SplineControl; 2],
    pub min_segment_length: f32,
    pub max_segments: Option<usize>,
}

impl Default for Spline {
    fn default() -> Self {
        Self {
            controls: Default::default(),
            min_segment_length: 10.0,
            max_segments: None,
        }
    }
}

impl Spline {
    pub fn with_max_segments(&mut self, max_segments: usize) -> &mut Self {
        self.max_segments = Some(max_segments);
        self
    }

    pub fn create_curve_control_points(&self) -> [[Vec3; 4]; 1] {
        let start = &self.controls[0];
        let end = &self.controls[1];

        let length = (end.pos - start.pos).length();

        [[
            start.pos,
            start.pos + start.forward * length * 0.5,
            end.pos + end.forward * length * 0.5,
            end.pos,
        ]]
    }

    /// Use points generated by create_curve_points
    pub fn create_curve_points(&self, points: [[Vec3; 4]; 1]) -> Vec<Vec3> {
        let start = points[0][0];
        let end = points[0][3];
        let mut segments =
            ((start.distance(end) / self.min_segment_length).round() as usize).max(2);
        if let Some(max_segments) = self.max_segments {
            segments = segments.min(max_segments);
        }

        CubicBezier::new(points)
            .to_curve()
            .unwrap()
            .iter_positions(segments)
            .collect()
    }

    pub fn create_cubic_curve(&self) -> CubicCurve<Vec3> {
        CubicBezier::new(self.create_curve_control_points())
            .to_curve()
            .unwrap()
    }
}

/// Represents the start and end of a spline, also knows as knots
#[derive(Default, Clone, Copy, Reflect, PartialEq)]
pub struct SplineControl {
    pub pos: Vec3,
    /// Points in the direction of the curve
    /// EX: for a horizontal curve the left control would point to the right
    pub forward: Vec3,
}

/// Samples a spline and generates a mesh from it
#[derive(Component, Reflect)]
#[require(Spline, Mesh3d)]
pub struct SplineMesh {
    pub width: f32,
    pub source_spline_data: Spline,
}

impl Default for SplineMesh {
    fn default() -> Self {
        Self {
            width: 2.,
            source_spline_data: default(),
        }
    }
}

fn update_spline_mesh(
    mut c: Commands,
    mut q: Query<(Entity, &mut Mesh3d, &Spline, &mut SplineMesh), Changed<Spline>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmos: Gizmos,
) {
    for (entity, mut mesh, spline, mut spline_mesh) in &mut q {
        let entity = c.get_entity(entity);
        if entity.is_none()
            || spline.controls[0].pos == spline.controls[1].pos
            || spline_mesh.source_spline_data == *spline
        {
            continue;
        }

        let mut ec = entity.unwrap();
        spline_mesh.source_spline_data = spline.clone();

        let mesh = match meshes.get_mut(mesh.id()) {
            Some(mesh) => mesh,
            None => {
                mesh.0 = meshes.add(create_spline_mesh());
                meshes.get_mut(mesh.id()).unwrap()
            }
        };

        let mut points = spline.create_curve_points(spline.create_curve_control_points());
        points.push(spline.controls[1].pos - spline.controls[1].forward);
        let mut vertices = Vec::new();
        let mut normal = Vec::new();
        vertices.reserve(points.len() * 2);
        normal.reserve(points.len() * 2);

        points
            .iter()
            .zip(points.iter().skip(1))
            .for_each(|(sample, next)| {
                let forward = (next - sample).normalize();
                let side = forward.cross(Vec3::Y);

                // Generate left and right vertices
                // TODO: Remove the vertical offset once we have a mesh with height, otherwise we will have z-fighting
                let right = sample - side * spline_mesh.width + Vec3::Y * 0.01;
                let left = sample + side * spline_mesh.width + Vec3::Y * 0.01;
                vertices.push(right);
                vertices.push(left);
                let up = side.cross(forward);
                normal.push(up);
                normal.push(up);

                gizmos.arrow(*sample, sample + forward, Color::srgb(1., 0., 0.));
                gizmos.line(*sample, right, Color::srgb(0., 1., 0.));
                gizmos.line(*sample, left, Color::srgb(1., 0., 0.));
                gizmos.line(*sample, sample + up, Color::srgb(0., 0., 1.));
            });

        let quads = vertices.len() / 2 - 1;

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);

        let mut indices = Vec::<u16>::new();
        indices.reserve(quads * 6);

        for i in 0..quads {
            let offset = (i * 2) as u16;
            indices.append(
                &mut [
                    offset + 0,
                    offset + 1,
                    offset + 2,
                    offset + 1,
                    offset + 3,
                    offset + 2,
                ]
                .to_vec(),
            );
        }

        info!("points {} indices {}", points.len(), indices.len());

        // info!(
        //     "points{}\nvertices{} {:?}\nindices{} {:?}",
        //     points.len(),
        //     vertices.len(),
        //     vertices,
        //     indices.len(),
        //     indices
        // );

        mesh.insert_indices(Indices::U16(indices));
        ec.insert(mesh.compute_aabb().unwrap());
    }
}

fn create_spline_mesh() -> Mesh {
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}
