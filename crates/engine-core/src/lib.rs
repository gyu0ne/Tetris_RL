//! Deterministic mechanics kernel for the local falling-block engine.
//!
//! This crate deliberately contains no rendering, networking, wall clock, or
//! learning code. TETR.IO-specific values are layered above this crate and are
//! not considered confirmed until a versioned differential fixture covers them.

#![forbid(unsafe_code)]

mod board;
mod clear;
mod game;
mod handling;
mod piece;
mod reachability;
mod rng;
mod rotation;
mod session;
mod spin;
mod timing;
mod topout;

pub use board::{
    Board, BoardError, ClearedLines, HEIGHT, LockResult, LockVisibility, VISIBLE_HEIGHT, WIDTH,
};
pub use clear::ClearEvent;
pub use game::{
    GameConfig, GameError, GameState, HoldOutcome, InitialActionOutcome, PlacementOutcome,
    SpawnRules,
};
pub use handling::{
    HandlingRules, HandlingState, InitialActions, InputButton, InputEdge, InputEdgeKind,
    NormalizedFrame, SoftDropMode, initial_actions_on_spawn, normalize_frame, on_piece_spawn,
};
pub use piece::{Orientation, PieceKind, PieceState};
pub use reachability::{GeometricPlacement, Movement, hard_drop, reachable_locks, try_movement};
pub use rng::{BagOrderError, MinStd, SevenBag};
pub use rotation::{RotationDirection, RotationResult, kick_tests, try_rotate};
pub use session::{FrameSession, SessionFrameOutcome, SessionStepError};
pub use spin::{SpinClassification, SpinMode, SpinOutcome, SpinRules, classify_spin};
pub use timing::{
    FrameInput, FrameOutcome, Gravity, LastAction, TimingConfigError, TimingRules, TimingState,
    TimingStepError, step_frame,
};
pub use topout::{TopOutReason, TopOutRules};
