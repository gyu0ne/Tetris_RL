use crate::{FrameInput, HEIGHT, RotationDirection, WIDTH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputButton {
    Left,
    Right,
    SoftDrop,
    HardDrop,
    RotateClockwise,
    RotateCounterclockwise,
    RotateHalf,
    Hold,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputEdgeKind {
    Press,
    Release,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputEdge {
    pub button: InputButton,
    pub kind: InputEdgeKind,
}

impl InputEdge {
    pub const fn press(button: InputButton) -> Self {
        Self {
            button,
            kind: InputEdgeKind::Press,
        }
    }

    pub const fn release(button: InputButton) -> Self {
        Self {
            button,
            kind: InputEdgeKind::Release,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoftDropMode {
    Disabled,
    CellsPerFrame(u16),
    Sonic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandlingRules {
    pub das_frames: u16,
    pub arr_frames: u16,
    pub dcd_frames: u16,
    pub soft_drop: SoftDropMode,
}

impl HandlingRules {
    pub const fn new(
        das_frames: u16,
        arr_frames: u16,
        dcd_frames: u16,
        soft_drop: SoftDropMode,
    ) -> Self {
        Self {
            das_frames,
            arr_frames,
            dcd_frames,
            soft_drop,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HorizontalDirection {
    Left,
    Right,
}

impl HorizontalDirection {
    const fn action(self) -> FrameInput {
        match self {
            Self::Left => FrameInput::MoveLeft,
            Self::Right => FrameInput::MoveRight,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandlingState {
    left_held: bool,
    right_held: bool,
    soft_drop_held: bool,
    active_horizontal: Option<HorizontalDirection>,
    das_elapsed: u16,
    arr_elapsed: u16,
    dcd_remaining: u16,
    hold_held: bool,
    rotation_held: [bool; 3],
    rotation_pressed_at: [u64; 3],
    input_sequence: u64,
}

impl HandlingState {
    pub const fn new() -> Self {
        Self {
            left_held: false,
            right_held: false,
            soft_drop_held: false,
            active_horizontal: None,
            das_elapsed: 0,
            arr_elapsed: 0,
            dcd_remaining: 0,
            hold_held: false,
            rotation_held: [false; 3],
            rotation_pressed_at: [0; 3],
            input_sequence: 0,
        }
    }

    pub const fn dcd_remaining(self) -> u16 {
        self.dcd_remaining
    }

    pub const fn das_elapsed(self) -> u16 {
        self.das_elapsed
    }
}

impl Default for HandlingState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedFrame {
    pub actions: Vec<FrameInput>,
    pub hold_requested: bool,
    /// Number of piece actions that appeared before the first hold press.
    pub hold_action_index: Option<usize>,
}

/// Held initial actions sampled when a new piece spawns.
///
/// The generic contract resolves IHS before IRS. Whether TETR.IO samples these
/// keys before or after other same-frame stages remains replay-fixture-gated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitialActions {
    pub hold_requested: bool,
    pub rotation: Option<RotationDirection>,
}

impl InitialActions {
    pub const fn new(hold_requested: bool, rotation: Option<RotationDirection>) -> Self {
        Self {
            hold_requested,
            rotation,
        }
    }
}

/// Converts ordered input edges and held state into ordered discrete actions.
///
/// This establishes a deterministic generic contract. TETR.IO-specific event
/// sampling and stage order still require differential fixtures; callers must
/// not present this function alone as target conformance.
pub fn normalize_frame(
    state: &mut HandlingState,
    rules: HandlingRules,
    edges: &[InputEdge],
) -> NormalizedFrame {
    let mut actions = Vec::new();
    let mut horizontal_pressed = false;
    let mut hold_requested = false;
    let mut hold_action_index = None;

    for edge in edges {
        match (edge.button, edge.kind) {
            (InputButton::Left, InputEdgeKind::Press) => {
                state.left_held = true;
                activate_horizontal(state, HorizontalDirection::Left);
                actions.push(FrameInput::MoveLeft);
                horizontal_pressed = true;
            }
            (InputButton::Right, InputEdgeKind::Press) => {
                state.right_held = true;
                activate_horizontal(state, HorizontalDirection::Right);
                actions.push(FrameInput::MoveRight);
                horizontal_pressed = true;
            }
            (InputButton::Left, InputEdgeKind::Release) => {
                state.left_held = false;
                release_horizontal(state, HorizontalDirection::Left);
            }
            (InputButton::Right, InputEdgeKind::Release) => {
                state.right_held = false;
                release_horizontal(state, HorizontalDirection::Right);
            }
            (InputButton::SoftDrop, InputEdgeKind::Press) => state.soft_drop_held = true,
            (InputButton::SoftDrop, InputEdgeKind::Release) => state.soft_drop_held = false,
            (InputButton::HardDrop, InputEdgeKind::Press) => actions.push(FrameInput::HardDrop),
            (InputButton::RotateClockwise, InputEdgeKind::Press) => {
                press_rotation(state, RotationDirection::Clockwise);
                actions.push(FrameInput::RotateClockwise);
                state.dcd_remaining = rules.dcd_frames;
            }
            (InputButton::RotateCounterclockwise, InputEdgeKind::Press) => {
                press_rotation(state, RotationDirection::Counterclockwise);
                actions.push(FrameInput::RotateCounterclockwise);
                state.dcd_remaining = rules.dcd_frames;
            }
            (InputButton::RotateHalf, InputEdgeKind::Press) => {
                press_rotation(state, RotationDirection::Half);
                actions.push(FrameInput::RotateHalf);
                state.dcd_remaining = rules.dcd_frames;
            }
            (InputButton::Hold, InputEdgeKind::Press) => {
                state.hold_held = true;
                hold_requested = true;
                hold_action_index.get_or_insert(actions.len());
            }
            (InputButton::RotateClockwise, InputEdgeKind::Release) => {
                release_rotation(state, RotationDirection::Clockwise);
            }
            (InputButton::RotateCounterclockwise, InputEdgeKind::Release) => {
                release_rotation(state, RotationDirection::Counterclockwise);
            }
            (InputButton::RotateHalf, InputEdgeKind::Release) => {
                release_rotation(state, RotationDirection::Half);
            }
            (InputButton::Hold, InputEdgeKind::Release) => state.hold_held = false,
            (InputButton::HardDrop, InputEdgeKind::Release) => {}
        }
    }

    advance_horizontal(state, rules, horizontal_pressed, &mut actions);
    append_soft_drop(state, rules, &mut actions);

    NormalizedFrame {
        actions,
        hold_requested,
        hold_action_index,
    }
}

/// Applies the documented DCD spawn pause while retaining held directions and
/// their accumulated DAS charge.
pub fn on_piece_spawn(state: &mut HandlingState, rules: HandlingRules) {
    state.dcd_remaining = rules.dcd_frames;
}

/// Samples held IHS/IRS inputs and applies the documented generic spawn DCD.
pub fn initial_actions_on_spawn(state: &mut HandlingState, rules: HandlingRules) -> InitialActions {
    on_piece_spawn(state, rules);
    InitialActions::new(state.hold_held, active_rotation(state))
}

fn press_rotation(state: &mut HandlingState, direction: RotationDirection) {
    state.input_sequence = state.input_sequence.saturating_add(1);
    let index = rotation_index(direction);
    state.rotation_held[index] = true;
    state.rotation_pressed_at[index] = state.input_sequence;
}

fn release_rotation(state: &mut HandlingState, direction: RotationDirection) {
    state.rotation_held[rotation_index(direction)] = false;
}

fn active_rotation(state: &HandlingState) -> Option<RotationDirection> {
    [
        RotationDirection::Clockwise,
        RotationDirection::Counterclockwise,
        RotationDirection::Half,
    ]
    .into_iter()
    .filter(|direction| state.rotation_held[rotation_index(*direction)])
    .max_by_key(|direction| state.rotation_pressed_at[rotation_index(*direction)])
}

const fn rotation_index(direction: RotationDirection) -> usize {
    match direction {
        RotationDirection::Clockwise => 0,
        RotationDirection::Counterclockwise => 1,
        RotationDirection::Half => 2,
    }
}

fn activate_horizontal(state: &mut HandlingState, direction: HorizontalDirection) {
    state.active_horizontal = Some(direction);
    state.das_elapsed = 0;
    state.arr_elapsed = 0;
}

fn release_horizontal(state: &mut HandlingState, direction: HorizontalDirection) {
    if state.active_horizontal != Some(direction) {
        return;
    }
    state.active_horizontal = match direction {
        HorizontalDirection::Left if state.right_held => Some(HorizontalDirection::Right),
        HorizontalDirection::Right if state.left_held => Some(HorizontalDirection::Left),
        HorizontalDirection::Left | HorizontalDirection::Right => None,
    };
    state.das_elapsed = 0;
    state.arr_elapsed = 0;
}

fn advance_horizontal(
    state: &mut HandlingState,
    rules: HandlingRules,
    horizontal_pressed: bool,
    actions: &mut Vec<FrameInput>,
) {
    let Some(direction) = state.active_horizontal else {
        return;
    };

    if state.dcd_remaining > 0 {
        state.dcd_remaining -= 1;
        return;
    }

    state.das_elapsed = state.das_elapsed.saturating_add(1);
    if state.das_elapsed < rules.das_frames {
        return;
    }

    if state.das_elapsed == rules.das_frames && !horizontal_pressed {
        append_horizontal_repeat(actions, direction, rules.arr_frames);
        return;
    }
    if rules.das_frames == 0 && horizontal_pressed {
        append_horizontal_repeat(actions, direction, rules.arr_frames);
        return;
    }
    if rules.arr_frames == 0 {
        append_horizontal_repeat(actions, direction, 0);
        return;
    }

    state.arr_elapsed = state.arr_elapsed.saturating_add(1);
    if state.arr_elapsed >= rules.arr_frames {
        actions.push(direction.action());
        state.arr_elapsed = 0;
    }
}

fn append_horizontal_repeat(
    actions: &mut Vec<FrameInput>,
    direction: HorizontalDirection,
    arr_frames: u16,
) {
    let repeats = if arr_frames == 0 { WIDTH } else { 1 };
    actions.extend(std::iter::repeat_n(direction.action(), repeats));
}

fn append_soft_drop(state: &HandlingState, rules: HandlingRules, actions: &mut Vec<FrameInput>) {
    if !state.soft_drop_held {
        return;
    }
    let cells = match rules.soft_drop {
        SoftDropMode::Disabled => 0,
        SoftDropMode::CellsPerFrame(cells) => usize::from(cells),
        SoftDropMode::Sonic => HEIGHT,
    };
    actions.extend(std::iter::repeat_n(FrameInput::SoftDropCell, cells));
}

#[cfg(test)]
mod tests {
    use super::{
        HandlingRules, HandlingState, InputButton, InputEdge, SoftDropMode,
        initial_actions_on_spawn, normalize_frame, on_piece_spawn,
    };
    use crate::{FrameInput, HEIGHT, RotationDirection, WIDTH};

    fn rules(das: u16, arr: u16, dcd: u16) -> HandlingRules {
        HandlingRules::new(das, arr, dcd, SoftDropMode::Disabled)
    }

    #[test]
    fn ordered_same_frame_presses_are_preserved() {
        let mut state = HandlingState::new();
        let result = normalize_frame(
            &mut state,
            rules(10, 2, 0),
            &[
                InputEdge::press(InputButton::Left),
                InputEdge::press(InputButton::Right),
            ],
        );

        assert_eq!(
            result.actions,
            vec![FrameInput::MoveLeft, FrameInput::MoveRight]
        );
    }

    #[test]
    fn das_then_arr_emit_at_declared_boundaries() {
        let mut state = HandlingState::new();
        let timing = rules(3, 2, 0);

        assert_eq!(
            normalize_frame(&mut state, timing, &[InputEdge::press(InputButton::Left)]).actions,
            vec![FrameInput::MoveLeft]
        );
        assert!(normalize_frame(&mut state, timing, &[]).actions.is_empty());
        assert_eq!(
            normalize_frame(&mut state, timing, &[]).actions,
            vec![FrameInput::MoveLeft]
        );
        assert!(normalize_frame(&mut state, timing, &[]).actions.is_empty());
        assert_eq!(
            normalize_frame(&mut state, timing, &[]).actions,
            vec![FrameInput::MoveLeft]
        );
    }

    #[test]
    fn zero_arr_expands_to_a_wall_reaching_action_bound() {
        let mut state = HandlingState::new();
        let result = normalize_frame(
            &mut state,
            rules(0, 0, 0),
            &[InputEdge::press(InputButton::Right)],
        );

        assert_eq!(result.actions.len(), WIDTH + 1);
        assert!(
            result
                .actions
                .iter()
                .all(|action| *action == FrameInput::MoveRight)
        );
    }

    #[test]
    fn rotation_and_spawn_pause_das_without_erasing_charge() {
        let mut state = HandlingState::new();
        let timing = rules(3, 1, 2);

        normalize_frame(&mut state, timing, &[InputEdge::press(InputButton::Left)]);
        assert_eq!(state.das_elapsed(), 1);

        let rotation = normalize_frame(
            &mut state,
            timing,
            &[InputEdge::press(InputButton::RotateClockwise)],
        );
        assert_eq!(rotation.actions, vec![FrameInput::RotateClockwise]);
        assert_eq!(state.das_elapsed(), 1);
        assert_eq!(state.dcd_remaining(), 1);

        on_piece_spawn(&mut state, timing);
        normalize_frame(&mut state, timing, &[]);
        normalize_frame(&mut state, timing, &[]);
        assert_eq!(state.das_elapsed(), 1);
        assert!(normalize_frame(&mut state, timing, &[]).actions.is_empty());
        assert_eq!(
            normalize_frame(&mut state, timing, &[]).actions,
            vec![FrameInput::MoveLeft]
        );
    }

    #[test]
    fn sonic_drop_expands_to_board_height_without_locking() {
        let mut state = HandlingState::new();
        let timing = HandlingRules::new(10, 2, 0, SoftDropMode::Sonic);
        let result = normalize_frame(
            &mut state,
            timing,
            &[InputEdge::press(InputButton::SoftDrop)],
        );

        assert_eq!(result.actions.len(), HEIGHT);
        assert!(
            result
                .actions
                .iter()
                .all(|action| *action == FrameInput::SoftDropCell)
        );
    }

    #[test]
    fn hold_press_is_exposed_to_the_game_layer() {
        let mut state = HandlingState::new();
        let result = normalize_frame(
            &mut state,
            rules(10, 2, 0),
            &[InputEdge::press(InputButton::Hold)],
        );

        assert!(result.hold_requested);
        assert!(result.actions.is_empty());
        assert_eq!(result.hold_action_index, Some(0));
    }

    #[test]
    fn hold_position_preserves_same_frame_event_order() {
        let mut state = HandlingState::new();
        let result = normalize_frame(
            &mut state,
            rules(10, 2, 0),
            &[
                InputEdge::press(InputButton::RotateClockwise),
                InputEdge::press(InputButton::Hold),
                InputEdge::press(InputButton::Right),
            ],
        );

        assert_eq!(result.hold_action_index, Some(1));
        assert_eq!(
            result.actions,
            vec![FrameInput::RotateClockwise, FrameInput::MoveRight]
        );
    }

    #[test]
    fn held_initial_actions_use_the_most_recent_rotation() {
        let mut state = HandlingState::new();
        let timing = rules(10, 2, 3);
        normalize_frame(
            &mut state,
            timing,
            &[
                InputEdge::press(InputButton::RotateClockwise),
                InputEdge::press(InputButton::RotateHalf),
                InputEdge::press(InputButton::Hold),
            ],
        );

        let initial = initial_actions_on_spawn(&mut state, timing);
        assert!(initial.hold_requested);
        assert_eq!(initial.rotation, Some(RotationDirection::Half));
        assert_eq!(state.dcd_remaining(), 3);

        normalize_frame(
            &mut state,
            timing,
            &[
                InputEdge::release(InputButton::RotateHalf),
                InputEdge::release(InputButton::Hold),
            ],
        );
        let released = initial_actions_on_spawn(&mut state, timing);
        assert!(!released.hold_requested);
        assert_eq!(released.rotation, Some(RotationDirection::Clockwise));
    }
}
