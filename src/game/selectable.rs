use super::*;

pub(super) fn selectable_plugin(app: &mut App) {
    app.init_resource::<SelectedStore>();
    app.add_event::<SelectedChangedEvent>();
    app.add_systems(Startup, init_selected_materials);
    app.add_systems(
        Update,
        (
            unselect_when_leaving_view_state.run_if(on_event::<PlayerStateChangedEvent>),
            on_selected_changed_event.run_if(on_event::<SelectedChangedEvent>),
        ),
    );
    app.add_systems(
        Update,
        handle_selected_input_in_view_state.run_if(
            not(on_event::<SelectedChangedEvent>),
            // handle_selected_input_in_view_state.run_if(
            //     in_player_state(PlayerState::Viewing).and(not(on_event::<SelectedChangedEvent>)),
        ),
    );
}

#[derive(Event)]
pub struct SelectedChangedEvent(pub Option<Entity>);

#[derive(Resource)]
struct SelectedMaterial(Handle<StandardMaterial>);

#[derive(Resource, Default)]
struct SelectedStore {
    selected: Option<Entity>,
}

#[derive(Component)]
pub struct Selected(Handle<StandardMaterial>);

#[derive(Component, Default)]
pub struct Selectable;

fn init_selected_materials(mut c: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    c.insert_resource(SelectedMaterial(materials.add(StandardMaterial {
        base_color: Color::Srgba(BLUE_300),
        ..default()
    })));
}

fn handle_selected_input_in_view_state(
    q: Query<(), With<Selectable>>,
    player: Single<(&PlayerCursor, &ActionState<PlayerViewAction>)>,
    mut ray_cast: MeshRayCast,
    mut ev: EventWriter<SelectedChangedEvent>,
) {
    let (cursor, input) = player.into_inner();
    if input.just_pressed(&PlayerViewAction::Interact) {
        let hits = ray_cast.cast_ray(cursor.ray, &RayCastSettings::default().always_early_exit());
        let mut e = None;
        if hits.len() > 0 && q.contains(hits[0].0) {
            e = Some(hits[0].0);
        }
        ev.send(SelectedChangedEvent(e));
    }
}

fn unselect_when_leaving_view_state(
    mut ev_state: EventReader<PlayerStateChangedEvent>,
    mut ev: EventWriter<SelectedChangedEvent>,
) {
    for e in ev_state.read() {
        if e.new_state == PlayerState::Viewing {
            ev.send(SelectedChangedEvent(None));
        }
    }
}

fn on_selected_changed_event(
    mut c: Commands,
    mut q: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&Selected>), With<Selectable>>,
    mut selected: ResMut<SelectedStore>,
    material: Res<SelectedMaterial>,
    mut ev: EventReader<SelectedChangedEvent>,
) {
    for e in ev.read() {
        let e = e.0;
        if selected.selected != e {
            if let Some(e) = selected.selected {
                if let Some(mut c) = c.get_entity(e) {
                    let (mut handle, select) = q.get_mut(e).unwrap();
                    handle.0 = select
                        .expect("Our stored selected entity has no selected component")
                        .0
                        .clone();

                    c.remove::<Selected>();
                }
            }
            selected.selected = e;
            if let Some(e) = selected.selected {
                let (mut handle, _) = q.get_mut(e).unwrap();
                let select = Selected(handle.0.clone());
                handle.0 = material.0.clone();
                c.entity(e).insert(select);
            }
        }
    }
}
