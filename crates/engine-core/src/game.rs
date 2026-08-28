use crate::{
    BagOrderError, Board, BoardError, ClearEvent, ClearedLines, GarbagePushResult,
    GeometricPlacement, HEIGHT, InitialActions, LastAction, LockVisibility, Orientation, PieceKind,
    PieceState, RotationResult, SevenBag, SpinRules, TopOutReason, TopOutRules, VISIBLE_HEIGHT,
    classify_spin, reachable_locks, try_rotate,
};
use std::collections::VecDeque;
use std::fmt;

/// Observed current-client pre-shuffle order (`z,l,o,s,i,j,t`).
pub const TETRIO_7_BAG_ORDER: [PieceKind; 7] = [
    PieceKind::Z,
    PieceKind::L,
    PieceKind::O,
    PieceKind::S,
    PieceKind::I,
    PieceKind::J,
    PieceKind::T,
];

/// Current client spawn y is `board_buffer - 2.04`; the integer board view
/// therefore starts with a 0.96-cell downward phase.
pub const TETRIO_SPAWN_FALL_FRACTION_MICROS: u32 = 960_000;

/// Profile-supplied spawn origins and orientations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpawnRules {
    origins: [(i16, i16); 7],
    orientations: [Orientation; 7],
}

impl SpawnRules {
    pub const fn new(origins: [(i16, i16); 7], orientations: [Orientation; 7]) -> Self {
        Self {
            origins,
            orientations,
        }
    }

    /// Observed modern spawn candidate. A current TETR.IO fixture must confirm
    /// this before a target conformance profile may mark it `CONFIRMED`.
    pub const fn modern_observed() -> Self {
        Self::new([(3, 18); 7], [Orientation::Spawn; 7])
    }

    pub const fn piece(self, kind: PieceKind) -> PieceState {
        let (x, y) = self.origins[kind.index()];
        PieceState::new(kind, self.orientations[kind.index()], x, y)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameConfig {
    pub preview: usize,
    pub spawn: SpawnRules,
    pub bag_order: [PieceKind; 7],
    pub spin: SpinRules,
    pub top_out: TopOutRules,
    pub clutch_clear: bool,
    pub spawn_fall_fraction_micros: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            preview: 5,
            spawn: SpawnRules::modern_observed(),
            bag_order: TETRIO_7_BAG_ORDER,
            spin: SpinRules::all_mini_plus_observed(),
            top_out: TopOutRules::block_out_only(),
            clutch_clear: true,
            spawn_fall_fraction_micros: TETRIO_SPAWN_FALL_FRACTION_MICROS,
        }
    }
}

/// Minimal deterministic single-player state used to validate queue, hold and
/// placement transitions before timing and versus layers are added.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameState {
    board: Board,
    bag: SevenBag,
    queue: VecDeque<PieceKind>,
    active: PieceState,
    hold: Option<PieceKind>,
    hold_available: bool,
    pieces_placed: u64,
    top_out: Option<TopOutReason>,
    pending_lock: Option<LockedPlacement>,
    config: GameConfig,
}

impl GameState {
    pub fn new(seed: u64, config: GameConfig) -> Result<Self, GameError> {
        Self::with_board(seed, config, Board::empty())
    }

    pub fn with_board(seed: u64, config: GameConfig, board: Board) -> Result<Self, GameError> {
        let mut bag = SevenBag::with_order(seed, config.bag_order)?;
        let active = config.spawn.piece(bag.next_piece());
        let mut queue = VecDeque::with_capacity(config.preview);
        while queue.len() < config.preview {
            queue.push_back(bag.next_piece());
        }
        let top_out = board.collides(active).then_some(TopOutReason::BlockOut);

        Ok(Self {
            board,
            bag,
            queue,
            active,
            hold: None,
            hold_available: true,
            pieces_placed: 0,
            top_out,
            pending_lock: None,
            config,
        })
    }

    pub const fn board(&self) -> &Board {
        &self.board
    }

    pub const fn spawn_fall_fraction_micros(&self) -> u32 {
        self.config.spawn_fall_fraction_micros
    }

    pub const fn active(&self) -> PieceState {
        self.active
    }

    pub const fn hold(&self) -> Option<PieceKind> {
        self.hold
    }

    pub const fn hold_available(&self) -> bool {
        self.hold_available
    }

    pub const fn pieces_placed(&self) -> u64 {
        self.pieces_placed
    }

    pub const fn is_top_out(&self) -> bool {
        self.top_out.is_some()
    }

    pub const fn top_out_reason(&self) -> Option<TopOutReason> {
        self.top_out
    }

    pub const fn pending_lock(&self) -> Option<LockedPlacement> {
        self.pending_lock
    }

    pub fn preview(&self) -> Vec<PieceKind> {
        self.queue.iter().copied().collect()
    }

    pub fn reachable_placements(&self) -> Vec<crate::GeometricPlacement> {
        if self.is_top_out() || self.pending_lock.is_some() {
            Vec::new()
        } else {
            reachable_locks(&self.board, self.active)
        }
    }

    /// Computes the authoritative lock facts and compacted afterstate for a
    /// placement returned by [`Self::reachable_placements`] without running a
    /// second reachability search. The game itself is not mutated.
    pub fn preview_reachable_placement(
        &self,
        placement: &GeometricPlacement,
    ) -> Result<PlacementPreview, GameError> {
        self.ensure_playable()?;
        if placement.state.kind != self.active.kind {
            return Err(GameError::WrongPiece {
                active: self.active.kind,
                attempted: placement.state.kind,
            });
        }

        let spin = classify_spin(
            &self.board,
            placement.state,
            placement.last_action,
            self.config.spin,
        );
        let mut board = self.board;
        let lock = board.lock(placement.state)?;
        let locked = LockedPlacement {
            cleared: lock.cleared,
            clear: ClearEvent::from_lock(
                placement.state.kind,
                lock.cleared,
                spin,
                lock.perfect_clear,
            ),
            cleared_garbage: lock.cleared_garbage,
            lock_visibility: lock.visibility,
            pieces_placed: self.pieces_placed + 1,
        };
        Ok(PlacementPreview { board, locked })
    }

    pub fn hold_active(&mut self) -> Result<HoldOutcome, GameError> {
        self.ensure_playable()?;
        if !self.hold_available {
            return Err(GameError::HoldAlreadyUsed);
        }

        let outgoing = self.active.kind;
        let incoming = self.hold.unwrap_or_else(|| self.take_next_piece());
        self.hold = Some(outgoing);
        self.active = self.config.spawn.piece(incoming);
        self.hold_available = false;
        self.top_out = self
            .board
            .collides(self.active)
            .then_some(TopOutReason::BlockOut);

        Ok(HoldOutcome {
            held: outgoing,
            active: self.active,
            top_out: self.is_top_out(),
            top_out_reason: self.top_out,
        })
    }

    /// Applies the generic spawn contract in IHS-then-IRS order.
    ///
    /// Exact TETR.IO same-frame sampling order is intentionally left to the
    /// versioned replay-conformance layer.
    pub fn apply_initial_actions(
        &mut self,
        actions: InitialActions,
    ) -> Result<InitialActionOutcome, GameError> {
        self.apply_initial_actions_with_clutch(actions, false)
    }

    /// Applies initial hold/rotation after a lock. `clutch_available` lets an
    /// IHS replacement receive the same post-clear Clutch Clear rescue as the
    /// normally spawned queue piece.
    pub fn apply_initial_actions_with_clutch(
        &mut self,
        actions: InitialActions,
        clutch_available: bool,
    ) -> Result<InitialActionOutcome, GameError> {
        self.ensure_playable()?;
        let clutch_available = self.config.clutch_clear && clutch_available;

        let hold_applied = if actions.hold_requested && self.hold_available {
            self.hold_active()?;
            true
        } else {
            false
        };

        let clutch =
            hold_applied && self.top_out == Some(TopOutReason::BlockOut) && clutch_available && {
                self.top_out = None;
                self.raise_spawn_until_legal()
            };
        if hold_applied && self.board.collides(self.active) {
            self.top_out = Some(TopOutReason::BlockOut);
        }

        if self.is_top_out() {
            return Ok(InitialActionOutcome {
                active: self.active,
                hold_applied,
                rotation: None,
                clutch,
                top_out: true,
                top_out_reason: self.top_out,
            });
        }

        let rotation = actions
            .rotation
            .and_then(|direction| try_rotate(&self.board, self.active, direction));
        if let Some(result) = rotation {
            self.active = result.state;
        }
        self.top_out = self
            .board
            .collides(self.active)
            .then_some(TopOutReason::BlockOut);

        Ok(InitialActionOutcome {
            active: self.active,
            hold_applied,
            rotation,
            clutch,
            top_out: self.is_top_out(),
            top_out_reason: self.top_out,
        })
    }

    pub fn lock_placement(&mut self, placement: PieceState) -> Result<PlacementOutcome, GameError> {
        self.lock_placement_with_action(placement, LastAction::None)
    }

    pub fn lock_placement_with_action(
        &mut self,
        placement: PieceState,
        last_action: LastAction,
    ) -> Result<PlacementOutcome, GameError> {
        self.lock_placement_deferred(placement, last_action)?;
        self.finish_lock()
    }

    /// Locks and clears the active piece without consuming the next bag item.
    /// Versus code uses this boundary to resolve attack/cancel/garbage before
    /// the next spawn and its Clutch Clear block-out check.
    pub fn lock_placement_deferred(
        &mut self,
        placement: PieceState,
        last_action: LastAction,
    ) -> Result<LockedPlacement, GameError> {
        self.ensure_playable()?;
        if placement.kind != self.active.kind {
            return Err(GameError::WrongPiece {
                active: self.active.kind,
                attempted: placement.kind,
            });
        }

        let reachable = reachable_locks(&self.board, self.active)
            .into_iter()
            .any(|candidate| candidate.state == placement);
        if !reachable {
            return Err(GameError::UnreachablePlacement(placement));
        }

        let spin = classify_spin(&self.board, placement, last_action, self.config.spin);
        let lock = self.board.lock(placement)?;
        self.pieces_placed += 1;
        self.hold_available = true;
        let locked = LockedPlacement {
            cleared: lock.cleared,
            clear: ClearEvent::from_lock(placement.kind, lock.cleared, spin, lock.perfect_clear),
            cleared_garbage: lock.cleared_garbage,
            lock_visibility: lock.visibility,
            pieces_placed: self.pieces_placed,
        };
        self.pending_lock = Some(locked);
        Ok(locked)
    }

    /// Completes a deferred lock after versus garbage processing.
    pub fn finish_lock(&mut self) -> Result<PlacementOutcome, GameError> {
        if self.is_top_out() {
            return Err(GameError::GameOver);
        }
        let locked = self.pending_lock.take().ok_or(GameError::NoPendingLock)?;
        let next = self.take_next_piece();
        self.active = self.config.spawn.piece(next);
        let clutch_available = self.config.clutch_clear && locked.cleared.count() > 0;
        let lock_out = self
            .config
            .top_out
            .lock_reason(locked.lock_visibility)
            .filter(|_| !clutch_available);
        let clutch = lock_out.is_none()
            && self.board.collides(self.active)
            && clutch_available
            && self.raise_spawn_until_legal();
        let block_out = self
            .board
            .collides(self.active)
            .then_some(TopOutReason::BlockOut);
        self.top_out = lock_out.or(block_out);

        Ok(PlacementOutcome {
            cleared: locked.cleared,
            clear: locked.clear,
            cleared_garbage: locked.cleared_garbage,
            lock_visibility: locked.lock_visibility,
            top_out: self.is_top_out(),
            top_out_reason: self.top_out,
            next_active: self.active,
            pieces_placed: self.pieces_placed,
            clutch,
        })
    }

    /// Inserts one instant garbage row at the lock/spawn boundary. A completely
    /// filled buffer ceiling is terminal before the push, matching
    /// `AreWeToppedYet`; a partially occupied ceiling may be discarded.
    pub fn push_garbage_before_spawn(
        &mut self,
        hole_column: usize,
    ) -> Result<GarbagePushResult, GameError> {
        if self.pending_lock.is_none() {
            return Err(GameError::NoPendingLock);
        }
        if self.board.buffer_ceiling_full() {
            self.top_out = Some(TopOutReason::GarbageOut);
            return Ok(GarbagePushResult {
                overflowed_buffer: true,
            });
        }
        self.board
            .push_garbage_line(hole_column)
            .map_err(GameError::Board)
    }

    fn raise_spawn_until_legal(&mut self) -> bool {
        let initial_y = self.active.y;
        let max_y = initial_y.saturating_add(VISIBLE_HEIGHT as i16);
        while self.board.collides(self.active) && self.active.y < max_y {
            self.active.y += 1;
        }
        !self.board.collides(self.active)
            && self
                .active
                .cells()
                .into_iter()
                .all(|(_, y)| y >= 0 && y < HEIGHT as i16)
    }

    fn take_next_piece(&mut self) -> PieceKind {
        let piece = self
            .queue
            .pop_front()
            .unwrap_or_else(|| self.bag.next_piece());
        while self.queue.len() < self.config.preview {
            self.queue.push_back(self.bag.next_piece());
        }
        piece
    }

    fn ensure_playable(&self) -> Result<(), GameError> {
        if self.is_top_out() {
            Err(GameError::GameOver)
        } else if self.pending_lock.is_some() {
            Err(GameError::AwaitingSpawn)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockedPlacement {
    pub cleared: ClearedLines,
    pub clear: ClearEvent,
    pub cleared_garbage: bool,
    pub lock_visibility: LockVisibility,
    pub pieces_placed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementPreview {
    pub board: Board,
    pub locked: LockedPlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HoldOutcome {
    pub held: PieceKind,
    pub active: PieceState,
    pub top_out: bool,
    pub top_out_reason: Option<TopOutReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitialActionOutcome {
    pub active: PieceState,
    pub hold_applied: bool,
    pub rotation: Option<RotationResult>,
    pub clutch: bool,
    pub top_out: bool,
    pub top_out_reason: Option<TopOutReason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementOutcome {
    pub cleared: ClearedLines,
    pub clear: ClearEvent,
    pub cleared_garbage: bool,
    pub lock_visibility: LockVisibility,
    pub top_out: bool,
    pub top_out_reason: Option<TopOutReason>,
    pub next_active: PieceState,
    pub pieces_placed: u64,
    pub clutch: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameError {
    BagOrder(BagOrderError),
    Board(BoardError),
    GameOver,
    AwaitingSpawn,
    NoPendingLock,
    HoldAlreadyUsed,
    WrongPiece {
        active: PieceKind,
        attempted: PieceKind,
    },
    UnreachablePlacement(PieceState),
}

impl From<BagOrderError> for GameError {
    fn from(error: BagOrderError) -> Self {
        Self::BagOrder(error)
    }
}

impl From<BoardError> for GameError {
    fn from(error: BoardError) -> Self {
        Self::Board(error)
    }
}

impl fmt::Display for GameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BagOrder(error) => error.fmt(formatter),
            Self::Board(error) => error.fmt(formatter),
            Self::GameOver => write!(formatter, "the game is already top-out terminal"),
            Self::AwaitingSpawn => write!(formatter, "the locked piece is awaiting next spawn"),
            Self::NoPendingLock => write!(formatter, "there is no deferred lock to finish"),
            Self::HoldAlreadyUsed => {
                write!(formatter, "hold was already used for the active piece")
            }
            Self::WrongPiece { active, attempted } => {
                write!(formatter, "active piece is {active:?}, not {attempted:?}")
            }
            Self::UnreachablePlacement(piece) => {
                write!(formatter, "placement is not reachable: {piece:?}")
            }
        }
    }
}

impl std::error::Error for GameError {}

#[cfg(test)]
mod tests {
    use super::{GameConfig, GameError, GameState, LockedPlacement};
    use crate::{
        Board, ClearEvent, HEIGHT, InitialActions, LockVisibility, Orientation, PieceKind,
        PieceState, RotationDirection, TETRIO_7_BAG_ORDER, TopOutReason, WIDTH,
    };

    fn inject_pending_single_clear(game: &mut GameState) {
        let mut rows = [0_u16; HEIGHT];
        rows[0] = (1_u16 << WIDTH) - 1;
        let mut source = Board::from_rows(rows).expect("valid source board");
        let cleared = source
            .lock(PieceState::new(PieceKind::O, Orientation::Spawn, 3, 5))
            .expect("dummy lock")
            .cleared;
        game.pending_lock = Some(LockedPlacement {
            cleared,
            clear: ClearEvent::from_lock(PieceKind::O, cleared, None, false),
            cleared_garbage: false,
            lock_visibility: LockVisibility::Visible,
            pieces_placed: 1,
        });
        game.pieces_placed = 1;
        game.hold_available = true;
    }

    #[test]
    fn queue_has_configured_preview_length() {
        let game = GameState::new(11, GameConfig::default()).expect("valid game");
        assert_eq!(game.preview().len(), 5);
        assert!(!game.is_top_out());
    }

    #[test]
    fn hold_is_limited_until_piece_lock() {
        let mut game = GameState::new(19, GameConfig::default()).expect("valid game");
        let initial = game.active().kind;
        let hold = game.hold_active().expect("first hold succeeds");
        assert_eq!(hold.held, initial);
        assert_eq!(game.hold(), Some(initial));
        assert_eq!(game.hold_active(), Err(GameError::HoldAlreadyUsed));

        let placement = game.reachable_placements()[0].state;
        game.lock_placement(placement)
            .expect("reachable placement locks");
        assert!(game.hold_available());
        game.hold_active().expect("hold resets after lock");
    }

    #[test]
    fn initial_hold_is_resolved_before_initial_rotation() {
        let mut game = GameState::new(29, GameConfig::default()).expect("valid game");
        let outgoing = game.active().kind;
        let incoming = game.preview()[0];
        let result = game
            .apply_initial_actions(InitialActions::new(
                true,
                Some(RotationDirection::Clockwise),
            ))
            .expect("initial actions apply");

        assert!(result.hold_applied);
        assert_eq!(game.hold(), Some(outgoing));
        assert_eq!(result.active.kind, incoming);
        assert_eq!(result.active.orientation, Orientation::Right);
        assert!(result.rotation.is_some());
    }

    #[test]
    fn identical_choices_reproduce_state() {
        let mut first = GameState::new(1234, GameConfig::default()).expect("valid game");
        let mut second = first.clone();

        for _ in 0..6 {
            let first_placement = first.reachable_placements()[0].state;
            let second_placement = second.reachable_placements()[0].state;
            assert_eq!(first_placement, second_placement);
            first.lock_placement(first_placement).expect("first lock");
            second
                .lock_placement(second_placement)
                .expect("second lock");
            assert_eq!(first, second);
        }
    }

    #[test]
    fn reachable_preview_matches_mutating_deferred_lock() {
        let game = GameState::new(77, GameConfig::default()).unwrap();
        for placement in game.reachable_placements() {
            let preview = game.preview_reachable_placement(&placement).unwrap();
            let mut applied = game.clone();
            let locked = applied
                .lock_placement_deferred(placement.state, placement.last_action)
                .unwrap();
            assert_eq!(preview.locked, locked);
            assert_eq!(preview.board, *applied.board());
        }
    }

    #[test]
    fn wrong_piece_is_rejected() {
        let mut game = GameState::new(3, GameConfig::default()).expect("valid game");
        let mut placement = game.reachable_placements()[0].state;
        placement.kind = crate::PieceKind::O;
        if placement.kind == game.active().kind {
            placement.kind = crate::PieceKind::I;
        }
        assert!(matches!(
            game.lock_placement(placement),
            Err(GameError::WrongPiece { .. })
        ));
    }

    #[test]
    fn colliding_initial_spawn_reports_block_out_reason() {
        let mut rows = [0; HEIGHT];
        rows[18..23].fill((1_u16 << 10) - 1);
        let board = Board::from_rows(rows).expect("valid blocked spawn rows");
        let game = GameState::with_board(5, GameConfig::default(), board).expect("valid game");

        assert!(game.is_top_out());
        assert_eq!(game.top_out_reason(), Some(TopOutReason::BlockOut));
    }

    #[test]
    fn post_clear_clutch_raises_a_colliding_queue_spawn() {
        let mut game = GameState::new(101, GameConfig::default()).expect("valid game");
        let next_spawn = game.config.spawn.piece(game.preview()[0]);
        let shifted_cells = next_spawn.translated(0, 1).cells();
        let blocker = next_spawn
            .cells()
            .into_iter()
            .find(|cell| !shifted_cells.contains(cell))
            .expect("one-row rescue cell");
        let mut board = Board::empty();
        board
            .set_cell(blocker.0 as usize, blocker.1 as usize, true)
            .expect("valid blocker");
        game.board = board;
        inject_pending_single_clear(&mut game);

        let outcome = game.finish_lock().expect("finish lock");

        assert!(outcome.clutch);
        assert!(!outcome.top_out);
        assert_eq!(outcome.next_active.y, next_spawn.y + 1);
    }

    #[test]
    fn post_clear_clutch_also_rescues_an_ihs_replacement() {
        let mut game = GameState::new(103, GameConfig::default()).expect("valid game");
        let next_spawn = game.config.spawn.piece(game.preview()[0]);
        let next_cells = next_spawn.cells();
        let (held_kind, blocker) = TETRIO_7_BAG_ORDER
            .into_iter()
            .find_map(|kind| {
                let held = game.config.spawn.piece(kind);
                let shifted = held.translated(0, 1).cells();
                held.cells()
                    .into_iter()
                    .find(|cell| !shifted.contains(cell) && !next_cells.contains(cell))
                    .map(|cell| (kind, cell))
            })
            .expect("held-only one-row rescue cell");
        let mut board = Board::empty();
        board
            .set_cell(blocker.0 as usize, blocker.1 as usize, true)
            .expect("valid blocker");
        game.board = board;
        game.hold = Some(held_kind);
        inject_pending_single_clear(&mut game);

        let placement = game.finish_lock().expect("normal spawn");
        assert!(!placement.clutch);
        let initial = game
            .apply_initial_actions_with_clutch(InitialActions::new(true, None), true)
            .expect("IHS with clutch");

        assert!(initial.hold_applied);
        assert!(initial.clutch);
        assert!(!initial.top_out);
        assert_eq!(initial.active.kind, held_kind);
    }
}
