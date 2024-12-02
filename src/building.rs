use bevy::{ecs::world::Command, pbr::NotShadowCaster, prelude::*};

pub mod rail;
pub use rail::*;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_assets);
        app.add_systems(
            Update,
            (
                on_add_build_preview_component,
                update_build_preview_material,
            )
                .chain(),
        );
        app.add_systems(PostUpdate, on_remove_build_preview_component);
    }
}

#[derive(Resource, PartialEq)]
pub struct BuildingPreviewMaterialValid(Handle<StandardMaterial>);

#[derive(Resource, PartialEq)]
pub struct BuildingPreviewMaterialInvalid(Handle<StandardMaterial>);

fn load_assets(
    mut c: Commands,
    meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    c.insert_resource(BuildingPreviewMaterialValid(materials.add(
        StandardMaterial {
            base_color: Color::srgba(0.2, 1.0, 0.2, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
    )));
    c.insert_resource(BuildingPreviewMaterialInvalid(materials.add(
        StandardMaterial {
            base_color: Color::srgba(1.0, 0.2, 0.2, 0.5),
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
    )));
    c.insert_resource(rail::create_rail_asset(meshes, materials));
}

#[derive(Component, Default)]
pub struct Building;

#[derive(Component, Default)]
pub struct BuildingPreview {
    orig_material: Handle<StandardMaterial>,
    pub valid: bool,
}

fn on_add_build_preview_component(
    mut c: Commands,
    mut q: Query<
        (Entity, &Handle<StandardMaterial>, &mut BuildingPreview),
        (With<Building>, Added<BuildingPreview>),
    >,
) {
    q.iter_mut().for_each(|(e, handle, mut preview)| {
        c.entity(e).insert(NotShadowCaster);
        preview.orig_material = handle.clone();
    });
}

fn on_remove_build_preview_component(
    mut c: Commands,
    mut q: Query<(&mut Handle<StandardMaterial>, &BuildingPreview), With<Building>>,
    mut removed: RemovedComponents<BuildingPreview>,
) {
    for entity in removed.read() {
        if let Ok((mut handle, preview)) = q.get_mut(entity) {
            c.entity(entity).remove::<NotShadowCaster>();
            *handle = preview.orig_material.clone();
        }
    }
}

fn update_build_preview_material(
    mut q: Query<(&mut Handle<StandardMaterial>, &BuildingPreview)>,
    valid: Res<BuildingPreviewMaterialValid>,
    invalid: Res<BuildingPreviewMaterialInvalid>,
) {
    q.iter_mut().for_each(|(mut handle, preview)| {
        if preview.valid && *handle != valid.0 {
            *handle = valid.0.clone();
        } else if !preview.valid && *handle != invalid.0 {
            *handle = invalid.0.clone();
        };
    });
}
