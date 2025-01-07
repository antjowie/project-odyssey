use bevy::prelude::*;

pub fn default_text_font() -> TextFont {
    TextFont {
        font_size: 15.,
        ..default()
    }
}
