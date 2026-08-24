use crate::{
    Board, HEIGHT, Movement, PieceState, RotationDirection, hard_drop, try_movement, try_rotate,
};

pub const FALL_MICROS_PER_CELL: u32 = 1_000_000;
pub const TETRIO_KICK_FALL_FRACTION_MICROS: u32 = 100_000;

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

/// Per-frame timing provider whose linear variant keeps one denominator for
/// the exact schedule before each frame is quantized to current-client
/// millionth-cell fall units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingSchedule {
    Fixed(TimingRules),
    LinearGravity(LinearGravityTiming),
}

impl TimingSchedule {
    pub const fn fixed(rules: TimingRules) -> Self {
        Self::Fixed(rules)
    }

    pub const fn linear_gravity(schedule: LinearGravityTiming) -> Self {
        Self::LinearGravity(schedule)
    }

    pub fn rules_at_frame(self, elapsed_frames: u64) -> Result<TimingRules, TimingScheduleError> {
        match self {
            Self::Fixed(rules) => Ok(rules),
            Self::LinearGravity(schedule) => schedule.rules_at_frame(elapsed_frames),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinearGravityTiming {
    pub base_rules: TimingRules,
    pub increase_per_second: Gravity,
    pub margin_frames: u64,
    pub tick_rate_hz: u32,
    pub cap: Gravity,
}

impl LinearGravityTiming {
    pub const fn new(
        base_rules: TimingRules,
        increase_per_second: Gravity,
        margin_frames: u64,
        tick_rate_hz: u32,
        cap: Gravity,
    ) -> Result<Self, TimingScheduleError> {
        if tick_rate_hz == 0 {
            return Err(TimingScheduleError::ZeroTickRate);
        }
        Ok(Self {
            base_rules,
            increase_per_second,
            margin_frames,
            tick_rate_hz,
            cap,
        })
    }

    pub fn rules_at_frame(self, elapsed_frames: u64) -> Result<TimingRules, TimingScheduleError> {
        // Current client mutates `g` at the end of a frame only when
        // `frame > gmargin`; actions on `margin + 1` still see the base value.
        let increase_frames = elapsed_frames.saturating_sub(self.margin_frames.saturating_add(1));
        let gravity = add_scaled_per_second(
            self.base_rules.gravity,
            self.increase_per_second,
            increase_frames,
            self.tick_rate_hz,
            self.cap,
        )?;
        Ok(TimingRules {
            gravity,
            ..self.base_rules
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingScheduleError {
    ZeroTickRate,
    ArithmeticOverflow,
    InvalidGravity(TimingConfigError),
}

fn add_scaled_per_second(
    base: Gravity,
    increase_per_second: Gravity,
    elapsed_frames: u64,
    tick_rate_hz: u32,
    cap: Gravity,
) -> Result<Gravity, TimingScheduleError> {
    if tick_rate_hz == 0 {
        return Err(TimingScheduleError::ZeroTickRate);
    }

    let base_numerator = u128::from(base.numerator());
    let base_denominator = u128::from(base.denominator());
    let increase_numerator = u128::from(increase_per_second.numerator());
    let increase_denominator = u128::from(increase_per_second.denominator());
    let ticks = u128::from(tick_rate_hz);
    let elapsed = u128::from(elapsed_frames);

    let common_denominator = base_denominator
        .checked_mul(increase_denominator)
        .and_then(|value| value.checked_mul(ticks))
        .and_then(|value| value.checked_mul(u128::from(cap.denominator())))
        .ok_or(TimingScheduleError::ArithmeticOverflow)?;
    let base_scaled = base_numerator
        .checked_mul(increase_denominator)
        .and_then(|value| value.checked_mul(ticks))
        .and_then(|value| value.checked_mul(u128::from(cap.denominator())))
        .ok_or(TimingScheduleError::ArithmeticOverflow)?;
    let increase_per_frame_scaled = increase_numerator
        .checked_mul(base_denominator)
        .and_then(|value| value.checked_mul(u128::from(cap.denominator())))
        .ok_or(TimingScheduleError::ArithmeticOverflow)?;
    let cap_scaled = u128::from(cap.numerator())
        .checked_mul(base_denominator)
        .and_then(|value| value.checked_mul(increase_denominator))
        .and_then(|value| value.checked_mul(ticks))
        .ok_or(TimingScheduleError::ArithmeticOverflow)?;

    let schedule_divisor = gcd_u128(
        gcd_u128(base_scaled, increase_per_frame_scaled),
        gcd_u128(common_denominator, cap_scaled),
    );
    let denominator = common_denominator / schedule_divisor;
    let numerator = (base_scaled / schedule_divisor)
        .checked_add(
            (increase_per_frame_scaled / schedule_divisor)
                .checked_mul(elapsed)
                .ok_or(TimingScheduleError::ArithmeticOverflow)?,
        )
        .ok_or(TimingScheduleError::ArithmeticOverflow)?
        .min(cap_scaled / schedule_divisor);

    Gravity::new(
        u32::try_from(numerator).map_err(|_| TimingScheduleError::ArithmeticOverflow)?,
        u32::try_from(denominator).map_err(|_| TimingScheduleError::ArithmeticOverflow)?,
    )
    .map_err(TimingScheduleError::InvalidGravity)
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
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
    /// Fractional downward y phase, quantized like the current client to one
    /// millionth of a cell. The integer `PieceState` stores `ceil(y)`.
    pub fall_fraction_micros: u32,
    pub lock_elapsed_frames: u16,
    pub lock_resets_used: u16,
    pub locked: bool,
    pub last_action: LastAction,
}

impl TimingState {
    pub fn new(board: &Board, piece: PieceState) -> Result<Self, TimingStepError> {
        Self::new_with_fall_fraction(board, piece, 0)
    }

    pub fn new_with_fall_fraction(
        board: &Board,
        piece: PieceState,
        fall_fraction_micros: u32,
    ) -> Result<Self, TimingStepError> {
        if board.collides(piece) {
            return Err(TimingStepError::CollidingPiece);
        }
        if fall_fraction_micros >= FALL_MICROS_PER_CELL {
            return Err(TimingStepError::InvalidFallFraction);
        }
        Ok(Self {
            piece,
            fall_fraction_micros,
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
        Self::with_last_action_and_fall_fraction(board, piece, last_action, 0)
    }

    pub fn with_last_action_and_fall_fraction(
        board: &Board,
        piece: PieceState,
        last_action: LastAction,
        fall_fraction_micros: u32,
    ) -> Result<Self, TimingStepError> {
        let mut state = Self::new_with_fall_fraction(board, piece, fall_fraction_micros)?;
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
    FallArithmeticOverflow,
    InvalidFallFraction,
}

pub fn step_frame(
    board: &Board,
    state: &mut TimingState,
    rules: TimingRules,
    inputs: &[FrameInput],
) -> Result<FrameOutcome, TimingStepError> {
    let input_phase = apply_input_phase(board, state, rules, inputs)?;
    if input_phase.locked {
        return Ok(outcome(board, *state, input_phase.successful_inputs, 0));
    }
    advance_after_inputs(board, state, rules, input_phase.successful_inputs)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InputPhaseOutcome {
    pub successful_inputs: u16,
    pub locked: bool,
}

pub(crate) fn apply_input_phase(
    board: &Board,
    state: &mut TimingState,
    rules: TimingRules,
    inputs: &[FrameInput],
) -> Result<InputPhaseOutcome, TimingStepError> {
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
            return Ok(InputPhaseOutcome {
                successful_inputs,
                locked: true,
            });
        }

        let grounded_before = is_grounded(board, state.piece);
        let applied = apply_input(board, state.piece, *input);
        let Some(applied) = applied else {
            continue;
        };

        state.piece = applied.piece;
        state.last_action = applied.last_action;
        if applied.used_kick {
            state.fall_fraction_micros = TETRIO_KICK_FALL_FRACTION_MICROS;
        }
        successful_inputs = successful_inputs.saturating_add(1);
        if grounded_before && resets_lock(*input, rules) {
            try_reset_lock(state, rules);
        }
    }

    Ok(InputPhaseOutcome {
        successful_inputs,
        locked: false,
    })
}

pub(crate) fn advance_after_inputs(
    board: &Board,
    state: &mut TimingState,
    rules: TimingRules,
    successful_inputs: u16,
) -> Result<FrameOutcome, TimingStepError> {
    let (gravity_rows, fall_failed) = apply_gravity(board, state, rules.gravity)?;

    let grounded = is_grounded(board, state.piece);
    if fall_failed {
        state.lock_elapsed_frames = state.lock_elapsed_frames.saturating_add(1);
        state.locked = state.lock_elapsed_frames > rules.lock_delay_frames
            || state.lock_resets_used >= rules.max_lock_resets;
    } else {
        if !grounded {
            state.lock_elapsed_frames = 0;
        }
        if gravity_rows > 0 {
            state.lock_resets_used = 0;
        }
    }

    Ok(outcome(board, *state, successful_inputs, gravity_rows))
}

pub(crate) fn input_phase_outcome(
    board: &Board,
    state: TimingState,
    successful_inputs: u16,
) -> FrameOutcome {
    outcome(board, state, successful_inputs, 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AppliedInput {
    piece: PieceState,
    last_action: LastAction,
    used_kick: bool,
}

fn apply_input(board: &Board, piece: PieceState, input: FrameInput) -> Option<AppliedInput> {
    match input {
        FrameInput::MoveLeft => {
            try_movement(board, piece, Movement::Left).map(|piece| AppliedInput {
                piece,
                last_action: LastAction::Translation,
                used_kick: false,
            })
        }
        FrameInput::MoveRight => {
            try_movement(board, piece, Movement::Right).map(|piece| AppliedInput {
                piece,
                last_action: LastAction::Translation,
                used_kick: false,
            })
        }
        FrameInput::SoftDropCell => {
            try_movement(board, piece, Movement::Down).map(|piece| AppliedInput {
                piece,
                last_action: LastAction::SoftDrop,
                used_kick: false,
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
        used_kick: result.used_kick,
    })
}

fn apply_gravity(
    board: &Board,
    state: &mut TimingState,
    gravity: Gravity,
) -> Result<(u16, bool), TimingStepError> {
    let scaled = u64::from(gravity.numerator())
        .checked_mul(u64::from(FALL_MICROS_PER_CELL))
        .ok_or(TimingStepError::FallArithmeticOverflow)?;
    let denominator = u64::from(gravity.denominator());
    let gravity_micros = scaled
        .checked_add(denominator / 2)
        .ok_or(TimingStepError::FallArithmeticOverflow)?
        / denominator;

    let full_cells = gravity_micros / u64::from(FALL_MICROS_PER_CELL);
    let remainder = gravity_micros % u64::from(FALL_MICROS_PER_CELL);
    let mut rows = 0_u16;

    for _ in 0..full_cells.min(HEIGHT as u64) {
        let Some(next) = try_movement(board, state.piece, Movement::Down) else {
            return Ok((rows, true));
        };
        state.piece = next;
        rows = rows.saturating_add(1);
    }

    if remainder == 0 {
        return Ok((rows, false));
    }
    if is_grounded(board, state.piece) {
        return Ok((rows, true));
    }

    let phase = u64::from(state.fall_fraction_micros)
        .checked_add(remainder)
        .ok_or(TimingStepError::FallArithmeticOverflow)?;
    if phase >= u64::from(FALL_MICROS_PER_CELL) {
        state.piece = try_movement(board, state.piece, Movement::Down)
            .ok_or(TimingStepError::CollidingPiece)?;
        rows = rows.saturating_add(1);
    }
    let next_phase = phase % u64::from(FALL_MICROS_PER_CELL);
    state.fall_fraction_micros = if next_phase == 0 {
        1
    } else {
        next_phase as u32
    };
    Ok((rows, false))
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
    state.lock_resets_used += 1;
    if state.lock_resets_used < rules.max_lock_resets {
        state.lock_elapsed_frames = 0;
    }
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
    use super::{
        FALL_MICROS_PER_CELL, FrameInput, Gravity, LastAction, LinearGravityTiming,
        TETRIO_KICK_FALL_FRACTION_MICROS, TimingRules, TimingSchedule, TimingState, step_frame,
    };
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
    fn fixed_point_gravity_accumulates_without_floats() {
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
    fn observed_spawn_phase_crosses_after_two_point_zero_two_g_frames() {
        let board = Board::empty();
        let spawn = PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18);
        let mut state = TimingState::new_with_fall_fraction(&board, spawn, 960_000)
            .expect("observed spawn phase is valid");
        let timing = rules(1, 50, 30, 15);

        let first = step_frame(&board, &mut state, timing, &[]).unwrap();
        let second = step_frame(&board, &mut state, timing, &[]).unwrap();

        assert_eq!(first.piece.y, 18);
        assert_eq!(first.gravity_rows, 0);
        assert_eq!(state.fall_fraction_micros, 1);
        assert_eq!(second.piece.y, 17);
        assert_eq!(second.gravity_rows, 1);
    }

    #[test]
    fn fallback_kick_quantizes_fall_phase_to_point_one() {
        let board = Board::empty();
        let piece = PieceState::new(PieceKind::T, Orientation::Right, -1, 4);
        let mut state = TimingState::new_with_fall_fraction(&board, piece, 777_777).unwrap();

        let rotated = step_frame(
            &board,
            &mut state,
            rules(0, 1, 30, 15),
            &[FrameInput::RotateCounterclockwise],
        )
        .unwrap();

        assert_eq!(rotated.piece.x, 0);
        assert_eq!(state.fall_fraction_micros, TETRIO_KICK_FALL_FRACTION_MICROS);
        assert_eq!(
            rotated.last_action,
            LastAction::Rotation {
                direction: RotationDirection::Counterclockwise,
                kick_index: 0,
            }
        );
    }

    #[test]
    fn linear_schedule_keeps_one_denominator_and_caps_exactly() {
        let base = rules(1, 50, 30, 15);
        let schedule = TimingSchedule::linear_gravity(
            LinearGravityTiming::new(
                base,
                Gravity::new(7, 2_000).unwrap(),
                7_200,
                60,
                Gravity::new(20, 1).unwrap(),
            )
            .unwrap(),
        );

        let margin = schedule.rules_at_frame(7_200).unwrap().gravity;
        let after_update_frame = schedule.rules_at_frame(7_201).unwrap().gravity;
        let next = schedule.rules_at_frame(7_202).unwrap().gravity;
        let capped = schedule.rules_at_frame(1_000_000).unwrap().gravity;
        assert_eq!(margin, after_update_frame);
        assert_eq!(margin.denominator(), next.denominator());
        assert_eq!(margin, Gravity::new(2_400, 120_000).unwrap());
        assert_eq!(next, Gravity::new(2_407, 120_000).unwrap());
        assert_eq!(capped, Gravity::new(2_400_000, 120_000).unwrap());
    }

    #[test]
    fn rational_schedule_matches_client_float_after_micro_quantization() {
        let schedule = LinearGravityTiming::new(
            rules(1, 50, 30, 15),
            Gravity::new(7, 2_000).unwrap(),
            7_200,
            60,
            Gravity::new(20, 1).unwrap(),
        )
        .unwrap();
        let mut client_g = 0.02_f64;

        for frame in 0..=350_000_u64 {
            let gravity = schedule.rules_at_frame(frame).unwrap().gravity;
            let engine_micros = (u64::from(gravity.numerator()) * u64::from(FALL_MICROS_PER_CELL)
                + u64::from(gravity.denominator()) / 2)
                / u64::from(gravity.denominator());
            let client_micros = (client_g.min(20.0) * f64::from(FALL_MICROS_PER_CELL)).round();
            assert_eq!(engine_micros as f64, client_micros, "frame {frame}");

            if frame > 7_200 {
                client_g += 0.0035_f64 / 60.0;
            }
        }
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
            step_frame(&board, &mut state, rules(20, 1, 30, 15), &[]).expect("frame advances");

        assert_eq!(result.piece.y, -1);
        assert_eq!(result.gravity_rows, 19);
        assert!(result.grounded);
        assert!(!result.locked);
    }

    #[test]
    fn grounded_piece_locks_on_the_configured_frame() {
        let board = Board::empty();
        let mut state = TimingState::new(&board, grounded_o()).expect("piece is valid");
        let timing = rules(1, 50, 3, 15);

        assert!(!step_frame(&board, &mut state, timing, &[]).unwrap().locked);
        assert!(!step_frame(&board, &mut state, timing, &[]).unwrap().locked);
        assert!(!step_frame(&board, &mut state, timing, &[]).unwrap().locked);
        let fourth = step_frame(&board, &mut state, timing, &[]).unwrap();

        assert!(fourth.locked);
        assert_eq!(fourth.lock_elapsed_frames, 4);
    }

    #[test]
    fn grounded_lateral_move_resets_lock_once() {
        let board = Board::empty();
        let mut state = TimingState::new(&board, grounded_o()).expect("piece is valid");
        let timing = rules(1, 50, 3, 2);

        step_frame(&board, &mut state, timing, &[]).unwrap();
        let moved = step_frame(&board, &mut state, timing, &[FrameInput::MoveLeft]).unwrap();
        assert_eq!(moved.lock_elapsed_frames, 1);
        assert_eq!(moved.lock_resets_used, 1);

        let capped = step_frame(&board, &mut state, timing, &[FrameInput::MoveRight]).unwrap();
        assert_eq!(capped.lock_elapsed_frames, 2);
        assert_eq!(capped.lock_resets_used, 2);
        assert!(capped.locked);
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
