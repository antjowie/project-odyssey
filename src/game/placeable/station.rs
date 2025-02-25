use bevy_egui::*;

use super::*;

pub(super) fn station_plugin(app: &mut App) {
    app.add_systems(Startup, load_station_asset);
    app.add_systems(
        Update,
        (
            handle_station_placement
                .after(update_placeable_preview_on_rail_transform)
                .run_if(
                    in_player_state(PlayerState::Building)
                        .and(is_placeable_preview(Placeable::Station)),
                ),
            handle_selected_station,
        )
            .in_set(GameSet::Update),
    );
}

#[derive(Component)]
#[require(Placeable(||Placeable::Station), Name(|| Name::new("Station")), Selectable)]
pub struct Station;

#[derive(Resource)]
pub struct StationAsset {
    pub scene: Handle<Scene>,
    pub scale: Vec3,
}

fn load_station_asset(mut c: Commands, asset_server: Res<AssetServer>) {
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/station.glb"));

    c.insert_resource(StationAsset {
        scene,
        scale: Vec3::splat(1.6),
    });
}

fn handle_station_placement(
    mut c: Commands,
    mut q: Query<&ActionState<PlayerBuildAction>>,
    mut preview: Query<(
        &mut Transform,
        &mut PlaceablePreview,
        &PlaceablePreviewOnRail,
    )>,
    station: Res<StationAsset>,
    mut feedback: ResMut<CursorFeedback>,
) {
    if preview.is_empty() {
        return;
    }
    let mut preview = preview.single_mut();
    let input = q.single_mut();

    preview.1.valid = false;
    if preview.2.rail.is_none() {
        feedback
            .entries
            .push(CursorFeedbackData::default().with_error("Not on rail".to_owned()));
    } else {
        preview.1.valid = true;
    }

    if preview.1.valid && input.just_pressed(&PlayerBuildAction::Interact) {
        c.spawn((
            Station,
            preview.0.clone().with_scale(station.scale),
            SceneRoot(station.scene.clone()),
        ))
        .observe(on_destroy_default);
    }
}

fn handle_selected_station(
    query: Query<&Station, With<Selected>>,
    trains: Query<(&Train, &Name)>,
    mut contexts: EguiContexts,
) {
    // if query.is_empty() {
    //     return;
    // }

    let trains = trains.iter().map(|x| x.1).collect::<Vec<_>>();
    let ctx = contexts.ctx_mut();
    egui::Window::new("Station").show(ctx, |ui| {
        ui.label(format!("Trains: \n{:#?}", trains));
    });
}
