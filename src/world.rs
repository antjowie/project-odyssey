use crate::camera::*;
use crate::game::*;

use bevy::{
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster},
    prelude::*,
};
use leafwing_input_manager::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_world);
    }
}

fn setup_world(
    mut c: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // --- Gameplay
    // Player State
    c.spawn(PlayerStateBundle::default());

    // Camera
    c.spawn(PanOrbitCameraBundle {
        input: InputManagerBundle::with_map(CameraAction::default_player_mapping()),
        ..default()
    });

    // --- Visuals
    // Fog
    c.spawn(FogSettings {
        color: Color::srgba(0.35, 0.48, 0.66, 1.0),
        directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
        directional_light_exponent: 30.0,
        falloff: FogFalloff::from_visibility_colors(
            15.0, // distance in world units up to which objects retain visibility (>= 5% contrast)
            Color::srgb(0.35, 0.5, 0.66), // atmospheric extinction color (after light is lost due to absorption by atmospheric particles)
            Color::srgb(0.8, 0.844, 1.0), // atmospheric inscattering color (light gained due to scattering from the sun)
        ),
    });

    // Sun
    // Configure a properly scaled cascade shadow map for this scene (defaults are too large, mesh units are in km)
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        first_cascade_far_bound: 0.3,
        maximum_distance: 3.0,
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

    // Sky, might wanna use a skybox later
    c.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material: materials.add(StandardMaterial {
                base_color: Srgba::hex("888888").unwrap().into(),
                unlit: true,
                cull_mode: None,
                ..default()
            }),
            transform: Transform::from_scale(Vec3::splat(100000.0)),
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
        const LEN: f32 = 1.0;
        c.spawn(PbrBundle {
            mesh: meshes.add(Cuboid::from_length(LEN)),
            material: materials.add(Color::BLACK),
            transform: Transform::from_translation(Vec3::new(LEN * 2.0 * i as f32, 0.0, 0.0)),
            ..default()
        });
    }
}
