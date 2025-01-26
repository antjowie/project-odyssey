//! Any logic or systems strongly related to gameplay and content are grouped under the game module
//!
//! These systems are made with the purpose to serve gameplay. It wouldn't make sense to use them outside
//! of this context.
//!
//! You could argue input and camera are also strongly game related, but these systems can still be used without
//! knowing about anything game related. To not bloat the root and ease reuse, the distinction is made.

use std::f32::consts::PI;

use bevy::{color::palettes::tailwind::*, picking::pointer::PointerInteraction};
use bevy::{math::*, prelude::*, window::PrimaryWindow};

use crate::camera::*;
use crate::input::*;
use crate::util::*;
use placeable::*;
use player::*;
use world::*;

pub mod placeable;
pub mod player;
pub mod world;

/// All game systems and rules
// 100 units is 1 meter
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(placeable_plugin);
        app.add_plugins(player_plugin);
        app.add_plugins(world_plugin);

        app.add_systems(
            Update,
            (
                draw_mesh_intersections,
                draw_build_grid.run_if(not(in_player_state(PlayerState::Viewing))),
            ),
        );
        app.register_type::<PlayerCursor>();
    }
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
            translation: vec3(cursor.world_grid_pos.x, 0.01, cursor.world_grid_pos.z).into(),
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
