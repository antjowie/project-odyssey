//! Input system to support displaying entries, changing keybindings.
//! To faciliate input handling consider any input as an contextual object.
//!
//! You create an enum of actions you want this context to do. These actions can be executed by anything,
//! but usually it will be through input response.
//!
//! Consider a player editor. We can have the following InputContexts:
//! * InGameAction
//!     * PauseGame
//! * PlayerViewAction
//!     * EnterBuildMode
//!     * InspectCursor
//! * PlayerBuildAction
//!     * EnterViewMode
//!     * Place
//!     * Remove
//!     
//! InputContext can be related with states, and we create and destroy them when entering the associated states.
//! By using this system we can generate a list of actionable inputs based on the current context
//!
//! To use you need to do the following:
//! ```
//! // 1. Create an action enum
//! #[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
//! enum CameraAction {
//!     #[actionlike(DualAxis)]
//!     Translate,
//!     Pan,
//!     #[actionlike(DualAxis)]
//!     Orbit,
//!     #[actionlike(Axis)]
//!     Zoom,
//! }
//!
//! // 2. Implement a default mapping
//! impl InputContextlike for CameraAction {
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
//! impl fmt::Display for CameraAction {
//! fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!     fmt::Debug::fmt(&self, f)
//! }
//! }
//!
//! // 4. Register the plugin
//! impl Plugin for CameraPlugin {
//! fn build(&self, app: &mut App) {
//!     app.add_plugins(InputContextPlugin::<CameraAction>::default());
//!     //...
//! }
//! }
//!
//! // 5. Add the InputContext as a required component to our relevant component
//! #[derive(Component)]
//! #[require(InputContext<CameraAction>)]
//! struct Camera();

use bevy::{
    prelude::*,
    utils::hashbrown::{HashMap, HashSet},
};
pub use leafwing_input_manager::{
    clashing_inputs::BasicInputs, plugin::InputManagerSystem, prelude::*,
};

use std::{
    fmt::{Display, Write},
    marker::PhantomData,
};

/// Add this component to an entity to start tracking input state
/// You can get the actual value by querrying for &ActionState<A>
#[derive(Component)]
#[require(ActionState<A>, InputMap<A>(|| A::default_input_map()))]
pub struct InputContext<A: InputContextlike> {
    // If empty, display all inputs in InputMap
    pub display_whitelist: HashSet<A>,
}

// Deriving default induces an undesired bound on the generic
impl<A: InputContextlike> Default for InputContext<A> {
    fn default() -> Self {
        Self {
            display_whitelist: HashSet::<A>::default(),
        }
    }
}

/// Implement this trait for any InputContext
pub trait InputContextlike: Actionlike + Display + Ord + Clone {
    fn default_input_map() -> InputMap<Self>;
    fn group_name() -> String;
}

// Deriving default induces an undesired bound on the generic
impl<A: InputContextlike> Default for InputContextPlugin<A> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

/// Plugin to add for each action context you specify
pub struct InputContextPlugin<A: InputContextlike> {
    _phantom: PhantomData<A>,
}

impl<A: InputContextlike + TypePath + bevy::reflect::GetTypeRegistration> Plugin
    for InputContextPlugin<A>
{
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InputSetupPlugin>() {
            app.add_plugins(InputSetupPlugin);
        }

        app.add_plugins(InputManagerPlugin::<A>::default());
        app.add_systems(
            Update,
            collect_input_display::<A>.before(update_input_display_text),
        );
    }
}

struct InputSetupPlugin;

impl Plugin for InputSetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AllInputContextEntries>();
        app.add_systems(Startup, spawn_input_display_ui);
        app.add_systems(
            Update,
            update_input_display_text.run_if(resource_changed::<AllInputContextEntries>),
        );
    }
}

/// Resource caching all input context entries
#[derive(Resource, Default)]
pub struct AllInputContextEntries {
    pub entries: Vec<InputContextEntries>,
}

pub struct InputContextEntries {
    pub name: String,
    /// Entries are ordered according to InputConfig Ord impl
    pub entries: Vec<InputContextEntry>,
}

impl InputContextEntries {
    fn new<A>(input_actions: &InputContext<A>, input_map: &InputMap<A>) -> Self
    where
        A: InputContextlike,
    {
        let mut map: HashMap<A, InputContextEntry> = HashMap::new();
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
            let value = InputContextEntry {
                action: format!("{action}"),
                // TODO: Might be better to impl a visitor, or just impl UI feedback trait?
                input: match inputs {
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

        InputContextEntries {
            name: A::group_name(),
            entries: action_entries
                .iter()
                .map(|x| map.get(x).unwrap().clone())
                .collect(),
        }
    }
}

#[derive(Clone)]
pub struct InputContextEntry {
    pub action: String,
    pub input: String,
}

#[derive(Component)]
struct InputEntriesUIMaker;

fn spawn_input_display_ui(mut c: Commands) {
    c.spawn((
        InputEntriesUIMaker,
        Text::default(),
        TextFont {
            font_size: 15.,
            ..Default::default()
        },
        TextLayout::new_with_justify(JustifyText::Left),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),

            ..default()
        },
    ));
}

fn collect_input_display<A>(
    mut data: ResMut<AllInputContextEntries>,
    q: Query<(&InputContext<A>, &InputMap<A>), Or<(Added<InputMap<A>>, Changed<InputMap<A>>)>>,
) where
    A: InputContextlike,
{
    q.iter().for_each(|(input_actions, input_map)| {
        data.entries
            .push(InputContextEntries::new(input_actions, input_map))
    });
}

fn update_input_display_text(
    mut data: ResMut<AllInputContextEntries>,
    mut q: Query<&mut Text, With<InputEntriesUIMaker>>,
) {
    info!("Updating input display data");
    if q.is_empty() {
        return;
    }

    let mut text = q.single_mut();
    text.0.clear();

    data.entries.iter().for_each(|collection| {
        text.0
            .write_fmt(format_args!("{}\n", collection.name))
            .unwrap();
        collection.entries.iter().for_each(|x| {
            text.0
                .write_fmt(format_args!("  {} - {}\n", x.action, x.input))
                .unwrap()
        });
    });

    *data.bypass_change_detection() = AllInputContextEntries::default();
}
