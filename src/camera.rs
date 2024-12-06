use crate::game::*;
use bevy::{
    math::vec3,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use leafwing_input_manager::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            // Grab Cursor will likely need a software cursor, cuz the harware impl seems to not have a lot of parity
            // (update_pan_orbit_camera, grab_cursor)
            //     .run_if(any_with_component::<PanOrbitCameraState>),
            (update_pan_orbit_camera).run_if(any_with_component::<PanOrbitCameraState>),
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
    #[actionlike(DualAxis)]
    Orbit,
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
                DualAxislikeChord::new(MouseButton::Middle, MouseMove::default().inverted()),
            )
            .with_dual_axis(
                CameraAction::Orbit,
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
    pub center: Vec3,
    pub velocity: Vec3,
    pub radius: f32,
    // Zoom is a normalized value representing the user's desired amount of content to see.
    // Radius is not linear and calculated from zoom.
    // 0 zoom == max radius
    // 1 zoom == min radius
    pub zoom: f32,
    pub pitch: f32,
    pub yaw: f32,
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
    pub acceleration: f32,
    // The max speed we want when fully zoomed in
    pub max_speed_zoomed: f32,
    // The max speed we want when fully zoomed out
    pub max_speed: f32,
    pub orbit_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub min_radius: f32,
    pub max_radius: f32,
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

#[derive(Default)]
struct CursorPos(Option<Vec2>);

fn grab_cursor(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut cursor_pos: Local<CursorPos>,
) {
    let mut window = windows.single_mut();
    let is_grabbed = window.cursor.grab_mode == CursorGrabMode::Locked;
    let should_grab = buttons.pressed(MouseButton::Right);

    if is_grabbed != should_grab {
        if should_grab {
            cursor_pos.0 = window.cursor_position();
            window.cursor.grab_mode = CursorGrabMode::Locked;
            // window.cursor.visible = false;
            info!("{:?}", cursor_pos.0);
        } else {
            info!("{:?}", cursor_pos.0);
            window.cursor.grab_mode = CursorGrabMode::None;
            // window.cursor.visible = true;
            window.set_cursor_position(cursor_pos.0);
            cursor_pos.0 = None;
        }
    }
    window.set_cursor_position(cursor_pos.0);
}

fn update_pan_orbit_camera(
    // mut gizmos: Gizmos,
    mut q: Query<(
        &ActionState<CameraAction>,
        &PanOrbitCameraSettings,
        &mut Transform,
        &mut PanOrbitCameraState,
    )>,
    player_cursors: Query<&PlayerCursor>,
    time: Res<Time>,
) {
    let player_cursor = player_cursors.single();

    q.iter_mut()
        .for_each(|(input, settings, mut t, mut state)| {
            // Calculate rotation
            let direction = input.axis_pair(&CameraAction::Orbit) * settings.orbit_sensitivity;
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
                let norm_radius_delta = -radius_delta / (state.radius + settings.min_radius);

                let mut center_to_cursor = player_cursor.world_pos - state.center;
                const MAX_CENTER_TO_CURSOR_LENGTH: f32 = 100.0;
                center_to_cursor = center_to_cursor.clamp_length_max(MAX_CENTER_TO_CURSOR_LENGTH);
                center_zoom_offset = center_to_cursor * norm_radius_delta;

                // gizmos.ray(state.center, center_to_cursor, RED);
                // gizmos.sphere(cursor, Quat::IDENTITY, 10.0, RED);
                // gizmos.sphere(state.center, Quat::IDENTITY, 10.0, GREEN);
            }

            // Calculate translation
            let direction = input.axis_pair(&CameraAction::Translate);
            let forward = Quat::from_axis_angle(Vec3::Y, state.yaw);
            let direction = forward * vec3(direction.x, 0.0, direction.y);
            let desired_velocity = direction.normalize_or_zero()
                * settings
                    .max_speed_zoomed
                    .lerp(settings.max_speed, state.zoom);

            state.velocity = state.velocity.move_towards(
                desired_velocity,
                settings.acceleration * time.delta_seconds(),
            );

            let pan = input.axis_pair(&CameraAction::Pan);
            let pan = forward * vec3(pan.x, 0.0, pan.y) * state.zoom;
            state.center =
                state.center + state.velocity * time.delta_seconds() + center_zoom_offset + pan;

            // Apply state to transform
            let offset = rotation * Vec3::Z * state.radius;
            t.translation = state.center + offset;
            t.rotation = rotation;
        });
}
