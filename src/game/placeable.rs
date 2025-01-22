use bevy::prelude::*;

#[derive(Component, Default, PartialEq, Clone)]
pub enum Placeable {
    #[default]
    Rail,
    Train,
}

pub fn is_placeable(placeable: Placeable) -> impl FnMut(Query<&Placeable>) -> bool {
    move |query: Query<&Placeable>| !query.is_empty() && *query.single() == placeable
}
