//! Input system to support displaying entries, changing keybindings.
//!
//! To facilitate input handling consider any input as an contextual object.
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
//! // NOTE: You can also derive DisplayDebug to automatically generate this
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
//!
//! // 6. Then you can access the state of the input context with Query<&ActionState<CameraAction>>
//!

pub use bevy::prelude::*;
use bevy::utils::hashbrown::{HashMap, HashSet};
pub use leafwing_input_manager::{
    clashing_inputs::BasicInputs, plugin::InputManagerSystem, prelude::*,
};
pub use project_odyssey_macros::DisplayDebug;
pub use std::fmt;

use std::{fmt::Write, marker::PhantomData};

use crate::util::default_text_font;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputSet {
    PurgeEntries,
    CollectEntries,
    EntriesCollected,
}

/// Add this component to an entity to start tracking input state
/// You can get the actual value by querrying for `&ActionState<A>`
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

/// We need to enable this with a one frame delay, event seems like the simplest
/// method although a defer method would be preferable. Command could facilitate
/// this but events are a bit simpler to use, even though there is additional
/// boilerplate that goes along with it
#[derive(Event)]
struct EnableInputContextEvent<A: InputContextlike> {
    entity: Entity,
    _phantom: PhantomData<A>,
}

/// Implement this trait for any InputContext
pub trait InputContextlike: Actionlike + std::fmt::Display + Ord + Clone {
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
        app.add_event::<EnableInputContextEvent<A>>();
        app.add_systems(
            PreUpdate,
            (
                (
                    handle_enable_input_context_event::<A>
                        .before(input_context_added::<A>)
                        .run_if(on_event::<EnableInputContextEvent<A>>),
                    input_context_added::<A>
                        .run_if(condition_changed(any_with_component::<InputContext<A>>)),
                    input_context_removed::<A>.run_if(any_component_removed::<InputContext<A>>),
                )
                    .in_set(InputSet::PurgeEntries),
                collect_input_context_entries::<A>
                    .run_if(resource_changed::<AllInputContextEntries>)
                    .in_set(InputSet::CollectEntries),
            ),
        );
    }
}

struct InputSetupPlugin;

impl Plugin for InputSetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AllInputContextEntries>();
        app.configure_sets(
            PreUpdate,
            (
                InputSet::PurgeEntries
                    .before(InputSet::CollectEntries)
                    .after(InputManagerSystem::ManualControl),
                InputSet::CollectEntries.before(InputSet::EntriesCollected),
            ),
        );
        app.add_systems(Startup, spawn_input_display_ui);
        app.add_systems(
            PreUpdate,
            update_input_display_text
                .run_if(resource_changed::<AllInputContextEntries>)
                .in_set(InputSet::EntriesCollected),
        );
    }
}

/// Resource caching all input context entries
/// When this is modified all input contexts will accumulate their entries
/// into this object
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
                                // info!("Processing composite input {:?}", x);
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
                            // info!("Processing chord input {:?}", item);
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
struct InputDisplayUIMarker;

fn spawn_input_display_ui(mut c: Commands) {
    c.spawn((
        InputDisplayUIMarker,
        Text::default(),
        default_text_font(),
        TextLayout::new_with_justify(JustifyText::Left),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),

            ..default()
        },
    ));
}

fn handle_enable_input_context_event<A: InputContextlike>(
    mut q: Query<&mut ActionState<A>>,
    mut data: ResMut<AllInputContextEntries>,
    mut ev_enable: EventReader<EnableInputContextEvent<A>>,
) {
    ev_enable.read().for_each(|ev| {
        if let Ok(mut input) = q.get_mut(ev.entity) {
            input.enable();
            *data = AllInputContextEntries::default();

            // info!("Added {}", A::group_name());
        }
    });
}

fn input_context_added<A: InputContextlike>(
    q: Query<Entity, Added<InputContext<A>>>,
    mut ev_enable: EventWriter<EnableInputContextEvent<A>>,
) {
    q.iter().for_each(|entity| {
        ev_enable.send(EnableInputContextEvent {
            entity,
            _phantom: PhantomData::<A>,
        });
    });
}

fn input_context_removed<A: InputContextlike>(
    mut q: Query<&mut ActionState<A>>,
    mut data: ResMut<AllInputContextEntries>,
) {
    q.iter_mut().for_each(|mut input| {
        input.disable();
        // info!("Removed {}", A::group_name());
    });
    *data = AllInputContextEntries::default();
}

fn collect_input_context_entries<A: InputContextlike>(
    mut data: ResMut<AllInputContextEntries>,
    q: Query<(&InputContext<A>, &InputMap<A>, &ActionState<A>)>,
) {
    q.iter().for_each(|(input_actions, input_map, state)| {
        if !state.disabled() {
            // info!("Collecting {}", A::group_name());
            data.bypass_change_detection()
                .entries
                .push(InputContextEntries::new(input_actions, input_map))
        }
    });
}

fn update_input_display_text(
    data: Res<AllInputContextEntries>,
    mut q: Query<&mut Text, With<InputDisplayUIMarker>>,
) {
    // info!(
    //     "-- Updating input display data... ({}) entries",
    //     data.entries.len()
    // );
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
    // info!("-- Updating input display data DONE");
}
