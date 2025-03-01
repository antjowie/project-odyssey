//! Any logic or systems strongly related to gameplay and content are grouped under the game module
//!
//! These systems are made with the purpose to serve gameplay. It wouldn't make sense to use them outside
//! of this context.

use std::f32::consts::PI;

use avian3d::prelude::Collider;
use bevy::{color::palettes::tailwind::*, picking::pointer::PointerInteraction};
use bevy::{prelude::*, window::PrimaryWindow};

use crate::util::*;

use camera::*;
use cursor_feedback::*;
use input::*;
use placeable::*;
use player::*;
use selectable::*;
use spline::*;
use world::*;

pub mod camera;
pub mod cursor_feedback;
pub mod input;
pub mod placeable;
pub mod player;
pub mod selectable;
pub mod spline;
pub mod world;

/// All game systems and rules
// 100 units is 1 meter
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(camera_plugin);
        app.add_plugins(cursor_feedback_plugin);
        app.add_plugins(placeable_plugin);
        app.add_plugins(player_plugin);
        app.add_plugins(selectable_plugin);
        app.add_plugins(spline_plugin);
        app.add_plugins(world_plugin);

        app.configure_sets(
            Update,
            (
                GameSet::Spawn.before(GameSet::Update),
                GameSet::Despawn.after(GameSet::Update),
            ),
        );

        app.add_systems(
            Update,
            (
                generate_collider_on_mesh_changed,
                draw_mesh_intersections,
                draw_build_grid.run_if(not(in_player_state(PlayerState::Viewing))),
            )
                .in_set(GameSet::Update),
        );
        app.register_type::<PlayerCursor>();
    }
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GameSet {
    Spawn,
    Update,
    Despawn,
}

#[derive(Component, Default)]
pub struct GenerateCollider;

fn generate_collider_on_mesh_changed(
    mut c: Commands,
    q: Query<(Entity, &Mesh3d), (Changed<Mesh3d>, With<GenerateCollider>)>,
    meshes: Res<Assets<Mesh>>,
) {
    q.iter().for_each(|(e, mesh)| {
        if let Some(mesh) = meshes.get(mesh) {
            if let Some(collider) = Collider::trimesh_from_mesh(&mesh) {
                // if let Some(collider) = Collider::convex_hull_from_mesh(&mesh) {
                c.entity(e).insert(collider);
            }
        }
    });
}

/// https://bevyengine.org/examples/picking/mesh-picking/
fn draw_mesh_intersections(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
    for (point, normal) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
    {
        gizmos.sphere(point, 0.05, RED_500);
        gizmos.arrow(point, point + normal.normalize() * 0.5, PINK_100);
    }
}

fn draw_build_grid(mut gizmos: Gizmos, player: Single<(&PlayerCursor, &Placeable)>) {
    let (cursor, placeable) = player.into_inner();

    gizmos.grid(
        Isometry3d {
            rotation: Quat::from_axis_angle(Vec3::X, -PI * 0.5),
            translation: Vec3::new(cursor.world_grid_pos.x, 0.01, cursor.world_grid_pos.z).into(),
        },
        UVec2::splat(16),
        Vec2::splat(1.0),
        if placeable == &Placeable::Destroyer {
            Color::srgba(0.8, 0.3, 0.3, 0.3)
        } else {
            Color::srgba(0.8, 0.8, 0.8, 0.3)
        },
    );
}
