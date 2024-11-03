use bevy::input::common_conditions::{input_just_pressed, input_toggle_active};
use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use iyes_perf_ui::prelude::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_debug);
        app.add_systems(Update, close.run_if(input_just_pressed(KeyCode::Escape)));
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

    fn name(&self) -> &str {
        "DebugPlugin"
    }
}

fn close(mut exit: EventWriter<AppExit>) {
    exit.send(AppExit::Success);
}

fn setup_debug(mut c: Commands) {
    c.spawn(PerfUiCompleteBundle::default());
}
