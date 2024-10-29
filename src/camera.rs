use bevy::prelude::*;
use derivative::Derivative;
use leafwing_input_manager::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_camera);
        app.add_plugins(InputManagerPlugin::<CameraAction>::default());
        app.register_type::<PanOrbitCameraState>();
        app.register_type::<PanOrbitCameraSettings>();
    }

    fn name(&self) -> &str {
        "CameraPlugin"
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
            .with_axis(CameraAction::Zoom, MouseScrollAxis::Y.inverted())
    }
}

#[derive(Bundle, Default)]
pub struct PanOrbitCameraBundle {
    pub camera: Camera3dBundle,
    pub state: PanOrbitCameraState,
    pub settings: PanOrbitCameraSettings,
    pub input: InputManagerBundle<CameraAction>,
}

#[derive(Reflect, Component, Derivative)]
#[derivative(Default)]
pub struct PanOrbitCameraState {
    center: Vec3,
    velocity: Vec3,
    #[derivative(Default(value = "50.0"))]
    radius: f32,
    #[derivative(Default(value = "50.0"))]
    desired_radius: f32,
    #[derivative(Default(value = "-0.7"))]
    pitch: f32,
    yaw: f32,
}

#[derive(Reflect, Component)]
pub struct PanOrbitCameraSettings {
    acceleration: f32,
    max_speed: f32,
    orbit_sensitivity: f32,
    radius_sensitivity: f32,
    min_radius: f32,
    max_radius: f32,
}

impl Default for PanOrbitCameraSettings {
    fn default() -> Self {
        let mut value = PanOrbitCameraSettings {
            acceleration: 1000.0,
            max_speed: 100.0,
            orbit_sensitivity: 0.01,
            radius_sensitivity: 10.0,
            min_radius: 10.0,
            max_radius: 1000.0,
        };

        // Not sure why, but for some reason on wasm it is just way faster
        if cfg!(target_arch = "wasm32") {
            value.radius_sensitivity *= 0.1;
        }

        value
    }
}

fn update_camera(
    mut q: Query<(
        &ActionState<CameraAction>,
        &PanOrbitCameraSettings,
        &mut Transform,
        &mut PanOrbitCameraState,
    )>,
    time: Res<Time>,
) {
    for (&ref input, &ref settings, mut t, mut state) in &mut q {
        // Calculate rotation
        let direction = input.axis_pair(&CameraAction::Pan) * settings.orbit_sensitivity;
        state.yaw += direction.x;
        state.pitch += direction.y;
        state.pitch = state
            .pitch
            .clamp(-89.0_f32.to_radians(), -10.0_f32.to_radians());
        let rotation = Quat::from_euler(EulerRot::YXZ, state.yaw, state.pitch, 0.0);

        // Calculate translation
        let direction = input.axis_pair(&CameraAction::Translate);
        let direction =
            Quat::from_axis_angle(Vec3::Y, state.yaw) * Vec3::new(direction.x, 0.0, direction.y);
        let desired_velocity = direction.normalize_or_zero() * settings.max_speed;

        state.velocity = state.velocity.move_towards(
            desired_velocity,
            settings.acceleration * time.delta_seconds(),
        );
        state.center = state.center + state.velocity * time.delta_seconds();

        // Calculate radius
        state.desired_radius += input.value(&CameraAction::Zoom) * settings.radius_sensitivity;
        state.desired_radius = state
            .desired_radius
            .clamp(settings.min_radius, settings.max_radius);
        let alpha = (time.delta_seconds() * 50.0).min(1.0);

        state.radius = state.radius.lerp(state.desired_radius, alpha);

        // Apply state to transform
        let offset = rotation * Vec3::Z * state.radius;
        t.translation = state.center + offset;
        t.rotation = rotation;
    }
}
