//! Systems related to player actions
use super::*;

pub(super) fn player_plugin(app: &mut App) {
    app.add_plugins(InputContextPlugin::<PlayerViewAction>::default());
    app.add_plugins(InputContextPlugin::<PlayerBuildAction>::default());
    app.add_event::<PlayerStateEvent>();
    app.add_systems(
        Update,
        (
            handle_view_state_input.run_if(any_with_component::<ActionState<PlayerViewAction>>),
            handle_build_state_input.run_if(any_with_component::<ActionState<PlayerBuildAction>>),
        ),
    );
}

#[derive(
    Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect, PartialOrd, Ord, DisplayDebug,
)]
pub enum PlayerViewAction {
    EnterBuildMode,
    ExitGame,
}

impl InputContextlike for PlayerViewAction {
    fn default_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(PlayerViewAction::EnterBuildMode, MouseButton::Left)
            .with(PlayerViewAction::ExitGame, KeyCode::Escape)
    }

    fn group_name() -> String {
        "View Actions".into()
    }
}

#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect, PartialOrd, Ord)]
pub enum PlayerBuildAction {
    Interact,
    Cancel,
    CancelWithMouse,
    Rotate,
    CounterRotate,
    SnapRotate,
    SnapCounterRotate,
    CyclePathRotateMode,
    ToggleSnapToGrid,
}

impl InputContextlike for PlayerBuildAction {
    fn default_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(PlayerBuildAction::Interact, MouseButton::Left)
            .with(PlayerBuildAction::Cancel, KeyCode::KeyE)
            .with(PlayerBuildAction::Cancel, KeyCode::Escape)
            .with(PlayerBuildAction::CancelWithMouse, MouseButton::Right)
            .with(PlayerBuildAction::Rotate, KeyCode::KeyR)
            .with(
                PlayerBuildAction::SnapRotate,
                ButtonlikeChord::modified(ModifierKey::Control, KeyCode::KeyR),
            )
            .with(
                PlayerBuildAction::CounterRotate,
                ButtonlikeChord::modified(ModifierKey::Shift, KeyCode::KeyR),
            )
            .with(
                PlayerBuildAction::SnapCounterRotate,
                ButtonlikeChord::default()
                    .with(ModifierKey::Control)
                    .with(ModifierKey::Shift)
                    .with(KeyCode::KeyR),
            )
            .with(PlayerBuildAction::CyclePathRotateMode, KeyCode::Tab)
            .with(PlayerBuildAction::ToggleSnapToGrid, KeyCode::ControlLeft)
    }
    fn group_name() -> String {
        "Build Actions".into()
    }
}

impl fmt::Display for PlayerBuildAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self == &PlayerBuildAction::CyclePathRotateMode {
            write!(f, "{:?} Straight/Curve/Chase", self)
        } else {
            fmt::Debug::fmt(&self, f)
        }
    }
}

#[derive(Component, Default, PartialEq, Clone)]
#[require(PlayerCursor, InputContext<PlayerViewAction>)]
pub enum PlayerState {
    #[default]
    Viewing,
    Building,
}

impl PlayerState {
    pub fn set(
        &mut self,
        new_state: PlayerState,
        c: &mut Commands,
        e: Entity,
        ev_player_state: &mut EventWriter<PlayerStateEvent>,
    ) {
        if new_state == *self {
            return;
        }

        ev_player_state.send(PlayerStateEvent {
            old_state: self.clone(),
            new_state: new_state.clone(),
        });

        let mut ec = c.entity(e);

        match self {
            PlayerState::Viewing => PlayerState::remove::<PlayerViewAction>(&mut ec),
            PlayerState::Building => PlayerState::remove::<PlayerBuildAction>(&mut ec),
        };

        *self = new_state;

        match self {
            PlayerState::Viewing => ec.insert(InputContext::<PlayerViewAction>::default()),
            PlayerState::Building => ec.insert(InputContext::<PlayerBuildAction>::default()),
        };
    }

    fn remove<T: InputContextlike>(ec: &mut EntityCommands) {
        ec.remove::<InputContext<T>>();
        ec.remove::<ActionState<T>>();
        ec.remove::<InputMap<T>>();
    }
}

pub fn in_player_state(state: PlayerState) -> impl FnMut(Query<&PlayerState>) -> bool {
    move |query: Query<&PlayerState>| !query.is_empty() && *query.single() == state
}

#[derive(Event)]
pub struct PlayerStateEvent {
    pub new_state: PlayerState,
    pub old_state: PlayerState,
}

/// Component that tracks the cursor position
#[derive(Component, Default, Reflect)]
pub struct PlayerCursor {
    pub screen_pos: Option<Vec2>,
    pub should_snap_to_grid: bool,
    // Cached build rotation
    pub manual_rotation: f32,
    pub rotation_mode: PathRotationMode,
    // Can be world or grid pos based on user desire
    pub build_pos: Vec3,
    pub world_pos: Vec3,
    pub prev_world_pos: Vec3,
    pub world_grid_pos: Vec3,
}

fn handle_view_state_input(
    mut q: Query<(Entity, &mut PlayerState, &ActionState<PlayerViewAction>)>,
    mut c: Commands,
    mut ev_state: EventWriter<PlayerStateEvent>,
    mut ev_exit: EventWriter<AppExit>,
) {
    q.iter_mut().for_each(|(e, mut state, input)| {
        if input.just_pressed(&PlayerViewAction::EnterBuildMode) {
            state.set(PlayerState::Building, &mut c, e, &mut ev_state);
        }

        if input.just_pressed(&PlayerViewAction::ExitGame) {
            ev_exit.send(AppExit::Success);
        }
    });
}

fn handle_build_state_input(
    mut q: Query<(
        Entity,
        &mut PlayerState,
        &PlayerCursor,
        &ActionState<PlayerBuildAction>,
    )>,
    mut previews: Query<&mut BuildingPreview>,
    mut c: Commands,
    mut ev_state: EventWriter<PlayerStateEvent>,
    mut cancel_mouse_pos: Local<Vec2>,
) {
    q.iter_mut().for_each(|(e, mut state, cursor, input)| {
        let wants_to_place = input.just_pressed(&PlayerBuildAction::Interact);
        previews.iter_mut().for_each(|mut preview| {
            preview.wants_to_place = wants_to_place;
        });

        if input.just_pressed(&PlayerBuildAction::Cancel) {
            state.set(PlayerState::Viewing, &mut c, e, &mut ev_state);
        }

        if input.just_pressed(&PlayerBuildAction::CancelWithMouse) {
            *cancel_mouse_pos = cursor.screen_pos.unwrap_or_default();
        } else if input.just_released(&PlayerBuildAction::CancelWithMouse)
            && *cancel_mouse_pos == cursor.screen_pos.unwrap_or_default()
        {
            state.set(PlayerState::Viewing, &mut c, e, &mut ev_state);
        }
    });
}
