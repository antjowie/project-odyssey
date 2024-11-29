use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_pan_orbit_camera.run_if(any_with_component::<PanOrbitCameraState>),
        );
        app.add_plugins(InputManagerPlugin::<CameraAction>::default());
        app.register_type::<PanOrbitCameraState>();
        app.register_type::<PanOrbitCameraSettings>();
    }
}

// This is the list of "things in the game I want to be able to do based on input"
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CameraAction {
    #[actionlike(DualAxis)]
    Translate,
    #[actionlike(DualAxis)]
    Pan,
    #[actionlike(Axis)]
    Zoom,
}

impl CameraAction {
    pub fn default_player_mapping() -> InputMap<CameraAction> {
        InputMap::default()
            .with_dual_axis(
                CameraAction::Translate,
                KeyboardVirtualDPad::WASD.inverted_y(),
            )
            .with_dual_axis(
                CameraAction::Pan,
                DualAxislikeChord::new(MouseButton::Right, MouseMove::default().inverted()),
            )
            // We use Digital to avoid inconsistencies between platform
            // On windows our pixel value is 1, but on web it is 100 (or 125 if you use Windows scaling)
            .with_axis(CameraAction::Zoom, MouseScrollAxis::Y.inverted().digital())
    }
}

#[derive(Bundle, Default)]
pub struct PanOrbitCameraBundle {
    pub camera: Camera3dBundle,
    pub state: PanOrbitCameraState,
    pub settings: PanOrbitCameraSettings,
    pub input: InputManagerBundle<CameraAction>,
}

#[derive(Reflect, Component)]
pub struct PanOrbitCameraState {
    center: Vec3,
    velocity: Vec3,
    radius: f32,
    // Zoom is a normalized value representing the user's desired amount of content to see.
    // Radius is not linear and calculated from zoom.
    // 0 zoom == max radius
    // 1 zoom == min radius
    zoom: f32,
    pitch: f32,
    yaw: f32,
}

impl Default for PanOrbitCameraState {
    fn default() -> Self {
        let default_settings = PanOrbitCameraSettings::default();
        const DEFAULT_ZOOM: f32 = 0.5;
        PanOrbitCameraState {
            center: Vec3::ZERO,
            velocity: Vec3::ZERO,
            zoom: DEFAULT_ZOOM,
            radius: calculate_desired_radius(
                DEFAULT_ZOOM,
                default_settings.min_radius,
                default_settings.max_radius,
            ),
            pitch: -45.0_f32.to_radians(),
            yaw: 0.0,
        }
    }
}

#[derive(Reflect, Component)]
pub struct PanOrbitCameraSettings {
    acceleration: f32,
    // The max speed we want when fully zoomed in
    max_speed_zoomed: f32,
    // The max speed we want when fully zoomed out
    max_speed: f32,
    orbit_sensitivity: f32,
    zoom_sensitivity: f32,
    min_radius: f32,
    max_radius: f32,
}

impl Default for PanOrbitCameraSettings {
    fn default() -> Self {
        PanOrbitCameraSettings {
            acceleration: 1000.0,
            max_speed_zoomed: 10.0,
            max_speed: 100.0,
            orbit_sensitivity: 0.01,
            zoom_sensitivity: 0.1,
            min_radius: 10.0,
            max_radius: 1000.0,
        }
    }
}

fn calculate_desired_radius(zoom: f32, min_radius: f32, max_radius: f32) -> f32 {
    min_radius.lerp(max_radius, zoom.powi(2))
}

fn update_pan_orbit_camera(
    // mut gizmos: Gizmos,
    windows: Query<&Window>,
    mut q: Query<(
        &Camera,
        &ActionState<CameraAction>,
        &PanOrbitCameraSettings,
        &mut Transform,
        &mut PanOrbitCameraState,
        &GlobalTransform,
    )>,
    time: Res<Time>,
) {
    let window = windows.single();

    q.iter_mut().for_each(
        |(camera, input, settings, mut t, mut state, global_transform)| {
            // Calculate rotation
            let direction = input.axis_pair(&CameraAction::Pan) * settings.orbit_sensitivity;
            state.yaw += direction.x;
            state.pitch += direction.y;
            state.pitch = state
                .pitch
                .clamp(-89.0_f32.to_radians(), -10.0_f32.to_radians());
            let rotation = Quat::from_euler(EulerRot::YXZ, state.yaw, state.pitch, 0.0);

            // Calculate radius
            state.zoom += input.value(&CameraAction::Zoom) * settings.zoom_sensitivity;
            state.zoom = state.zoom.clamp(0.0, 1.0);

            let desired_radius =
                calculate_desired_radius(state.zoom, settings.min_radius, settings.max_radius);
            const RADIUS_LERP_RATE: f32 = 50.0;
            let alpha = (time.delta_seconds() * RADIUS_LERP_RATE).min(1.0);

            let radius_delta = state.radius;
            state.radius = state.radius.lerp(desired_radius, alpha);
            let radius_delta = state.radius - radius_delta;

            // If we zoom with mkb we want to zoom towards cursor pos
            let mut center_zoom_offset = Vec3::ZERO;
            if radius_delta != 0.0 {
                // Check if cursor is in window
                if let Some(ray) = window
                    .cursor_position()
                    .and_then(|cursor| camera.viewport_to_world(global_transform, cursor))
                {
                    // Check if cursor intersects ground
                    if let Some(len) =
                        ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
                    {
                        let norm_radius_delta =
                            -radius_delta / (state.radius + settings.min_radius);

                        let cursor = ray.origin + ray.direction * len;
                        let mut center_to_cursor = cursor - state.center;
                        const MAX_CENTER_TO_CURSOR_LENGTH: f32 = 100.0;
                        center_to_cursor =
                            center_to_cursor.clamp_length_max(MAX_CENTER_TO_CURSOR_LENGTH);
                        center_zoom_offset = center_to_cursor * norm_radius_delta;

                        // gizmos.ray(state.center, center_to_cursor, RED);
                        // gizmos.sphere(cursor, Quat::IDENTITY, 10.0, RED);
                        // gizmos.sphere(state.center, Quat::IDENTITY, 10.0, GREEN);
                    }
                }
            }

            // Calculate translation
            let direction = input.axis_pair(&CameraAction::Translate);
            let direction = Quat::from_axis_angle(Vec3::Y, state.yaw)
                * Vec3::new(direction.x, 0.0, direction.y);
            let desired_velocity = direction.normalize_or_zero()
                * settings
                    .max_speed_zoomed
                    .lerp(settings.max_speed, state.zoom);

            state.velocity = state.velocity.move_towards(
                desired_velocity,
                settings.acceleration * time.delta_seconds(),
            );

            state.center =
                state.center + state.velocity * time.delta_seconds() + center_zoom_offset;

            // Apply state to transform
            let offset = rotation * Vec3::Z * state.radius;
            t.translation = state.center + offset;
            t.rotation = rotation;
        },
    );
}
