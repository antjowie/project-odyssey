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

#[derive(Component, Clone, Reflect, PartialEq)]
pub struct Spline {
    controls: [SplineControl; 2],
    pub min_segment_length: f32,
    pub max_segments: Option<usize>,
    curve: CubicCurve<Vec3>,
    curve_length: f32,
}

impl Default for Spline {
    fn default() -> Self {
        Self {
            controls: Default::default(),
            min_segment_length: 10.0,
            max_segments: None,
            curve: CubicBezier::new([[Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO]])
                .to_curve()
                .unwrap(),
            curve_length: 0.0,
        }
    }
}

impl Spline {
    pub fn controls(&self) -> &[SplineControl; 2] {
        &self.controls
    }

    pub fn set_controls(&mut self, controls: [SplineControl; 2]) {
        self.controls = controls;
        self.calculate_curve();
    }

    pub fn set_controls_index(&mut self, index: usize, control: SplineControl) {
        self.controls[index] = control;
        self.calculate_curve();
    }

    fn calculate_curve(&mut self) {
        self.curve = CubicBezier::new(self.create_curve_control_points())
            .to_curve()
            .unwrap();

        // let mut points = self.create_curve_points();
        // // points.push(self.controls[1].pos - self.controls[1].forward);

        // let (start, end) = points
        //     .iter()
        //     .zip(points.iter().skip(1))
        //     .min_by(|x, y| {
        //         let left = pos.distance_squared(*x.0) + pos.distance_squared(*x.1);
        //         let right = pos.distance_squared(*y.0) + pos.distance_squared(*y.1);
        //         left.total_cmp(&right)
        //     })
        //     .unwrap();

        // self.curve_length = self.curve.iter_accelerations(subdivisions)
    }

    pub fn curve(&self) -> &CubicCurve<Vec3> {
        &self.curve
    }

    pub fn with_max_segments(mut self, max_segments: usize) -> Self {
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

    pub fn create_curve_points(&self) -> Vec<Vec3> {
        let start = self.controls[0].pos;
        let end = self.controls[1].pos;
        let mut segments =
            ((start.distance(end) / self.min_segment_length).round() as usize).max(2);
        if let Some(max_segments) = self.max_segments {
            segments = segments.min(max_segments);
        }

        self.curve.iter_positions(segments).collect()
    }

    /// Returns the nearest position to the spline, for rails this represents
    /// the center of the rail.
    pub fn get_nearest_point(&self, pos: &Vec3) -> (Vec3, Dir3) {
        // Gather all point and do distance checks to see which segment pos is closest to
        let points = self.create_curve_points();
        let (start, end) = points
            .iter()
            .zip(points.iter().skip(1))
            .min_by(|x, y| {
                let left = pos.distance_squared(*x.0) + pos.distance_squared(*x.1);
                let right = pos.distance_squared(*y.0) + pos.distance_squared(*y.1);
                left.total_cmp(&right)
            })
            .unwrap();

        // Calculate perpendicular vec from pos to rail
        let forward = Dir3::new(end - start).unwrap();
        let right = forward.cross(Vec3::Y);

        let to_center = (start - pos).project_onto(right);
        (pos + to_center, forward)
    }

    pub fn t_from_pos(&self, pos: &Vec3, gizmos: &mut Gizmos) {
        let control_points = self.create_curve_control_points();
        let s = control_points[0][0];
        let sc = control_points[0][1];
        let ec = control_points[0][2];
        let e = control_points[0][3];

        // https://pomax.github.io/bezierinfo/#splitting
        let pos = self.get_nearest_point(pos).0;
        let t = 0.5;
        let t1 = s + (sc - s) * t;
        let t2 = sc + (ec - sc) * t;
        let t3 = ec + (e - ec) * t;
        let start = [[s, t1, t1 + (t2 - t1) * t, pos]];
        let end = [[pos, t2 + (t3 - t2) * t, t3, e]];

        gizmos.sphere(pos, 5.0, Color::BLACK);
        gizmos.sphere(s, 5.0, Color::srgb(1.0, 0.0, 0.0));
        gizmos.sphere(sc, 5.0, Color::srgb(0.0, 1.0, 0.0));
        gizmos.sphere(ec, 5.0, Color::srgb(0.0, 0.0, 1.0));
        gizmos.sphere(e, 5.0, Color::srgb(1.0, 1.0, 1.0));
        // gizmos.sphere(t1, 5.0, Color::srgb(1.0, 0.0, 0.0));
        // gizmos.sphere(t2, 5.0, Color::srgb(0.0, 1.0, 0.0));
        // gizmos.sphere(t3, 5.0, Color::srgb(0.0, 0.0, 1.0));
        gizmos.line(start[0][0], start[0][1], Color::srgb(0.0, 0.0, 1.0));
        gizmos.line(start[0][1], start[0][2], Color::srgb(0.0, 0.0, 1.0));
        gizmos.line(start[0][2], start[0][3], Color::srgb(0.0, 0.0, 1.0));
        gizmos.line(end[0][0], end[0][1], Color::srgb(1.0, 0.0, 0.0));
        gizmos.line(end[0][1], end[0][2], Color::srgb(1.0, 0.0, 0.0));
        gizmos.line(end[0][2], end[0][3], Color::srgb(1.0, 0.0, 0.0));
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
        if spline.controls()[0].pos == spline.controls()[1].pos
            || spline_mesh.source_spline_data == *spline
        {
            continue;
        }

        spline_mesh.source_spline_data = spline.clone();

        let mesh = match meshes.get_mut(mesh.id()) {
            Some(mesh) => mesh,
            None => {
                mesh.0 = meshes.add(create_spline_mesh());
                meshes.get_mut(mesh.id()).unwrap()
            }
        };

        let mut points = spline.create_curve_points();
        // Insert one element after last, imagine we have 3 samples
        // 1. 0->1 == Insert vertices
        // 2. 1->2 == Insert vertices
        // 3. 2->None == No insertion
        // To generate the mesh, we want to also insert an element at the end which is just an extension
        points.push(spline.controls()[1].pos - spline.controls()[1].forward);

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

        // info!("points {} indices {}", points.len(), indices.len());
        // info!(
        //     "points{}\nvertices{} {:?}\nindices{} {:?}",
        //     points.len(),
        //     vertices.len(),
        //     vertices,
        //     indices.len(),
        //     indices
        // );

        mesh.insert_indices(Indices::U16(indices));

        // At this moment check if our entity still exists, otherwise we just drop it
        if let Some(mut ec) = c.get_entity(entity) {
            ec.try_insert(mesh.compute_aabb().unwrap());
        }
    }
}

fn create_spline_mesh() -> Mesh {
    Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}
