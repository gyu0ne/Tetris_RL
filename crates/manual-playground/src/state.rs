use engine_core::{
    FrameSession, GameConfig, InputButton, InputEdge, LastAction, PieceKind, RotationDirection,
    SessionFrameOutcome, SoftDropMode, SpinClassification, TimingState, TopOutReason,
    VISIBLE_HEIGHT,
};
use rules_tetrio::{ActiveTimingProfile, PlayerHandlingProfile, TetrioRulesDraft};
use serde::{Deserialize, Serialize};
use std::fmt;

const DEFAULT_SEED: u64 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepRequest {
    #[serde(default)]
    pub edges: Vec<WireInputEdge>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetRequest {
    #[serde(default = "default_seed")]
    pub seed: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireInputEdge {
    pub button: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaygroundView {
    pub frame: u64,
    pub board_rows: Vec<u16>,
    pub garbage_rows: Vec<u16>,
    pub active: Option<ActivePieceView>,
    pub hold: Option<&'static str>,
    pub preview: Vec<&'static str>,
    pub pieces_placed: u64,
    pub top_out: Option<&'static str>,
    pub timing: Option<TimingView>,
    pub last_event: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActivePieceView {
    pub kind: &'static str,
    pub cells: [[i16; 2]; 4],
}

#[derive(Clone, Debug, Serialize)]
pub struct TimingView {
    pub fall_fraction_micros: u32,
    pub lock_elapsed_frames: u16,
    pub lock_resets_used: u16,
    pub locked: bool,
    pub last_action: String,
}

#[derive(Debug)]
pub enum PlaygroundError {
    Profile(String),
    Engine(String),
    Input(String),
}

impl fmt::Display for PlaygroundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(message) => write!(formatter, "profile error: {message}"),
            Self::Engine(message) => write!(formatter, "engine error: {message}"),
            Self::Input(message) => write!(formatter, "input error: {message}"),
        }
    }
}

impl std::error::Error for PlaygroundError {}

pub struct PlaygroundState {
    session: FrameSession,
    timing: ActiveTimingProfile,
    handling: engine_core::HandlingRules,
    last_event: String,
}

impl PlaygroundState {
    pub fn new(seed: u64) -> Result<Self, PlaygroundError> {
        let draft = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let timing = draft
            .try_timing_profile()
            .map_err(|error| PlaygroundError::Profile(format!("{error:?}")))?;
        // Manual-test handling, not a TETRA LEAGUE room constant. The mode
        // supplies mechanics while the player supplies effective handling.
        let handling = PlayerHandlingProfile::normalized(10, 2, 2, SoftDropMode::CellsPerFrame(41))
            .core_rules();
        let session = FrameSession::new(seed, GameConfig::default())
            .map_err(|error| PlaygroundError::Engine(error.to_string()))?;
        Ok(Self {
            session,
            timing,
            handling,
            last_event: "준비 완료".to_owned(),
        })
    }

    pub fn reset(&mut self, seed: u64) -> Result<PlaygroundView, PlaygroundError> {
        *self = Self::new(seed)?;
        Ok(self.view())
    }

    pub fn step(&mut self, request: StepRequest) -> Result<PlaygroundView, PlaygroundError> {
        if self.session.is_terminal() {
            return Ok(self.view());
        }
        let edges = request
            .edges
            .into_iter()
            .map(convert_edge)
            .collect::<Result<Vec<_>, _>>()?;
        let rules = self
            .timing
            .core_rules_at_frame(self.session.frame())
            .map_err(|error| PlaygroundError::Profile(format!("{error:?}")))?;
        let outcome = self
            .session
            .step(rules, self.handling, &edges)
            .map_err(|error| PlaygroundError::Engine(error.to_string()))?;
        if let Some(event) = describe_outcome(&outcome) {
            self.last_event = event;
        }
        Ok(self.view())
    }

    pub fn view(&self) -> PlaygroundView {
        let game = self.session.game();
        let board = game.board();
        let active = self.session.timing().map(|timing| {
            let cells = timing.piece.cells().map(|(x, y)| [x, y]);
            ActivePieceView {
                kind: piece_name(timing.piece.kind),
                cells,
            }
        });
        PlaygroundView {
            frame: self.session.frame(),
            board_rows: board.rows()[..VISIBLE_HEIGHT].to_vec(),
            garbage_rows: board.garbage_rows()[..VISIBLE_HEIGHT].to_vec(),
            active,
            hold: game.hold().map(piece_name),
            preview: game.preview().into_iter().map(piece_name).collect(),
            pieces_placed: game.pieces_placed(),
            top_out: game.top_out_reason().map(top_out_name),
            timing: self.session.timing().map(timing_view),
            last_event: self.last_event.clone(),
        }
    }
}

impl Default for PlaygroundState {
    fn default() -> Self {
        Self::new(DEFAULT_SEED).expect("observed solo profile must activate")
    }
}

fn convert_edge(edge: WireInputEdge) -> Result<InputEdge, PlaygroundError> {
    let button = match edge.button.as_str() {
        "left" => InputButton::Left,
        "right" => InputButton::Right,
        "soft_drop" => InputButton::SoftDrop,
        "hard_drop" => InputButton::HardDrop,
        "rotate_clockwise" => InputButton::RotateClockwise,
        "rotate_counterclockwise" => InputButton::RotateCounterclockwise,
        "rotate_half" => InputButton::RotateHalf,
        "hold" => InputButton::Hold,
        _ => return Err(PlaygroundError::Input("unknown button".to_owned())),
    };
    match edge.kind.as_str() {
        "press" => Ok(InputEdge::press(button)),
        "release" => Ok(InputEdge::release(button)),
        _ => Err(PlaygroundError::Input("unknown edge kind".to_owned())),
    }
}

fn describe_outcome(outcome: &SessionFrameOutcome) -> Option<String> {
    let Some(placement) = outcome.placement else {
        if outcome.hold_applied {
            return Some("HOLD".to_owned());
        }
        return None;
    };
    let clear = placement.clear;
    let mut parts = vec![format!("{} 고정", piece_name(clear.piece))];
    if clear.lines > 0 {
        parts.push(format!("{}줄 제거", clear.lines));
    }
    if let Some(spin) = clear.spin {
        let classification = match spin.classification {
            SpinClassification::Mini => "MINI",
            SpinClassification::Full => "FULL",
        };
        parts.push(format!("{classification} SPIN"));
    }
    if clear.perfect_clear {
        parts.push("PERFECT CLEAR".to_owned());
    }
    if placement.clutch {
        parts.push("CLUTCH".to_owned());
    }
    if let Some(reason) = placement.top_out_reason {
        parts.push(format!("TOP OUT: {}", top_out_name(reason)));
    }
    Some(parts.join(" · "))
}

fn timing_view(timing: &TimingState) -> TimingView {
    TimingView {
        fall_fraction_micros: timing.fall_fraction_micros,
        lock_elapsed_frames: timing.lock_elapsed_frames,
        lock_resets_used: timing.lock_resets_used,
        locked: timing.locked,
        last_action: last_action_name(timing.last_action),
    }
}

fn last_action_name(action: LastAction) -> String {
    match action {
        LastAction::None => "none".to_owned(),
        LastAction::Translation => "translation".to_owned(),
        LastAction::SoftDrop => "soft_drop".to_owned(),
        LastAction::HardDrop => "hard_drop".to_owned(),
        LastAction::Rotation {
            direction,
            kick_index,
        } => format!("rotation:{}:kick{kick_index}", rotation_name(direction)),
    }
}

const fn piece_name(piece: PieceKind) -> &'static str {
    match piece {
        PieceKind::I => "I",
        PieceKind::J => "J",
        PieceKind::L => "L",
        PieceKind::O => "O",
        PieceKind::S => "S",
        PieceKind::T => "T",
        PieceKind::Z => "Z",
    }
}

const fn rotation_name(direction: RotationDirection) -> &'static str {
    match direction {
        RotationDirection::Clockwise => "cw",
        RotationDirection::Counterclockwise => "ccw",
        RotationDirection::Half => "180",
    }
}

const fn top_out_name(reason: TopOutReason) -> &'static str {
    match reason {
        TopOutReason::BlockOut => "block_out",
        TopOutReason::LockOut => "lock_out",
        TopOutReason::PartialLockOut => "partial_lock_out",
        TopOutReason::GarbageOut => "garbage_out",
    }
}

const fn default_seed() -> u64 {
    DEFAULT_SEED
}

#[cfg(test)]
mod tests {
    use super::{PlaygroundState, StepRequest, WireInputEdge};

    #[test]
    fn hard_drop_advances_the_authoritative_session() {
        let mut state = PlaygroundState::new(7).expect("valid playground");
        let view = state
            .step(StepRequest {
                edges: vec![WireInputEdge {
                    button: "hard_drop".to_owned(),
                    kind: "press".to_owned(),
                }],
            })
            .expect("hard drop");

        assert_eq!(view.frame, 1);
        assert_eq!(view.pieces_placed, 1);
        assert!(view.last_event.contains("고정"));
    }

    #[test]
    fn view_exposes_only_the_twenty_visible_rows() {
        let state = PlaygroundState::new(11).expect("valid playground");
        let view = state.view();
        assert_eq!(view.board_rows.len(), 20);
        assert_eq!(view.garbage_rows.len(), 20);
        assert!(view.active.is_some());
    }

    #[test]
    fn invalid_wire_input_is_rejected() {
        let mut state = PlaygroundState::new(13).expect("valid playground");
        assert!(
            state
                .step(StepRequest {
                    edges: vec![WireInputEdge {
                        button: "teleport".to_owned(),
                        kind: "press".to_owned(),
                    }],
                })
                .is_err()
        );
    }
}
