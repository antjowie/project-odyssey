use bevy_egui::*;

use super::*;

pub(super) fn station_plugin(app: &mut App) {
    app.add_systems(Startup, load_station_asset);
    app.add_systems(
        Update,
        (
            handle_station_placement.run_if(
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
pub struct Station {}

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
    mut q: Query<(&mut PlayerCursor, &ActionState<PlayerBuildAction>)>,
    mut preview: Query<(&mut Transform, &mut PlaceablePreview)>,
    rails: Query<&Spline, With<Rail>>,
    mut ray_cast: MeshRayCast,
    station: Res<StationAsset>,
    mut feedback: ResMut<CursorFeedback>,
) {
    if preview.is_empty() {
        return;
    }
    let mut preview = preview.single_mut();

    let (mut cursor, input) = q.single_mut();
    let mut pos = cursor.build_pos;
    let mut spline_forward = preview.0.forward();
    let mut target_rail = None;
    let mut target_spline = None;

    let hit = get_closest_rail(cursor.ray, &mut ray_cast, &rails);
    if let Some(hit) = &hit {
        if let Ok(spline) = rails.get(hit.0) {
            let t = spline.t_from_pos(&pos);
            pos = spline.position(t);
            spline_forward = spline.forward(t);
            cursor.manual_rotation = 0.0;

            let mut align_to_right = spline_forward.dot(preview.0.forward().as_vec3()) > 0.;
            if input.just_pressed(&PlayerBuildAction::Rotate) {
                align_to_right = !align_to_right;
            }

            spline_forward = if align_to_right {
                spline_forward
            } else {
                Dir3::new(spline_forward.as_vec3() * -1.0).unwrap()
            };

            target_rail = Some(hit.0);
            target_spline = Some(spline);
        }
    } else {
        spline_forward = Quat::from_rotation_y(cursor.manual_rotation) * spline_forward;
    }

    preview.0.translation = pos;
    preview.0.look_at(pos + spline_forward.as_vec3(), Vec3::Y);

    // TODO: Overlap check for other trains
    let collide_with_other = false;
    // let collide_with_other = hit.is_some() && Collider::int
    //     && spatial_query
    //         .shape_intersections(
    //             &train.collider,
    //             preview.0.translation,
    //             preview.0.rotation,
    //             &SpatialQueryFilter::default(),
    //         )
    //         .len()
    //         == 0;

    preview.1.valid = false;
    if hit.is_none() {
        feedback
            .entries
            .push(CursorFeedbackData::default().with_error("Not on rail".to_owned()));
    } else {
        preview.1.valid = true;
    }

    if preview.1.valid && input.just_pressed(&PlayerBuildAction::Interact) {
        c.spawn((
            Station {},
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
