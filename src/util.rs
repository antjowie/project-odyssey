use bevy::prelude::*;

// Maybe I could use some UID system
#[derive(Default)]
pub struct IdProvider {
    next_id: u32,
    available_ids: Vec<u32>,
}

impl IdProvider {
    pub fn get_available_id(&mut self) -> u32 {
        if self.available_ids.len() > 0 {
            return self.available_ids.pop().unwrap();
        }
        self.next_id += 1;
        return self.next_id;
    }

    pub fn return_id(&mut self, id: u32) {
        self.available_ids.push(id);
    }
}

pub fn default_text_font() -> TextFont {
    TextFont {
        font_size: 15.,
        ..default()
    }
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
