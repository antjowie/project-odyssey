//! Systems related to player actions
use super::*;

pub(super) fn player_plugin(app: &mut App) {
    app.add_plugins(InputContextPlugin::<PlayerViewAction>::default());
    app.add_plugins(InputContextPlugin::<PlayerBuildAction>::default());
    app.add_event::<PlayerStateEvent>();
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
    pub fn set(&mut self, new_state: PlayerState, c: &mut Commands, e: Entity) {
        match self {
            PlayerState::Viewing => c.entity(e).remove::<InputContext<PlayerViewAction>>(),
            PlayerState::Building => c.entity(e).remove::<InputContext<PlayerBuildAction>>(),
        };

        *self = new_state;

        match self {
            PlayerState::Viewing => c
                .entity(e)
                .insert(InputContext::<PlayerViewAction>::default()),
            PlayerState::Building => c
                .entity(e)
                .insert(InputContext::<PlayerBuildAction>::default()),
        };
    }
}

pub fn in_player_state(
    state: PlayerState,
) -> impl FnMut(Query<&PlayerState, With<NetOwner>>) -> bool {
    move |query: Query<&PlayerState, With<NetOwner>>| !query.is_empty() && *query.single() == state
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
