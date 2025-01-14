use bevy::prelude::*;

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
