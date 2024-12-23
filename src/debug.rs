use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::*;
use bevy_inspector_egui::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin);
        app.add_plugins(bevy_inspector_egui::DefaultInspectorConfigPlugin); // adds default options and `InspectorEguiImpl`s
        app.add_systems(
            Update,
            world_inspector_ui.run_if(input_toggle_active(false, KeyCode::Delete)),
        );
    }
}

fn world_inspector_ui(world: &mut World) {
    const DEFAULT_SIZE: (f32, f32) = (320., 320.);

    let egui_context = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .get_single(world);

    let Ok(egui_context) = egui_context else {
        return;
    };
    let mut egui_context = egui_context.clone();

    egui::Window::new("World Inspector")
        .default_size(DEFAULT_SIZE)
        .show(egui_context.get_mut(), |ui| {
            egui::ScrollArea::both().show(ui, |ui| {
                bevy_inspector::ui_for_world(world, ui);
                ui.allocate_space(ui.available_size());
            });
        });
}
