//! Wrapper around input so we can display a dynamic list of possible actions
//! as well as input remaping and all that stuff
//!
//! To use you need to do the following
//! ```
//! // 1. Create an action enum
//! #[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
//! pub enum CameraAction {
//!     #[actionlike(DualAxis)]
//!     Translate,
//!     Pan,
//!     #[actionlike(DualAxis)]
//!     Orbit,
//!     #[actionlike(Axis)]
//!     Zoom,
//! }
//! // 2. Implement a default mapping
//! impl InputConfig for CameraAction {
//! fn default_input_map() -> InputMap<Self> {
//!     InputMap::default()
//!         .with_dual_axis(CameraAction::Translate, VirtualDPad::wasd().inverted_y())
//!         .with(CameraAction::Pan, MouseButton::Middle)
//!         .with_dual_axis(
//!             CameraAction::Orbit,
//!             DualAxislikeChord::new(MouseButton::Right, MouseMove::default().inverted()),
//!         )
//!         // We use Digital to avoid inconsistencies between platform
//!         // On windows our pixel value is 1, but on web it is 100 (or 125 if you use Windows scaling)
//!         .with_axis(CameraAction::Zoom, MouseScrollAxis::Y.inverted().digital())
//! }
//! }
//!
//! // 3. Implement Display trait, can usually directly map debug unless you want to localize enums
//!
//! impl fmt::Display for CameraAction {
//! fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!     fmt::Debug::fmt(&self, f)
//! }
//! }
//!
//! // 4. Register the plugin
//! impl Plugin for CameraPlugin {
//! fn build(&self, app: &mut App) {
//!     app.add_plugins(InputDisplayPlugin::<CameraAction>::default());
//!     //...
//! }
//! }
//! ```

use std::{
    fmt::{Display, Write},
    marker::PhantomData,
};

use bevy::{
    prelude::*,
    utils::hashbrown::{HashMap, HashSet},
};
use leafwing_input_manager::{clashing_inputs::BasicInputs, prelude::*};

pub struct InputDisplayPlugin<A: InputConfig> {
    _phantom: PhantomData<A>,
}

// Deriving default induces an undesired bound on the generic
impl<A: InputConfig> Default for InputDisplayPlugin<A> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<A: InputConfig + TypePath + bevy::reflect::GetTypeRegistration> Plugin
    for InputDisplayPlugin<A>
{
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InputDisplaySetupPlugin>() {
            app.add_plugins(InputDisplaySetupPlugin);
        }

        app.add_plugins(InputManagerPlugin::<A>::default());
        app.add_systems(
            Update,
            collect_input_display::<A>.before(update_input_display_text),
        );
    }
}

struct InputDisplaySetupPlugin;

impl Plugin for InputDisplaySetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputDisplayFrameData>();
        app.add_systems(Startup, spawn_input_display_ui);
        app.add_systems(
            Update,
            update_input_display_text.run_if(resource_changed::<InputDisplayFrameData>),
        );
    }
}

#[derive(Component)]
struct InputDisplayUIMarker;

fn spawn_input_display_ui(mut c: Commands) {
    c.spawn((
        InputDisplayUIMarker,
        Text::default(),
        TextFont {
            font_size: 15.,
            ..Default::default()
        },
        TextLayout::new_with_justify(JustifyText::Left),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(10.0),
            left: Val::Px(10.0),

            ..default()
        },
    ));
}

#[derive(Resource, Default)]
pub struct InputDisplayFrameData {
    pub inputs: Vec<InputDisplayCollection>,
}

pub struct InputDisplayCollection {
    /// Higher values are displayed higher in the list
    pub order: i32,
    pub name: String,
    /// Entries are ordered by enum/action order
    pub entries: Vec<InputDisplayEntry>,
}

impl InputDisplayCollection {
    fn new<A>(input_actions: &InputActions<A>, input_map: &InputMap<A>) -> Self
    where
        A: InputConfig,
    {
        let mut map: HashMap<A, InputDisplayEntry> = HashMap::new();
        // Keep a vec of all the action enums, so we can sort it and match
        let mut action_entries = vec![];

        let mut action_to_entry = |action: &A, inputs: BasicInputs| {
            let get_key = |input| {
                format!("{:?}", input)
                    .replace("Key", "")
                    .replace("ShiftLeft", "Shift")
                    .replace("ShiftRight", "Shift")
                    .replace("ControlLeft", "Control")
                    .replace("ControlRight", "Control")
            };
            let value = InputDisplayEntry {
                action: format!("{action}"),
                input: match inputs {
                    // TODO: Might be better to impl a visitor, or just impl UI feedback trait?
                    BasicInputs::None => "None".into(),
                    BasicInputs::Simple(buttonlike) => get_key(buttonlike),
                    BasicInputs::Composite(vec) => {
                        let vec = vec
                            .into_iter()
                            .filter(|x| x.kind() == InputControlKind::Button)
                            .map(|x: Box<dyn Buttonlike>| -> String {
                                info!("Processing composite input {:?}", x);
                                let dbg = x.clone();
                                let any = x.into_any();
                                if let Some(_keycode) = any.downcast_ref::<KeyCode>() {
                                    get_key(dbg)
                                } else if let Some(button) = any.downcast_ref::<MouseButton>() {
                                    format!("Mouse{:?}", button)
                                } else if let Some(_mouse_scroll) =
                                    any.downcast_ref::<MouseScrollDirection>()
                                {
                                    // format!("Scroll{:?}", mouse_scroll.direction)
                                    "MouseScroll".into()
                                } else if let Some(_mouse_move) =
                                    any.downcast_ref::<MouseMoveDirection>()
                                {
                                    // format!("Mouse{:?}", mouse_move.direction)
                                    "MouseMove".into()
                                } else {
                                    error!("Can't process action input type: {:?}", dbg);
                                    // assert!(false);
                                    "ERR".into()
                                }
                            })
                            .collect::<Vec<String>>();

                        let mut seen = HashSet::new();
                        let mut text = String::new();
                        for item in vec {
                            if seen.insert(item.clone()) {
                                if !text.is_empty() {
                                    text.push_str(" + ");
                                }
                                text.push_str(&item);
                            }
                        }

                        text
                    }
                    BasicInputs::Chord(vec) => {
                        let mut seen = HashSet::new();
                        let mut text = String::new();
                        for item in vec {
                            info!("Processing chord input {:?}", item);
                            let item = get_key(item);
                            if seen.insert(item.clone()) {
                                if !text.is_empty() {
                                    text.push_str(" + ");
                                }
                                text.push_str(&item);
                            }
                        }

                        text
                    }
                },
            };

            map.entry(action.clone())
                .and_modify(|x| x.input = format!("{} | {}", x.input, value.input.as_str()))
                .or_insert(value);
            action_entries.push(action.clone());
        };

        let should_process = |a: &A| {
            input_actions.display_whitelist.is_empty()
                || input_actions.display_whitelist.contains(a)
        };

        input_map.buttonlike_bindings().for_each(|(a, i)| {
            if should_process(a) {
                action_to_entry(a, i.decompose())
            }
        });
        input_map.axislike_bindings().for_each(|(a, i)| {
            if should_process(a) {
                action_to_entry(a, i.decompose())
            }
        });
        input_map.dual_axislike_bindings().for_each(|(a, i)| {
            if should_process(a) {
                action_to_entry(a, i.decompose())
            }
        });
        input_map.triple_axislike_bindings().for_each(|(a, i)| {
            if should_process(a) {
                action_to_entry(a, i.decompose())
            }
        });

        action_entries.dedup();
        action_entries.sort();

        InputDisplayCollection {
            order: 0,
            name: A::group_name(),
            entries: action_entries
                .iter()
                .map(|x| map.get(x).unwrap().clone())
                .collect(),
        }
    }
}

#[derive(Clone)]
pub struct InputDisplayEntry {
    pub action: String,
    pub input: String,
}

/// Resembles a collection of input that correspond to actions. Add this component to get feedback and listen to input
#[derive(Component)]
#[require(ActionState<A>, InputMap<A>(|| A::default_input_map()))]
pub struct InputActions<A: InputConfig> {
    // If empty, display all inputs in InputMap
    pub display_whitelist: HashSet<A>,
}

/// Our actions
pub trait InputConfig: Actionlike + Display + Ord + Clone {
    fn default_input_map() -> InputMap<Self>;
    fn group_name() -> String;
}

// Deriving default induces an undesired bound on the generic
impl<A: InputConfig + InputConfig> Default for InputActions<A> {
    fn default() -> Self {
        Self {
            display_whitelist: HashSet::<A>::default(),
        }
    }
}

fn collect_input_display<A>(
    mut data: ResMut<InputDisplayFrameData>,
    q: Query<(&InputActions<A>, &InputMap<A>), Or<(Added<InputMap<A>>, Changed<InputMap<A>>)>>,
) where
    A: InputConfig,
{
    q.iter().for_each(|(input_actions, input_map)| {
        data.inputs
            .push(InputDisplayCollection::new(input_actions, input_map))
    });
}

fn update_input_display_text(
    mut data: ResMut<InputDisplayFrameData>,
    mut q: Query<&mut Text, With<InputDisplayUIMarker>>,
) {
    info!("Updating input display data");
    if q.is_empty() {
        return;
    }

    let mut text = q.single_mut();
    text.0.clear();

    data.inputs.iter().for_each(|collection| {
        text.0
            .write_fmt(format_args!("{}\n", collection.name))
            .unwrap();
        collection.entries.iter().for_each(|x| {
            text.0
                .write_fmt(format_args!("  {} - {}\n", x.action, x.input))
                .unwrap()
        });
    });

    *data.bypass_change_detection() = InputDisplayFrameData::default();
}
