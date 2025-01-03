//! Loads our initial world
use super::*;
use bevy::pbr::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster};

pub(super) fn world_plugin(app: &mut App) {
    app.insert_resource(DirectionalLightShadowMap { size: 4096 });
    app.add_systems(Startup, spawn_test_world);
}

fn spawn_test_world(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // --- Gameplay
    // Player State
    c.spawn((NetOwner, PlayerState::default()));

    // Camera
    c.spawn((
        NetOwner,
        PanOrbitCamera::default(),
        DistanceFog {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::from_visibility_colors(
                PanOrbitCameraSettings::default().max_radius * 5.0, // distance in world units up to which objects retain visibility (>= 5% contrast)
                Color::srgb(0.35, 0.5, 0.66), // atmospheric extinction color (after light is lost due to absorption by atmospheric particles)
                Color::srgb(0.8, 0.844, 1.0), // atmospheric inscattering color (light gained due to scattering from the sun)
            ),
        },
    ));

    // Sun
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        // num_cascades: 4,
        // minimum_distance: 0.1,
        // maximum_distance: 1000.0,
        // overlap_proportion: 0.5,
        minimum_distance: 1.0,
        // first_cascade_far_bound: 2.0,
        maximum_distance: PanOrbitCameraSettings::default().max_radius * 1.5,
        ..default()
    }
    .build();

    c.spawn((
        DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(-0.15, -0.05, 0.25), Vec3::Y),
        cascade_shadow_config,
    ));

    // Sky, might wanna use a skybox later
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("888888").unwrap().into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(100_000_000.0)),
        NotShadowCaster,
    ));

    // Terrain
    c.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(100_000.0)))),
        MeshMaterial3d(materials.add(Color::WHITE)),
    ));

    // Blocks
    const LEN: f32 = 2.0;
    let mesh = meshes.add(Cuboid::from_length(LEN));
    let material = materials.add(Color::BLACK);
    for i in 0..10 {
        c.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(LEN * 2.0 * i as f32, LEN * 0.5, 0.0)),
        ));
    }
}
