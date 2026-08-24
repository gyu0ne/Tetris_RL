use crate::{
    Board, HEIGHT, Movement, PieceState, RotationDirection, hard_drop, try_movement, try_rotate,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gravity {
    numerator: u32,
    denominator: u32,
}

impl Gravity {
    pub const fn new(numerator: u32, denominator: u32) -> Result<Self, TimingConfigError> {
        if denominator == 0 {
            return Err(TimingConfigError::ZeroGravityDenominator);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingConfigError {
    ZeroGravityDenominator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingRules {
    pub gravity: Gravity,
    pub lock_delay_frames: u16,
    pub max_lock_resets: u16,
    pub reset_on_lateral_move: bool,
    pub reset_on_rotation: bool,
}

impl TimingRules {
    pub const fn new(
        gravity: Gravity,
        lock_delay_frames: u16,
        max_lock_resets: u16,
        reset_on_lateral_move: bool,
        reset_on_rotation: bool,
    ) -> Self {
        Self {
            gravity,
            lock_delay_frames,
            max_lock_resets,
            reset_on_lateral_move,
            reset_on_rotation,
        }
    }
}

/// Discrete actions already normalized into their execution order for one frame.
///
/// Held-key interpretation (DAS/ARR/DCD), IRS/IHS buffering, and same-frame
/// conflict resolution occur before this timing kernel. It consumes the
/// resulting ordered action list without inventing an upstream order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameInput {
    MoveLeft,
    MoveRight,
    SoftDropCell,
    RotateClockwise,
    RotateCounterclockwise,
    RotateHalf,
    HardDrop,
}

/// Last successful player action that can affect spin classification.
///
/// Automatic gravity does not overwrite this value. A hard drop only replaces
/// it when the piece actually changes rows, so a zero-distance lock preserves
/// a rotation performed at the final position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LastAction {
    None,
    Translation,
    SoftDrop,
    HardDrop,
    Rotation {
        direction: RotationDirection,
        kick_index: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingState {
    pub piece: PieceState,
    pub gravity_accumulator: u64,
    pub lock_elapsed_frames: u16,
    pub lock_resets_used: u16,
    pub locked: bool,
    pub last_action: LastAction,
}

impl TimingState {
    pub fn new(board: &Board, piece: PieceState) -> Result<Self, TimingStepError> {
        if board.collides(piece) {
            return Err(TimingStepError::CollidingPiece);
        }
        Ok(Self {
            piece,
            gravity_accumulator: 0,
            lock_elapsed_frames: 0,
            lock_resets_used: 0,
            locked: false,
            last_action: LastAction::None,
        })
    }

    pub fn with_last_action(
        board: &Board,
        piece: PieceState,
        last_action: LastAction,
    ) -> Result<Self, TimingStepError> {
        let mut state = Self::new(board, piece)?;
        state.last_action = last_action;
        Ok(state)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameOutcome {
    pub piece: PieceState,
    pub locked: bool,
    pub grounded: bool,
    pub successful_inputs: u16,
    pub gravity_rows: u16,
    pub lock_elapsed_frames: u16,
    pub lock_resets_used: u16,
    pub last_action: LastAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingStepError {
    AlreadyLocked,
    CollidingPiece,
    GravityAccumulatorOverflow,
}

pub fn step_frame(
    board: &Board,
    state: &mut TimingState,
    rules: TimingRules,
    inputs: &[FrameInput],
) -> Result<FrameOutcome, TimingStepError> {
    if state.locked {
        return Err(TimingStepError::AlreadyLocked);
    }
    if board.collides(state.piece) {
        return Err(TimingStepError::CollidingPiece);
    }

    let mut successful_inputs = 0_u16;
    for input in inputs {
        if *input == FrameInput::HardDrop {
            let dropped = hard_drop(board, state.piece).ok_or(TimingStepError::CollidingPiece)?;
            if dropped != state.piece {
                state.last_action = LastAction::HardDrop;
            }
            state.piece = dropped;
            state.locked = true;
            successful_inputs = successful_inputs.saturating_add(1);
            return Ok(outcome(board, *state, successful_inputs, 0));
        }

        let grounded_before = is_grounded(board, state.piece);
        let applied = apply_input(board, state.piece, *input);
        let Some(applied) = applied else {
            continue;
        };

        state.piece = applied.piece;
        state.last_action = applied.last_action;
        successful_inputs = successful_inputs.saturating_add(1);
        if grounded_before && resets_lock(*input, rules) {
            try_reset_lock(state, rules);
        }
    }

    state.gravity_accumulator = state
        .gravity_accumulator
        .checked_add(u64::from(rules.gravity.numerator()))
        .ok_or(TimingStepError::GravityAccumulatorOverflow)?;
    let gravity_rows_requested = state.gravity_accumulator / u64::from(rules.gravity.denominator());
    state.gravity_accumulator %= u64::from(rules.gravity.denominator());

    let mut gravity_rows = 0_u16;
    for _ in 0..gravity_rows_requested.min(HEIGHT as u64) {
        let Some(next) = try_movement(board, state.piece, Movement::Down) else {
            break;
        };
        state.piece = next;
        gravity_rows += 1;
    }

    let grounded = is_grounded(board, state.piece);
    if grounded {
        if rules.lock_delay_frames == 0 {
            state.locked = true;
        } else {
            state.lock_elapsed_frames = state.lock_elapsed_frames.saturating_add(1);
            state.locked = state.lock_elapsed_frames >= rules.lock_delay_frames;
        }
    } else {
        state.lock_elapsed_frames = 0;
    }

    Ok(outcome(board, *state, successful_inputs, gravity_rows))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AppliedInput {
    piece: PieceState,
    last_action: LastAction,
}

fn apply_input(board: &Board, piece: PieceState, input: FrameInput) -> Option<AppliedInput> {
    match input {
        FrameInput::MoveLeft => {
            try_movement(board, piece, Movement::Left).map(|piece| AppliedInput {
                piece,
                last_action: LastAction::Translation,
            })
        }
        FrameInput::MoveRight => {
            try_movement(board, piece, Movement::Right).map(|piece| AppliedInput {
                piece,
                last_action: LastAction::Translation,
            })
        }
        FrameInput::SoftDropCell => {
            try_movement(board, piece, Movement::Down).map(|piece| AppliedInput {
                piece,
                last_action: LastAction::SoftDrop,
            })
        }
        FrameInput::RotateClockwise => apply_rotation(board, piece, RotationDirection::Clockwise),
        FrameInput::RotateCounterclockwise => {
            apply_rotation(board, piece, RotationDirection::Counterclockwise)
        }
        FrameInput::RotateHalf => apply_rotation(board, piece, RotationDirection::Half),
        FrameInput::HardDrop => unreachable!("hard drop is handled before discrete movement"),
    }
}

fn apply_rotation(
    board: &Board,
    piece: PieceState,
    direction: RotationDirection,
) -> Option<AppliedInput> {
    try_rotate(board, piece, direction).map(|result| AppliedInput {
        piece: result.state,
        last_action: LastAction::Rotation {
            direction: result.direction,
            kick_index: result.kick_index,
        },
    })
}

fn resets_lock(input: FrameInput, rules: TimingRules) -> bool {
    match input {
        FrameInput::MoveLeft | FrameInput::MoveRight => rules.reset_on_lateral_move,
        FrameInput::RotateClockwise
        | FrameInput::RotateCounterclockwise
        | FrameInput::RotateHalf => rules.reset_on_rotation,
        FrameInput::SoftDropCell | FrameInput::HardDrop => false,
    }
}

fn try_reset_lock(state: &mut TimingState, rules: TimingRules) {
    if state.lock_resets_used >= rules.max_lock_resets {
        return;
    }
    state.lock_elapsed_frames = 0;
    state.lock_resets_used += 1;
}

fn is_grounded(board: &Board, piece: PieceState) -> bool {
    try_movement(board, piece, Movement::Down).is_none()
}

fn outcome(
    board: &Board,
    state: TimingState,
    successful_inputs: u16,
    gravity_rows: u16,
) -> FrameOutcome {
    FrameOutcome {
        piece: state.piece,
        locked: state.locked,
        grounded: is_grounded(board, state.piece),
        successful_inputs,
        gravity_rows,
        lock_elapsed_frames: state.lock_elapsed_frames,
        lock_resets_used: state.lock_resets_used,
        last_action: state.last_action,
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameInput, Gravity, LastAction, TimingRules, TimingState, step_frame};
    use crate::{Board, Orientation, PieceKind, PieceState, RotationDirection};

    fn rules(
        numerator: u32,
        denominator: u32,
        lock_delay_frames: u16,
        max_lock_resets: u16,
    ) -> TimingRules {
        TimingRules::new(
            Gravity::new(numerator, denominator).expect("test gravity is valid"),
            lock_delay_frames,
            max_lock_resets,
            true,
            true,
        )
    }

    fn grounded_o() -> PieceState {
        PieceState::new(PieceKind::O, Orientation::Spawn, 3, -1)
    }

    #[test]
    fn rational_gravity_accumulates_without_floats() {
        let board = Board::empty();
        let mut state = TimingState::new(
            &board,
            PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18),
        )
        .expect("spawn is valid");

        let first =
            step_frame(&board, &mut state, rules(1, 2, 30, 0), &[]).expect("first frame advances");
        let second =
            step_frame(&board, &mut state, rules(1, 2, 30, 0), &[]).expect("second frame advances");

        assert_eq!(first.piece.y, 18);
        assert_eq!(first.gravity_rows, 0);
        assert_eq!(second.piece.y, 17);
        assert_eq!(second.gravity_rows, 1);
    }

    #[test]
    fn twenty_g_reaches_the_floor_in_one_frame() {
        let board = Board::empty();
        let mut state = TimingState::new(
            &board,
            PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18),
        )
        .expect("spawn is valid");

        let result =
            step_frame(&board, &mut state, rules(20, 1, 30, 0), &[]).expect("frame advances");

        assert_eq!(result.piece.y, -1);
        assert_eq!(result.gravity_rows, 19);
        assert!(result.grounded);
        assert!(!result.locked);
    }

    #[test]
    fn grounded_piece_locks_on_the_configured_frame() {
        let board = Board::empty();
        let mut state = TimingState::new(&board, grounded_o()).expect("piece is valid");
        let timing = rules(0, 1, 3, 0);

        assert!(!step_frame(&board, &mut state, timing, &[]).unwrap().locked);
        assert!(!step_frame(&board, &mut state, timing, &[]).unwrap().locked);
        let third = step_frame(&board, &mut state, timing, &[]).unwrap();

        assert!(third.locked);
        assert_eq!(third.lock_elapsed_frames, 3);
    }

    #[test]
    fn grounded_lateral_move_resets_lock_once() {
        let board = Board::empty();
        let mut state = TimingState::new(&board, grounded_o()).expect("piece is valid");
        let timing = rules(0, 1, 3, 1);

        step_frame(&board, &mut state, timing, &[]).unwrap();
        let moved = step_frame(&board, &mut state, timing, &[FrameInput::MoveLeft]).unwrap();
        assert_eq!(moved.lock_elapsed_frames, 1);
        assert_eq!(moved.lock_resets_used, 1);

        let capped = step_frame(&board, &mut state, timing, &[FrameInput::MoveRight]).unwrap();
        assert_eq!(capped.lock_elapsed_frames, 2);
        assert_eq!(capped.lock_resets_used, 1);
        assert!(step_frame(&board, &mut state, timing, &[]).unwrap().locked);
    }

    #[test]
    fn hard_drop_locks_immediately() {
        let board = Board::empty();
        let mut state = TimingState::new(
            &board,
            PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18),
        )
        .expect("spawn is valid");

        let result = step_frame(
            &board,
            &mut state,
            rules(0, 1, 30, 15),
            &[FrameInput::HardDrop],
        )
        .expect("hard drop succeeds");

        assert!(result.locked);
        assert!(result.grounded);
        assert_eq!(result.piece.y, -1);
        assert_eq!(result.last_action, LastAction::HardDrop);
    }

    #[test]
    fn identical_ordered_inputs_reproduce_state() {
        let board = Board::empty();
        let spawn = PieceState::new(PieceKind::L, Orientation::Spawn, 3, 18);
        let timing = rules(1, 2, 30, 15);
        let frames = [
            vec![FrameInput::MoveLeft, FrameInput::RotateClockwise],
            vec![FrameInput::MoveRight],
            vec![FrameInput::SoftDropCell, FrameInput::RotateHalf],
            vec![],
        ];
        let mut left = TimingState::new(&board, spawn).unwrap();
        let mut right = TimingState::new(&board, spawn).unwrap();

        for inputs in frames {
            let a = step_frame(&board, &mut left, timing, &inputs).unwrap();
            let b = step_frame(&board, &mut right, timing, &inputs).unwrap();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn successful_rotation_preserves_direction_and_kick_until_next_player_move() {
        let board = Board::empty();
        let mut state = TimingState::new(
            &board,
            PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18),
        )
        .expect("spawn is valid");
        let timing = rules(0, 1, 30, 15);

        let rotated = step_frame(&board, &mut state, timing, &[FrameInput::RotateClockwise])
            .expect("rotation succeeds");
        assert_eq!(
            rotated.last_action,
            LastAction::Rotation {
                direction: RotationDirection::Clockwise,
                kick_index: 0,
            }
        );

        let moved = step_frame(&board, &mut state, timing, &[FrameInput::MoveLeft])
            .expect("translation succeeds");
        assert_eq!(moved.last_action, LastAction::Translation);
    }

    #[test]
    fn zero_distance_hard_drop_preserves_rotation_metadata() {
        let board = Board::empty();
        let rotation = LastAction::Rotation {
            direction: RotationDirection::Counterclockwise,
            kick_index: 2,
        };
        let mut state = TimingState::with_last_action(&board, grounded_o(), rotation)
            .expect("grounded piece is valid");

        let outcome = step_frame(
            &board,
            &mut state,
            rules(0, 1, 30, 15),
            &[FrameInput::HardDrop],
        )
        .expect("zero-distance hard drop locks");

        assert!(outcome.locked);
        assert_eq!(outcome.last_action, rotation);
    }
}
