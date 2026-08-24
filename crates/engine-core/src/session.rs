use crate::{
    Board, FrameOutcome, GameConfig, GameError, GameState, GarbagePushResult, HandlingRules,
    HandlingState, InitialActionOutcome, InputEdge, LastAction, LockedPlacement, NormalizedFrame,
    PlacementOutcome, TETRIO_KICK_FALL_FRACTION_MICROS, TimingRules, TimingState, TimingStepError,
    initial_actions_on_spawn, normalize_frame, step_frame,
};
use std::fmt;

use crate::timing::{advance_after_inputs, apply_input_phase, input_phase_outcome};

/// Continuous generic game session joining input handling, frame timing, lock,
/// queue/hold, and next-piece spawn transitions.
///
/// The session is deterministic but not itself a TETR.IO conformance claim.
/// A versioned adapter still determines exact same-frame command ordering and
/// supplies the effective rules for each frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameSession {
    game: GameState,
    timing: Option<TimingState>,
    handling: HandlingState,
    frame: u64,
}

impl FrameSession {
    pub fn new(seed: u64, config: GameConfig) -> Result<Self, SessionStepError> {
        Self::with_board(seed, config, Board::empty())
    }

    pub fn with_board(
        seed: u64,
        config: GameConfig,
        board: Board,
    ) -> Result<Self, SessionStepError> {
        let game = GameState::with_board(seed, config, board)?;
        let timing = if game.is_top_out() {
            None
        } else {
            Some(TimingState::new_with_fall_fraction(
                game.board(),
                game.active(),
                game.spawn_fall_fraction_micros(),
            )?)
        };
        Ok(Self {
            game,
            timing,
            handling: HandlingState::new(),
            frame: 0,
        })
    }

    pub const fn game(&self) -> &GameState {
        &self.game
    }

    pub const fn timing(&self) -> Option<&TimingState> {
        self.timing.as_ref()
    }

    pub const fn handling(&self) -> &HandlingState {
        &self.handling
    }

    pub const fn frame(&self) -> u64 {
        self.frame
    }

    pub const fn is_terminal(&self) -> bool {
        self.game.is_top_out()
    }

    pub const fn is_awaiting_spawn(&self) -> bool {
        self.game.pending_lock().is_some()
    }

    /// Advances one input frame through piece lock, but deliberately stops
    /// before consuming/spawning the next piece. Versus orchestration resolves
    /// attacks, cancellation, and garbage at this boundary.
    pub fn step_until_lock(
        &mut self,
        timing_rules: TimingRules,
        handling_rules: HandlingRules,
        edges: &[InputEdge],
    ) -> Result<SessionLockFrameOutcome, SessionStepError> {
        if self.is_terminal() {
            return Err(SessionStepError::Game(GameError::GameOver));
        }
        if self.is_awaiting_spawn() {
            return Err(SessionStepError::Game(GameError::AwaitingSpawn));
        }

        let frame = self.frame;
        let normalized = normalize_frame(&mut self.handling, handling_rules, edges);
        let mut hold_applied = false;

        let timing_outcome = if let Some(hold_index) = normalized.hold_action_index {
            let prefix = {
                let timing = self
                    .timing
                    .as_mut()
                    .ok_or(SessionStepError::Game(GameError::GameOver))?;
                apply_input_phase(
                    self.game.board(),
                    timing,
                    timing_rules,
                    &normalized.actions[..hold_index],
                )?
            };

            if prefix.locked {
                input_phase_outcome(
                    self.game.board(),
                    *self.timing.as_ref().expect("locked phase retains timing"),
                    prefix.successful_inputs,
                )
            } else {
                if self.game.hold_available() {
                    let hold = self.game.hold_active()?;
                    hold_applied = true;
                    self.timing = if hold.top_out {
                        None
                    } else {
                        Some(TimingState::new_with_fall_fraction(
                            self.game.board(),
                            hold.active,
                            self.game.spawn_fall_fraction_micros(),
                        )?)
                    };
                    if self.is_terminal() {
                        self.frame = self.frame.saturating_add(1);
                        return Ok(SessionLockFrameOutcome {
                            frame,
                            normalized,
                            timing: None,
                            locked: None,
                            hold_applied,
                            terminal: true,
                        });
                    }
                }

                let timing = self
                    .timing
                    .as_mut()
                    .ok_or(SessionStepError::Game(GameError::GameOver))?;
                let suffix = apply_input_phase(
                    self.game.board(),
                    timing,
                    timing_rules,
                    &normalized.actions[hold_index..],
                )?;
                let successful_inputs = prefix
                    .successful_inputs
                    .saturating_add(suffix.successful_inputs);
                if suffix.locked {
                    input_phase_outcome(self.game.board(), *timing, successful_inputs)
                } else {
                    advance_after_inputs(
                        self.game.board(),
                        timing,
                        timing_rules,
                        successful_inputs,
                    )?
                }
            }
        } else {
            let timing = self
                .timing
                .as_mut()
                .ok_or(SessionStepError::Game(GameError::GameOver))?;
            step_frame(self.game.board(), timing, timing_rules, &normalized.actions)?
        };

        let locked = if timing_outcome.locked {
            let locked = self
                .game
                .lock_placement_deferred(timing_outcome.piece, timing_outcome.last_action)?;
            self.timing = None;
            Some(locked)
        } else {
            None
        };

        self.frame = self.frame.saturating_add(1);
        Ok(SessionLockFrameOutcome {
            frame,
            normalized,
            timing: Some(timing_outcome),
            locked,
            hold_applied,
            terminal: self.is_terminal(),
        })
    }

    pub fn push_garbage_before_spawn(
        &mut self,
        hole_column: usize,
    ) -> Result<GarbagePushResult, SessionStepError> {
        self.game
            .push_garbage_before_spawn(hole_column)
            .map_err(Into::into)
    }

    /// Completes a lock after versus processing and applies IHS/IRS to the new
    /// spawn. This method does not advance the input frame.
    pub fn finish_pending_spawn(
        &mut self,
        handling_rules: HandlingRules,
    ) -> Result<SessionSpawnOutcome, SessionStepError> {
        let mut placement = self.game.finish_lock()?;
        let mut spawned_initial = None;

        if !placement.top_out {
            let initial = initial_actions_on_spawn(&mut self.handling, handling_rules);
            let initial_outcome = self
                .game
                .apply_initial_actions_with_clutch(initial, placement.cleared.count() > 0)?;
            placement.clutch |= initial_outcome.clutch;
            spawned_initial = Some(initial_outcome);
            if !initial_outcome.top_out {
                let last_action = initial_outcome
                    .rotation
                    .map_or(LastAction::None, |rotation| LastAction::Rotation {
                        direction: rotation.direction,
                        kick_index: rotation.kick_index,
                    });
                let fall_fraction_micros = initial_outcome.rotation.map_or(
                    self.game.spawn_fall_fraction_micros(),
                    |rotation| {
                        if rotation.used_kick {
                            TETRIO_KICK_FALL_FRACTION_MICROS
                        } else {
                            self.game.spawn_fall_fraction_micros()
                        }
                    },
                );
                self.timing = Some(TimingState::with_last_action_and_fall_fraction(
                    self.game.board(),
                    initial_outcome.active,
                    last_action,
                    fall_fraction_micros,
                )?);
            }
        }

        Ok(SessionSpawnOutcome {
            placement,
            spawned_initial,
            terminal: self.is_terminal(),
        })
    }

    /// Compatibility one-frame API for solo play. Versus callers should use
    /// `step_until_lock` and `finish_pending_spawn` around garbage processing.
    pub fn step(
        &mut self,
        timing_rules: TimingRules,
        handling_rules: HandlingRules,
        edges: &[InputEdge],
    ) -> Result<SessionFrameOutcome, SessionStepError> {
        let lock = self.step_until_lock(timing_rules, handling_rules, edges)?;
        let spawn = if lock.locked.is_some() {
            Some(self.finish_pending_spawn(handling_rules)?)
        } else {
            None
        };

        Ok(SessionFrameOutcome {
            frame: lock.frame,
            normalized: lock.normalized,
            timing: lock.timing,
            placement: spawn.as_ref().map(|outcome| outcome.placement),
            hold_applied: lock.hold_applied,
            spawned_initial: spawn.and_then(|outcome| outcome.spawned_initial),
            terminal: self.is_terminal(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionLockFrameOutcome {
    pub frame: u64,
    pub normalized: NormalizedFrame,
    pub timing: Option<FrameOutcome>,
    pub locked: Option<LockedPlacement>,
    pub hold_applied: bool,
    pub terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionSpawnOutcome {
    pub placement: PlacementOutcome,
    pub spawned_initial: Option<InitialActionOutcome>,
    pub terminal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionFrameOutcome {
    pub frame: u64,
    pub normalized: NormalizedFrame,
    pub timing: Option<FrameOutcome>,
    pub placement: Option<PlacementOutcome>,
    pub hold_applied: bool,
    pub spawned_initial: Option<InitialActionOutcome>,
    pub terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStepError {
    Game(GameError),
    Timing(TimingStepError),
}

impl From<GameError> for SessionStepError {
    fn from(error: GameError) -> Self {
        Self::Game(error)
    }
}

impl From<TimingStepError> for SessionStepError {
    fn from(error: TimingStepError) -> Self {
        Self::Timing(error)
    }
}

impl fmt::Display for SessionStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Game(error) => error.fmt(formatter),
            Self::Timing(error) => write!(formatter, "timing step failed: {error:?}"),
        }
    }
}

impl std::error::Error for SessionStepError {}

#[cfg(test)]
mod tests {
    use super::FrameSession;
    use crate::{
        GameConfig, GameError, Gravity, HandlingRules, InputButton, InputEdge, Orientation,
        SessionStepError, SoftDropMode, TimingRules, WIDTH,
    };

    fn timing_rules() -> TimingRules {
        TimingRules::new(Gravity::new(0, 1).unwrap(), 30, 15, true, true)
    }

    fn handling_rules() -> HandlingRules {
        HandlingRules::new(10, 2, 0, SoftDropMode::Disabled)
    }

    #[test]
    fn hard_drop_locks_and_spawns_the_next_piece_in_one_session_step() {
        let mut session = FrameSession::new(71, GameConfig::default()).expect("valid session");
        let first = session.game().active().kind;
        let outcome = session
            .step(
                timing_rules(),
                handling_rules(),
                &[InputEdge::press(InputButton::HardDrop)],
            )
            .expect("hard drop step");

        assert!(outcome.timing.expect("timing outcome").locked);
        assert!(outcome.placement.is_some());
        assert_eq!(session.game().pieces_placed(), 1);
        assert_ne!(session.game().active().kind, first);
        assert_eq!(session.frame(), 1);
        assert!(!session.is_terminal());
    }

    #[test]
    fn immediate_hold_replaces_the_piece_before_timing_actions() {
        let mut session = FrameSession::new(73, GameConfig::default()).expect("valid session");
        let outgoing = session.game().active().kind;
        let incoming = session.game().preview()[0];
        let outcome = session
            .step(
                timing_rules(),
                handling_rules(),
                &[InputEdge::press(InputButton::Hold)],
            )
            .expect("hold step");

        assert!(outcome.hold_applied);
        assert!(outcome.placement.is_none());
        assert_eq!(session.game().hold(), Some(outgoing));
        assert_eq!(
            session.timing().expect("active timing").piece.kind,
            incoming
        );
        assert_eq!(session.game().pieces_placed(), 0);
    }

    #[test]
    fn same_frame_hold_keeps_client_event_order() {
        let mut rotate_then_hold =
            FrameSession::new(79, GameConfig::default()).expect("valid session");
        rotate_then_hold
            .step(
                timing_rules(),
                handling_rules(),
                &[
                    InputEdge::press(InputButton::RotateClockwise),
                    InputEdge::press(InputButton::Hold),
                ],
            )
            .unwrap();
        assert_eq!(
            rotate_then_hold.timing().unwrap().piece.orientation,
            Orientation::Spawn
        );

        let mut hold_then_rotate =
            FrameSession::new(79, GameConfig::default()).expect("valid session");
        hold_then_rotate
            .step(
                timing_rules(),
                handling_rules(),
                &[
                    InputEdge::press(InputButton::Hold),
                    InputEdge::press(InputButton::RotateClockwise),
                ],
            )
            .unwrap();
        assert_eq!(
            hold_then_rotate.timing().unwrap().piece.orientation,
            Orientation::Right
        );
    }

    #[test]
    fn deferred_lock_preserves_the_next_piece_until_spawn_is_finished() {
        let mut session = FrameSession::new(77, GameConfig::default()).expect("valid session");
        let locked_kind = session.game().active().kind;
        let queued_next = session.game().preview()[0];
        let hard_drop = [InputEdge::press(InputButton::HardDrop)];

        let lock = session
            .step_until_lock(timing_rules(), handling_rules(), &hard_drop)
            .expect("deferred lock");

        assert!(lock.locked.is_some());
        assert!(session.is_awaiting_spawn());
        assert!(!session.is_terminal());
        assert_eq!(session.game().active().kind, locked_kind);
        assert_eq!(session.game().preview()[0], queued_next);
        assert_eq!(
            session
                .step_until_lock(timing_rules(), handling_rules(), &[])
                .expect_err("spawn boundary must be resolved first"),
            SessionStepError::Game(GameError::AwaitingSpawn)
        );

        let spawn = session
            .finish_pending_spawn(handling_rules())
            .expect("finish spawn");
        assert!(!spawn.terminal);
        assert!(!session.is_awaiting_spawn());
        assert_eq!(session.game().active().kind, queued_next);
    }

    #[test]
    fn garbage_can_be_inserted_between_lock_and_spawn() {
        let mut session = FrameSession::new(83, GameConfig::default()).expect("valid session");
        session
            .step_until_lock(
                timing_rules(),
                handling_rules(),
                &[InputEdge::press(InputButton::HardDrop)],
            )
            .expect("deferred lock");

        session
            .push_garbage_before_spawn(4)
            .expect("garbage insertion");
        session
            .finish_pending_spawn(handling_rules())
            .expect("finish spawn");

        for x in 0..WIDTH {
            assert_eq!(session.game().board().is_garbage(x, 0), Some(x != 4));
        }
    }

    #[test]
    fn identical_multi_piece_sessions_are_deterministic() {
        let mut first = FrameSession::new(79, GameConfig::default()).expect("valid session");
        let mut second = first.clone();
        let hard_drop = [InputEdge::press(InputButton::HardDrop)];

        for _ in 0..4 {
            let first_outcome = first
                .step(timing_rules(), handling_rules(), &hard_drop)
                .expect("first session step");
            let second_outcome = second
                .step(timing_rules(), handling_rules(), &hard_drop)
                .expect("second session step");
            assert_eq!(first_outcome, second_outcome);
            assert_eq!(first, second);
        }
        assert_eq!(first.game().pieces_placed(), 4);
    }
}
