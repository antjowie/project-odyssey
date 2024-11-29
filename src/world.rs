use crate::camera::*;
use crate::game::*;

use bevy::pbr::DirectionalLightShadowMap;
use bevy::{
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster},
    prelude::*,
};
use leafwing_input_manager::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_world);
        app.insert_resource(DirectionalLightShadowMap { size: 4096 });
    }
}

fn setup_world(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // --- Gameplay
    // Player State
    c.spawn((NetOwner, PlayerStateBundle::default()));

    // Camera
    c.spawn((
        NetOwner,
        PanOrbitCameraBundle {
            input: InputManagerBundle::with_map(CameraAction::default_player_mapping()),
            ..default()
        },
        FogSettings {
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

    c.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::srgb(0.98, 0.95, 0.82),
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 0.0, 0.0)
            .looking_at(Vec3::new(-0.15, -0.05, 0.25), Vec3::Y),
        cascade_shadow_config,
        ..default()
    });

    // // Sky, might wanna use a skybox later
    c.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material: materials.add(StandardMaterial {
                base_color: Srgba::hex("888888").unwrap().into(),
                unlit: true,
                cull_mode: None,
                ..default()
            }),
            transform: Transform::from_scale(Vec3::splat(100_000_000.0)),
            ..default()
        },
        NotShadowCaster,
    ));

    // Terrain
    c.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(100_000.0))),
        material: materials.add(Color::WHITE),
        ..default()
    });

    for i in 0..10 {
        const LEN: f32 = 2.0;
        c.spawn(PbrBundle {
            mesh: meshes.add(Cuboid::from_length(LEN)),
            material: materials.add(Color::BLACK),
            transform: Transform::from_translation(Vec3::new(LEN * 2.0 * i as f32, LEN * 0.5, 0.0)),
            ..default()
        });
    }
}
