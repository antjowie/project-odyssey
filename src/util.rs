use bevy::prelude::*;

pub fn default_text_font() -> TextFont {
    TextFont {
        font_size: 15.,
        ..default()
    }
}

pub fn destroy_with_children(c: &mut Commands, entity: Entity, children: &Query<&Children>) {
    let mut destroy = |e| {
        c.entity(e).despawn();
    };

    children.iter_descendants(entity).for_each(&mut destroy);
    destroy(entity);
}

/// Returns an observer that updates the entity's material to the one specified.
/// https://bevyengine.org/examples/picking/mesh-picking/
pub fn update_material_on<E>(
    new_material: Handle<StandardMaterial>,
) -> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
    // An observer closure that captures `new_material`. We do this to avoid needing to write four
    // versions of this observer, each triggered by a different event and with a different hardcoded
    // material. Instead, the event type is a generic, and the material is passed in.
    move |trigger, mut query| {
        if let Ok(mut material) = query.get_mut(trigger.entity()) {
            material.0 = new_material.clone();
        }
    }
}
