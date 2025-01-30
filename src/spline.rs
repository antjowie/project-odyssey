use core::f32;

use avian3d::position;
use bevy::{
    asset::RenderAssetUsages,
    color::palettes::tailwind::{BLUE_500, GREEN_500, RED_500},
    prelude::*,
    render::mesh::{Indices, MeshAabb},
};

pub(super) struct SplinePlugin;

impl Plugin for SplinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_spline_mesh);
        app.register_type::<SplineMesh>();
        app.add_systems(Update, debug_spline);
    }
}

const LUT_SAMPLES: usize = 32;
#[derive(Component, Clone, Reflect, PartialEq)]
pub struct Spline {
    controls: [SplineControl; 2],
    /// Override for the curve control points, used when we are extending and can't realy on the path types
    /// TODO: Might have to reconsider how we build rails, since we calculate control points based on length
    /// we lose control in these kinds of scenarios, it might be better to just go for a more granular build mode
    /// where the user specified their own control points, maybe even allow modifying control points of exisiting rails
    controls_override: Option<[Vec3; 2]>,
    /// Segments define how many curve_points to generate
    pub min_segment_length: f32,
    curve: CubicCurve<Vec3>,
    /// Uniformly spaced points in the curve, length is controlled by min segment length and max segments
    curve_points: Vec<Vec3>,
    /// https://pomax.github.io/bezierinfo/#tracing
    lut: [SplineLUT; LUT_SAMPLES],
    curve_length: f32,
}

#[derive(Default, Reflect, Clone, Copy, PartialEq)]
struct SplineLUT {
    t: f32,
    /// Distance between this and previous pos
    distance: f32,
    distance_along_curve: f32,
    pos: Vec3,
}

impl Default for Spline {
    fn default() -> Self {
        Self {
            controls: Default::default(),
            controls_override: None,
            min_segment_length: 10.0,
            curve: CubicBezier::new([[Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO]])
                .to_curve()
                .unwrap(),
            curve_points: vec![],
            lut: [SplineLUT::default(); LUT_SAMPLES],
            curve_length: 0.0,
        }
    }
}

impl Spline {
    pub fn controls(&self) -> &[SplineControl; 2] {
        &self.controls
    }

    pub fn curve_points(&self) -> &Vec<Vec3> {
        &self.curve_points
    }

    pub fn curve_length(&self) -> f32 {
        self.curve_length
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

        // Populate LUT and curve length
        self.curve_length = 0.0;
        let points: Vec<Vec3> = self.curve.iter_positions(LUT_SAMPLES - 1).collect();
        points
            .iter()
            .zip(points.iter().skip(1))
            .enumerate()
            .for_each(|(i, (left, right))| {
                let i = i + 1;
                let t = i as f32 / (LUT_SAMPLES - 1) as f32;
                let distance = left.distance(*right);
                self.curve_length += distance;
                self.lut[i] = SplineLUT {
                    t,
                    distance,
                    distance_along_curve: self.curve_length,
                    pos: *right,
                };
            });
        self.lut[0].pos = points[0];

        // Populate curve points
        let segments = ((self.curve_length / self.min_segment_length).round() as usize).max(2);
        self.curve_points.clear();
        self.curve_points.reserve(segments);
        self.curve_points.push(self.lut[0].pos);
        let segment_length = self.curve_length / segments as f32;
        for i in 0..segments {
            self.curve_points
                .push(self.position(self.t_from_distance(i as f32 * segment_length)));
        }
        self.curve_points.push(self.lut[LUT_SAMPLES - 1].pos);
    }

    pub fn create_curve_control_points(&self) -> [[Vec3; 4]; 1] {
        let start = &self.controls[0];
        let end = &self.controls[1];

        if let Some(control_points) = self.controls_override {
            [[start.pos, control_points[0], control_points[1], end.pos]]
        } else {
            let length = (end.pos - start.pos).length();
            [[
                start.pos,
                start.pos + start.forward * length * 0.5,
                end.pos + end.forward * length * 0.5,
                end.pos,
            ]]
        }
    }

    pub fn t_from_distance(&self, distance: f32) -> f32 {
        let mut idx = 0;
        for (i, x) in self.lut.iter().enumerate() {
            idx = i;
            if x.distance_along_curve > distance {
                break;
            }
        }

        // Out of bounds, so return the bound
        if idx == 0 {
            return self.lut[idx].t;
        }
        let l = self.lut[idx - 1];
        let r = self.lut[idx];

        let range = r.distance_along_curve - l.distance_along_curve;
        let ratio = (distance - l.distance_along_curve) / range;

        // Interpolate to a better distance -> t relation
        let mut t = l.t.lerp(r.t, ratio);
        let mut interval = ratio.min(1.0 - ratio);
        let pos = self.position(t);
        let distance_to_t = pos.distance(l.pos);
        let mut distance_delta_squared = distance - distance_to_t;
        distance_delta_squared = distance_delta_squared * distance_delta_squared;
        let distance_threshold = 1.0e-2 as f32;
        // let mut iter = 0;
        // Depending on our LUT samples, we might already be close enough and have no need
        // to further interpolate
        for _ in 0..100 {
            // iter += 1;
            if distance_delta_squared.abs() < distance_threshold {
                break;
            }

            let lmt = t - interval * 0.5;
            let rmt = t + interval * 0.5;
            let last_t = t;
            for x in [lmt, rmt] {
                let dist = pos.distance_squared(self.position(x));

                if dist < distance_delta_squared {
                    t = x;
                    distance_delta_squared = dist;
                }
            }

            // This means our guess is still the best, lower so we hit threshold
            if t == last_t {
                interval *= 0.5;
            } else {
                interval = (t - last_t).abs();
            }
        }
        // info!("Did {iter} iterations");

        t
    }

    /// Returns the nearest position to the spline, for rails this represents
    /// the center of the rail.
    /// Returns (t, pos)
    pub fn t_from_pos(&self, pos: &Vec3) -> f32 {
        // https://pomax.github.io/bezierinfo/#projections
        let mut min_idx = 0;
        let mut min_dist = f32::MAX;
        for (i, x) in self.lut.iter().enumerate() {
            let dist = pos.distance_squared(x.pos);
            if dist < min_dist {
                min_idx = i;
                min_dist = dist;
            }
        }

        // Interpolate to a closer value
        let mut t = self.lut[min_idx].t;
        let lt = if min_idx == 0 {
            0.0
        } else {
            self.lut[min_idx - 1].t
        };
        let rt = if min_idx == LUT_SAMPLES - 1 {
            1.0
        } else {
            self.lut[min_idx + 1].t
        };
        let mut interval = (t - lt).max(rt - t);
        let interval_threshold = 1.0e-2 / self.curve_points.len() as f32;
        // let mut iter = 0;
        for _ in 0..100 {
            // iter += 1;
            if interval < interval_threshold {
                break;
            }

            let lmt = t - interval * 0.5;
            let rmt = t + interval * 0.5;
            let last_t = t;
            for x in [lmt, rmt] {
                let dist = pos.distance_squared(self.position(x));
                if dist < min_dist {
                    t = x;
                    min_dist = dist;
                }
            }

            // This means our guess is still the best, lower so we hit threshold
            if t == last_t {
                interval *= 0.5;
            } else {
                interval = (t - last_t).abs();
            }
        }
        // info!("Did {iter} iterations");

        if t < 0.0 || t > 1.0 {
            t = t.clamp(0.0, 1.0);
        }
        if interval > interval_threshold {
            warn!(
                "Ended up with a t {} (last interval {}) which is above our threshold {}. Reduce precision",
                t, interval, interval_threshold
            );
        }

        t
    }

    pub fn position(&self, t: f32) -> Vec3 {
        self.curve.position(t)
    }

    pub fn forward(&self, t: f32) -> Dir3 {
        Dir3::new(self.curve.velocity(t).normalize()).unwrap()
    }

    /// Create left and right spline with pos as center
    pub fn split(&self, pos: &Vec3, gizmos: &mut Option<&mut Gizmos>) -> (Spline, Spline) {
        let t = self.t_from_pos(&pos);
        let pos = self.curve.sample(t).unwrap();
        let control_points = self.create_curve_control_points();
        let s = control_points[0][0];
        let sc = control_points[0][1];
        let ec = control_points[0][2];
        let e = control_points[0][3];

        // https://pomax.github.io/bezierinfo/#splitting
        let t1 = s + (sc - s) * t;
        let t2 = sc + (ec - sc) * t;
        let t3 = ec + (e - ec) * t;
        let start = [s, t1, t1 + (t2 - t1) * t, pos];
        let end = [pos, t2 + (t3 - t2) * t, t3, e];

        if let Some(gizmos) = gizmos {
            gizmos.sphere(self.curve.sample(t).unwrap(), 5.0, Color::BLACK);
            // gizmos.sphere(s, 5.0, Color::srgb(1.0, 0.0, 0.0));
            // gizmos.sphere(sc, 5.0, Color::srgb(0.0, 1.0, 0.0));
            // gizmos.sphere(ec, 5.0, Color::srgb(0.0, 0.0, 1.0));
            // gizmos.sphere(e, 5.0, Color::srgb(1.0, 1.0, 1.0));
            gizmos.sphere(t1, 5.0, Color::srgb(1.0, 0.0, 0.0));
            gizmos.sphere(t2, 5.0, Color::srgb(0.0, 1.0, 0.0));
            gizmos.sphere(t3, 5.0, Color::srgb(0.0, 0.0, 1.0));
            gizmos.line(start[0], start[1], Color::srgb(0.0, 0.0, 1.0));
            gizmos.line(start[1], start[2], Color::srgb(0.0, 0.0, 1.0));
            gizmos.line(start[2], start[3], Color::srgb(0.0, 0.0, 1.0));
            gizmos.line(end[0], end[1], Color::srgb(1.0, 0.0, 0.0));
            gizmos.line(end[1], end[2], Color::srgb(1.0, 0.0, 0.0));
            gizmos.line(end[2], end[3], Color::srgb(1.0, 0.0, 0.0));
        }

        let mut start_spline = Spline::default();
        start_spline.controls_override = Some([start[1], start[2]]);
        start_spline.set_controls([
            SplineControl {
                pos: start[0],
                forward: (start[1] - start[0]).normalize(),
            },
            SplineControl {
                pos: start[3],
                forward: (start[2] - start[3]).normalize(),
            },
        ]);

        let mut end_spline = Spline::default();
        end_spline.controls_override = Some([end[1], end[2]]);
        end_spline.set_controls([
            SplineControl {
                pos: end[0],
                forward: (end[1] - end[0]).normalize(),
            },
            SplineControl {
                pos: end[3],
                forward: (end[2] - end[3]).normalize(),
            },
        ]);

        (start_spline, end_spline)
    }
}

/// Represents the start and end of a spline, also knows as knots
#[derive(Default, Clone, Copy, Reflect, PartialEq)]
pub struct SplineControl {
    pub pos: Vec3,
    /// Points in the direction of the curve
    /// EX: for a horizontal curve the left control would point to the right
    /// and the right would point to the left
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

fn debug_spline(q: Query<&Spline>, mut gizmos: Gizmos) {
    q.iter().for_each(|spline| {
        let lut: Vec<Vec3> = spline.lut.iter().map(|x| x.pos).collect();
        gizmos.linestrip(lut.clone(), GREEN_500);
        for pos in lut {
            gizmos.sphere(Isometry3d::from_translation(pos.clone()), 0.2, BLUE_500);
        }
        gizmos.linestrip(spline.curve_points().clone(), RED_500);
    });
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

        let mut points = spline.curve_points().clone();
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

        let mut indices = Vec::<u32>::new();
        indices.reserve(quads * 6);

        for i in 0..quads {
            let offset = (i * 2) as u32;
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

        mesh.insert_indices(Indices::U32(indices));

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
