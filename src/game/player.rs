//! Systems related to player actions
use super::*;

pub(super) fn player_plugin(app: &mut App) {
    app.add_plugins(InputContextPlugin::<PlayerViewAction>::default());
    app.add_plugins(InputContextPlugin::<PlayerBuildAction>::default());
    app.add_event::<PlayerStateEvent>();
    app.add_systems(
        Update,
        (
            setup_player_state.run_if(any_with_component::<PlayerState>),
            handle_view_state_input.run_if(any_with_component::<ActionState<PlayerViewAction>>),
            handle_build_state_input.run_if(any_with_component::<ActionState<PlayerBuildAction>>),
            update_picked_placeable.run_if(in_player_state(PlayerState::Building)),
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

#[derive(
    Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect, PartialOrd, Ord, DisplayDebug,
)]
pub enum PlayerBuildAction {
    PickRail,
    PickTrain,
    Interact,
    Cancel,
    CancelWithMouse,
    Rotate,
    CounterRotate,
    SnapRotate,
    SnapCounterRotate,
    CycleCurveMode,
    ToggleSnapToGrid,
}

impl InputContextlike for PlayerBuildAction {
    fn default_input_map() -> InputMap<Self> {
        InputMap::default()
            .with(PlayerBuildAction::PickRail, KeyCode::Digit1)
            .with(PlayerBuildAction::PickTrain, KeyCode::Digit2)
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
            .with(PlayerBuildAction::CycleCurveMode, KeyCode::Tab)
            .with(PlayerBuildAction::ToggleSnapToGrid, KeyCode::ControlLeft)
    }
    fn group_name() -> String {
        "Build Actions".into()
    }
}

#[derive(Component, Default, PartialEq, Clone)]
#[require(PlayerCursor, Placeable, InputContext<PlayerViewAction>)]
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
            PlayerState::Viewing => ec.remove::<InputContext<PlayerViewAction>>(),
            PlayerState::Building => ec.remove::<InputContext<PlayerBuildAction>>(),
        };

        *self = new_state;

        match self {
            PlayerState::Viewing => ec.insert(InputContext::<PlayerViewAction>::default()),
            PlayerState::Building => ec.insert(InputContext::<PlayerBuildAction>::default()),
        };
    }
}

pub fn in_player_state(state: PlayerState) -> impl FnMut(Query<&PlayerState>) -> bool {
    move |query: Query<&PlayerState>| !query.is_empty() && *query.single() == state
}

fn setup_player_state(mut c: Commands, q: Query<Entity, Added<PlayerState>>) {
    q.iter().for_each(|e| {
        c.entity(e).observe(handle_build_state_cancel_event);
    });
}

#[derive(Event)]
pub struct PlayerStateEvent {
    pub new_state: PlayerState,
    pub old_state: PlayerState,
}

#[derive(Component)]
pub struct BuildStateCancelEvent;

impl Event for BuildStateCancelEvent {
    type Traversal = &'static BuildingPreview;
    const AUTO_PROPAGATE: bool = true;
}

/// Component that tracks the cursor position
#[derive(Component, Default, Reflect)]
pub struct PlayerCursor {
    pub screen_pos: Option<Vec2>,
    pub should_snap_to_grid: bool,
    // Cached build rotation
    pub manual_rotation: f32,
    pub curve_mode: PathCurveMode,
    // Can be world or grid pos based on user desire
    pub build_pos: Vec3,
    pub world_pos: Vec3,
    pub prev_world_pos: Vec3,
    pub world_grid_pos: Vec3,
}

#[derive(Default, Reflect, PartialEq, Debug, DisplayDebug)]
pub enum PathCurveMode {
    // Share same angle between start and end joint
    #[default]
    Curve,
    // Keep aligned with start joint
    Straight,
    // Align end joint with direction between end and start point
    Chase,
}

impl PathCurveMode {
    pub fn next(&self) -> Self {
        match self {
            PathCurveMode::Curve => PathCurveMode::Straight,
            PathCurveMode::Straight => PathCurveMode::Chase,
            PathCurveMode::Chase => PathCurveMode::Curve,
        }
    }
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
    mut c: Commands,
    q: Query<(Entity, &PlayerCursor, &ActionState<PlayerBuildAction>)>,
    previews: Query<Entity, With<BuildingPreview>>,
    mut cancel_mouse_pos: Local<Vec2>,
) {
    let mut trigger = false;
    q.iter().for_each(|(_, cursor, input)| {
        if input.just_pressed(&PlayerBuildAction::Cancel) {
            trigger = true;
        }

        if input.just_pressed(&PlayerBuildAction::CancelWithMouse) {
            *cancel_mouse_pos = cursor.screen_pos.unwrap_or_default();
        } else if input.just_released(&PlayerBuildAction::CancelWithMouse)
            && *cancel_mouse_pos == cursor.screen_pos.unwrap_or_default()
        {
            trigger = true;
        }
    });

    if trigger {
        if !previews.is_empty() {
            previews
                .iter()
                .for_each(|e| c.trigger_targets(BuildStateCancelEvent, e));
        } else {
            q.iter()
                .for_each(|(e, _, _)| c.trigger_targets(BuildStateCancelEvent, e));
        }
    }
}

/// Event propogated from BuildPreview
fn handle_build_state_cancel_event(
    trigger: Trigger<BuildStateCancelEvent>,
    mut q: Query<&mut PlayerState>,
    mut ev_state: EventWriter<PlayerStateEvent>,
    mut c: Commands,
) {
    let e = trigger.entity();
    let mut state = q.get_mut(e).unwrap();
    state.set(PlayerState::Viewing, &mut c, e, &mut ev_state);
}

fn update_picked_placeable(mut q: Query<(&ActionState<PlayerBuildAction>, &mut Placeable)>) {
    q.iter_mut().for_each(|(input, mut placeable)| {
        if input.just_pressed(&PlayerBuildAction::PickRail) {
            *placeable = Placeable::Rail;
        }
        if input.just_pressed(&PlayerBuildAction::PickTrain) {
            *placeable = Placeable::Train;
        }
    });
}
