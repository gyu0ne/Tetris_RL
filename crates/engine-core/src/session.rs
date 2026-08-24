use crate::{
    Board, FrameOutcome, GameConfig, GameError, GameState, HandlingRules, HandlingState,
    InitialActionOutcome, InputEdge, LastAction, NormalizedFrame, PlacementOutcome, TimingRules,
    TimingState, TimingStepError, initial_actions_on_spawn, normalize_frame, step_frame,
};
use std::fmt;

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
            Some(TimingState::new(game.board(), game.active())?)
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
        self.timing.is_none()
    }

    pub fn step(
        &mut self,
        timing_rules: TimingRules,
        handling_rules: HandlingRules,
        edges: &[InputEdge],
    ) -> Result<SessionFrameOutcome, SessionStepError> {
        if self.is_terminal() {
            return Err(SessionStepError::Game(GameError::GameOver));
        }

        let frame = self.frame;
        let normalized = normalize_frame(&mut self.handling, handling_rules, edges);
        let mut hold_applied = false;

        // Generic stage order: an immediate hold request replaces the active
        // piece before this frame's normalized piece actions are consumed.
        if normalized.hold_requested && self.game.hold_available() {
            let hold = self.game.hold_active()?;
            hold_applied = true;
            self.timing = if hold.top_out {
                None
            } else {
                Some(TimingState::new(self.game.board(), hold.active)?)
            };
            if self.is_terminal() {
                self.frame = self.frame.saturating_add(1);
                return Ok(SessionFrameOutcome {
                    frame,
                    normalized,
                    timing: None,
                    placement: None,
                    hold_applied,
                    spawned_initial: None,
                    terminal: true,
                });
            }
        }

        let timing = self
            .timing
            .as_mut()
            .ok_or(SessionStepError::Game(GameError::GameOver))?;
        let timing_outcome =
            step_frame(self.game.board(), timing, timing_rules, &normalized.actions)?;

        let mut placement = None;
        let mut spawned_initial = None;
        if timing_outcome.locked {
            let locked = self
                .game
                .lock_placement_with_action(timing_outcome.piece, timing_outcome.last_action)?;
            placement = Some(locked);
            self.timing = None;

            if !locked.top_out {
                let initial = initial_actions_on_spawn(&mut self.handling, handling_rules);
                let initial_outcome = self.game.apply_initial_actions(initial)?;
                spawned_initial = Some(initial_outcome);
                if !initial_outcome.top_out {
                    let last_action =
                        initial_outcome
                            .rotation
                            .map_or(LastAction::None, |rotation| LastAction::Rotation {
                                direction: rotation.direction,
                                kick_index: rotation.kick_index,
                            });
                    self.timing = Some(TimingState::with_last_action(
                        self.game.board(),
                        initial_outcome.active,
                        last_action,
                    )?);
                }
            }
        }

        self.frame = self.frame.saturating_add(1);
        Ok(SessionFrameOutcome {
            frame,
            normalized,
            timing: Some(timing_outcome),
            placement,
            hold_applied,
            spawned_initial,
            terminal: self.is_terminal(),
        })
    }
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
        GameConfig, Gravity, HandlingRules, InputButton, InputEdge, SoftDropMode, TimingRules,
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
