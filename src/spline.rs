use core::f32;

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
        // app.add_systems(Update, _debug_spline);
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
            min_segment_length: 1.0,
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
    pub fn with_min_segment_length(mut self, length: f32) -> Self {
        self.min_segment_length = length;
        self
    }

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

    /// Returns a new t based on an amount of movement along the curve
    /// You should use this for any linear movement over the curve such as a train
    pub fn traverse(&self, t: f32, movement: f32) -> f32 {
        let mut idx = 0;
        for (i, x) in self.lut.iter().enumerate() {
            idx = i;
            if x.t > t {
                break;
            }
        }

        // Out of bounds, so return the bound
        if idx == 0 {
            return 0.0;
        }
        let l = self.lut[idx - 1];
        let r = self.lut[idx];

        // We calc a ratio of distance (this tells us how much % we moved along the segment)
        // We then take that ratio and add it to the current t
        // This makes a linear assumption between t and distance, which is not valid
        // but for our small intervals is good enough, more importantly, we won't run
        // into issues where we're off by very small float values, and our train ends
        // up moving backwards
        let dist_range = r.distance_along_curve - l.distance_along_curve;
        let dist_ratio = movement / dist_range;

        let t_range = r.t - l.t;
        t + t_range * dist_ratio
    }

    /// Since these are all approximations, you can run into float issues once
    /// these get to close. For example:
    /// distance(t) = 10.0
    /// t_from_distance(t + 1.0e-5) = 9.99999
    /// You would expect it to go further right, but we're approximating so it's not always valid
    /// Because of this, it is NOT safe to try and write code that relies on such assumptions
    ///
    /// If you have such a need (such as moving a train along a track) use the traverse method
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
        let distance_threshold = 1.0e-2 / self.curve_points.len() as f32;
        // let mut iter = 0;
        // Depending on our LUT samples, we might already be close enough and have no need
        // to further interpolate
        for _ in 0..100 {
            // iter += 1;
            if distance_delta_squared < distance_threshold {
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
        let interval_threshold = 1.0e-4 / self.curve_points.len() as f32;
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

    pub fn distance_along_curve(&self, t: f32) -> f32 {
        let mut idx = 0;
        for (i, x) in self.lut.iter().enumerate() {
            idx = i;
            if x.t > t {
                break;
            }
        }

        // Out of bounds, so return the bound
        if idx == 0 {
            return self.lut[idx].distance_along_curve;
        }
        let l = self.lut[idx - 1];
        let r = self.lut[idx];

        let range = r.t - l.t;
        let ratio = (t - l.t) / range;

        // Interpolate to a better t -> distance relation
        l.distance_along_curve.lerp(r.distance_along_curve, ratio)
    }

    /// Create left and right spline with pos as center
    pub fn split(&self, pos: &Vec3, gizmos: Option<&mut Gizmos>) -> (Spline, Spline) {
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
                forward: Dir3::new(start[1] - start[0]).unwrap(),
            },
            SplineControl {
                pos: start[3],
                forward: Dir3::new(start[2] - start[3]).unwrap(),
            },
        ]);

        let mut end_spline = Spline::default();
        end_spline.controls_override = Some([end[1], end[2]]);
        end_spline.set_controls([
            SplineControl {
                pos: end[0],
                forward: Dir3::new(end[1] - end[0]).unwrap(),
            },
            SplineControl {
                pos: end[3],
                forward: Dir3::new(end[2] - end[3]).unwrap(),
            },
        ]);

        (start_spline, end_spline)
    }
}

/// Represents the start and end of a spline, also knows as knots
#[derive(Clone, Copy, Reflect, PartialEq)]
pub struct SplineControl {
    pub pos: Vec3,
    /// Points in the direction of the curve
    /// EX: for a horizontal curve the left control would point to the right
    /// and the right would point to the left
    pub forward: Dir3,
}

impl Default for SplineControl {
    fn default() -> Self {
        Self {
            pos: Default::default(),
            forward: Dir3::new_unchecked(Vec3::NEG_Z),
        }
    }
}

/// Samples a spline and generates a mesh from it
#[derive(Component, Reflect)]
#[require(Spline, Mesh3d)]
pub struct SplineMesh {
    pub width: f32,
    pub height: f32,
    pub source_spline_data: Spline,
}

impl SplineMesh {
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl Default for SplineMesh {
    fn default() -> Self {
        Self {
            width: 2.,
            height: 0.5,

            source_spline_data: default(),
        }
    }
}

fn _debug_spline(q: Query<&Spline>, mut gizmos: Gizmos) {
    q.iter().for_each(|spline| {
        let lut: Vec<Vec3> = spline.lut.iter().map(|x| x.pos).collect();
        for pos in lut {
            gizmos.sphere(Isometry3d::from_translation(pos.clone()), 0.2, BLUE_500);
        }
        for i in 0..=spline.curve_length().ceil() as u32 {
            let distance = i as f32;
            let t = spline.t_from_distance(distance);
            let pos = spline.position(t);
            gizmos.sphere(Isometry3d::from_translation(pos.clone()), 0.3, GREEN_500);
            let pos = spline.position(spline.t_from_distance(spline.distance_along_curve(t)));
            gizmos.sphere(Isometry3d::from_translation(pos.clone()), 0.4, RED_500);
        }
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
        points.push(spline.controls()[1].pos - spline.controls()[1].forward.as_vec3());

        let mut vertices = Vec::new();
        let mut normal = Vec::new();
        vertices.reserve(points.len() * 2);
        normal.reserve(points.len() * 2);
        let width = spline_mesh.width * 0.5;
        let height = spline_mesh.height;

        points
            .iter()
            .zip(points.iter().skip(1))
            .for_each(|(sample, next)| {
                let forward = (next - sample).normalize();
                let side = forward.cross(Vec3::Y);

                let left = sample - side * width;
                let right = sample + side * width;

                // Up down left right
                vertices.push(left + Vec3::Y * height);
                vertices.push(right + Vec3::Y * height);
                vertices.push(left);
                vertices.push(right);
                vertices.push(left + Vec3::Y * height);
                vertices.push(left);
                vertices.push(right + Vec3::Y * height);
                vertices.push(right);
                let up = side.cross(forward);
                normal.push(up);
                normal.push(up);
                normal.push(-up);
                normal.push(-up);
                normal.push(-side);
                normal.push(-side);
                normal.push(side);
                normal.push(side);

                gizmos.arrow(*sample, sample + forward, Color::srgb(1., 0., 0.));
                gizmos.line(*sample, left, Color::srgb(0., 1., 0.));
                gizmos.line(*sample, right, Color::srgb(1., 0., 0.));
                gizmos.line(*sample, sample + up, Color::srgb(0., 0., 1.));
            });

        let segments = vertices.len() / 8 - 1;
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);

        let mut indices = Vec::<u32>::new();
        indices.reserve(segments * 24);

        for i in 0..segments {
            let offset = (i * 8) as u32;
            indices.append(
                &mut [
                    // Up
                    offset + 0,
                    offset + 1,
                    offset + 8,
                    offset + 1,
                    offset + 9,
                    offset + 8,
                    // Down
                    offset + 2,
                    offset + 10,
                    offset + 3,
                    offset + 3,
                    offset + 10,
                    offset + 11,
                    // Left
                    offset + 5,
                    offset + 4,
                    offset + 12,
                    offset + 5,
                    offset + 12,
                    offset + 13,
                    // Right
                    offset + 7,
                    offset + 14,
                    offset + 6,
                    offset + 7,
                    offset + 15,
                    offset + 14,
                    // offset + 0,
                    // offset + 1,
                    // offset + 2,
                    // offset + 1,
                    // offset + 3,
                    // offset + 2,
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
