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
pub struct Selected;

#[derive(Component)]
struct SelectedOriginalMaterial(Handle<StandardMaterial>);

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
    parents: Query<&Parent>,
    player: Single<(&PlayerCursor, &ActionState<PlayerViewAction>)>,
    mut ray_cast: MeshRayCast,
    mut ev: EventWriter<SelectedChangedEvent>,
) {
    let (cursor, input) = player.into_inner();
    if input.just_pressed(&PlayerViewAction::Interact) {
        let hits = ray_cast.cast_ray(cursor.ray, &RayCastSettings::default().always_early_exit());
        let mut e = None;
        if hits.len() > 0 {
            if q.contains(hits[0].0) {
                e = Some(hits[0].0);
            } else {
                e = parents.iter_ancestors(hits[0].0).find(|x| q.contains(*x));
            }
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
    children: Query<&Children>,
    mut q: Query<(
        &mut MeshMaterial3d<StandardMaterial>,
        Option<&SelectedOriginalMaterial>,
    )>,
    mut selected: ResMut<SelectedStore>,
    material: Res<SelectedMaterial>,
    mut ev: EventReader<SelectedChangedEvent>,
) {
    for e in ev.read() {
        let e = e.0;
        if selected.selected != e {
            if let Some(e) = selected.selected {
                let mut handle = |entity| {
                    if let Ok((mut mat, orig)) = q.get_mut(entity) {
                        mat.0 = orig.unwrap().0.clone();
                        c.entity(entity).remove::<SelectedOriginalMaterial>();
                    }
                };

                handle(e);
                children.iter_descendants(e).for_each(handle);

                c.entity(e).remove::<Selected>();
            }
            selected.selected = e;
            if let Some(e) = selected.selected {
                let mut handle = |entity| {
                    if let Ok((mut mat, _orig)) = q.get_mut(entity) {
                        c.entity(entity)
                            .insert(SelectedOriginalMaterial(mat.0.clone()));
                        mat.0 = material.0.clone();
                    }
                };

                handle(e);
                children.iter_descendants(e).for_each(handle);
                c.entity(e).insert(Selected);
            }
        }
    }
}
