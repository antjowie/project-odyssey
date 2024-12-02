use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use iyes_perf_ui::prelude::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_debug);
        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins((
            PerfUiPlugin,
            bevy::diagnostic::FrameTimeDiagnosticsPlugin,
            bevy::diagnostic::EntityCountDiagnosticsPlugin,
            bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        ));
        app.add_plugins(
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Delete)),
        );
    }
}

fn setup_debug(mut c: Commands) {
    c.spawn(PerfUiCompleteBundle::default());
}
