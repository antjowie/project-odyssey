//! Loads our initial world
use super::*;
use bevy::pbr::{DirectionalLightShadowMap, NotShadowCaster};

pub(super) fn world_plugin(app: &mut App) {
    app.add_systems(Startup, spawn_test_world);
}

fn spawn_test_world(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // mut config_store: ResMut<GizmoConfigStore>,
) {
    // Draw gizmos over everything
    // for (_, config, _) in config_store.iter_mut() {
    //     config.depth_bias = -1.;
    // }
    // --- Gameplay
    // Player State
    c.spawn((Name::new("PlayerState"), PlayerState::default()));

    // Camera
    c.spawn((
        Name::new("PanOrbitCamera"),
        PanOrbitCamera::default(),
        DistanceFog {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::from_visibility_colors(
                PanOrbitCameraSettings::default().max_radius * 5.0, // distance in world units up to which objects retain visibility (>= 5% contrast)
                Color::srgb(0.35, 0.5, 0.66), // atmospheric extinction color (after light is lost due to absorption by atmospheric particles)
                Srgba::hex("#C3EEFA").unwrap().into(), // atmospheric inscattering color (light gained due to scattering from the sun)
            ),
        },
    ));

    // Sun
    c.insert_resource(DirectionalLightShadowMap { size: 4096 });
    // These options below make my laptop die
    // c.insert_resource(DirectionalLightShadowMap { size: 8192 });
    // c.insert_resource(DirectionalLightShadowMap { size: 8192 + 4096 });

    c.spawn((
        Name::new("DirectionalLight"),
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(-0.8, -0.5, 0.65), Vec3::Y),
    ));

    c.spawn((
        Name::new("Skybox"),
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.529, 0.808, 0.922),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(100_000_000.0)),
        NotShadowCaster,
    ));

    // If plane is too big shadows bug out on AMD hardware
    // https://github.com/bevyengine/bevy/issues/6542
    c.spawn((
        Name::new("Floor"),
        Transform::default(),
        Visibility::Visible,
    ))
    .with_children(|parent| {
        const SIZE: f32 = 1_000.0;
        let mesh_handle = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(SIZE)));
        // Slight grey-beige color for a natural looking ground

        let concrete_color = Color::srgb(0.75, 0.74, 0.72);
        // let dirt_color = Color::srgb(0.60, 0.51, 0.39);
        // let grass_color = Color::srgb(0.45, 0.55, 0.35);
        let material_handle = materials.add(concrete_color);
        for x in -10..=10 {
            for z in -10..=10 {
                parent.spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(material_handle.clone()),
                    PickingBehavior::IGNORE,
                    Transform::from_translation(Vec3::new(
                        x as f32 * SIZE * 2.0,
                        0.0,
                        z as f32 * SIZE * 2.0,
                    )),
                ));
            }
        }
    });

    // Blocks
    const LEN: f32 = 2.0;
    let mesh = meshes.add(Cuboid::from_length(LEN));
    let material = materials.add(Color::BLACK);
    let hover_material = materials.add(Color::srgb(0.5, 0.5, 1.));
    for i in 0..10 {
        c.spawn((
            Name::new("Block"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(LEN * 2.0 * i as f32, LEN * 0.5, 0.0)),
        ))
        .observe(update_material_on::<Pointer<Over>>(hover_material.clone()))
        .observe(update_material_on::<Pointer<Out>>(material.clone()));
    }
}
