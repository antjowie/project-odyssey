use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use iyes_perf_ui::prelude::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_debug);
        app.add_plugins(PerfUiPlugin);
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin);
        app.add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin);
        app.add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin);
        app.add_plugins(
            WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Delete)),
        );
    }
}

fn setup_debug(mut c: Commands) {
    c.spawn(PerfUiCompleteBundle::default());
}
