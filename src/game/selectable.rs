use super::*;

pub(super) fn selectable_plugin(app: &mut App) {
    app.init_resource::<SelectedStore>();
    app.add_systems(Startup, init_selected_materials);
    app.add_systems(Update, update_selected);
}

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

fn update_selected(
    mut c: Commands,
    mut q: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&Selected>), With<Selectable>>,
    player: Single<(&PlayerState, &PlayerCursor, &ActionState<PlayerViewAction>)>,
    mut ray_cast: MeshRayCast,
    mut selected: ResMut<SelectedStore>,
    material: Res<SelectedMaterial>,
) {
    let (state, cursor, input) = player.into_inner();
    if *state != PlayerState::Viewing {
        if let Some(e) = selected.selected {
            if let Some(mut c) = c.get_entity(e) {
                let (mut handle, select) = q.get_mut(e).unwrap();
                handle.0 = select.unwrap().0.clone();

                c.remove::<Selected>();
            }
        }
    } else if input.just_pressed(&PlayerViewAction::Interact) {
        let hits = ray_cast.cast_ray(cursor.ray, &RayCastSettings::default().always_early_exit());
        let mut e = None;
        if hits.len() > 0 && q.contains(hits[0].0) {
            e = Some(hits[0].0);
        }

        if selected.selected != e {
            if let Some(e) = selected.selected {
                if let Some(mut c) = c.get_entity(e) {
                    let (mut handle, select) = q.get_mut(e).unwrap();
                    handle.0 = select.unwrap().0.clone();

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
