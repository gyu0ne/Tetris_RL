use super::{
    AttackConfigError, AttackContext, AttackError, AttackOutcome, AttackPackets, AttackRules,
    AttackState, GarbageCancellationOutcome, GarbageConfigError, GarbageError,
    GarbageInsertionOutcome, GarbageMultiplierState, GarbageRules, IncomingGarbagePacket,
    IncomingGarbageQueue, cancel_attack_packets, resolve_attack,
};
use engine_core::{
    Board, FrameSession, GameConfig, HandlingRules, InputEdge, LastAction, LockedPlacement,
    NormalizedFrame, PieceState, SessionLockFrameOutcome, SessionSpawnOutcome, SessionStepError,
    TimingSchedule, TimingScheduleError,
};
use std::fmt;

/// Complete score-free rules needed by the deterministic local 1v1 loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattleRules {
    pub game: GameConfig,
    pub timing: TimingSchedule,
    pub handling: [HandlingRules; 2],
    pub attack: AttackRules,
    pub garbage: GarbageRules,
}

impl BattleRules {
    pub fn validate(&self) -> Result<(), BattleError> {
        self.timing
            .rules_at_frame(0)
            .map_err(BattleError::TimingSchedule)?;
        self.attack
            .validate()
            .map_err(BattleError::AttackConfiguration)?;
        self.garbage
            .validate()
            .map_err(BattleError::GarbageConfiguration)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerId {
    One,
    Two,
}

impl PlayerId {
    const fn index(self) -> usize {
        match self {
            Self::One => 0,
            Self::Two => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattleResult {
    Ongoing,
    PlayerOneWin,
    PlayerTwoWin,
    Draw,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacementAction {
    pub used_hold: bool,
    pub placement: PieceState,
    pub last_action: LastAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattlePlayerState {
    session: FrameSession,
    attack: AttackState,
    incoming: IncomingGarbageQueue,
    sent_lines: u64,
}

impl BattlePlayerState {
    fn new(session: FrameSession, garbage_seed: u64) -> Self {
        Self {
            session,
            attack: AttackState::default(),
            incoming: IncomingGarbageQueue::new(garbage_seed),
            sent_lines: 0,
        }
    }

    pub const fn session(&self) -> &FrameSession {
        &self.session
    }

    pub const fn attack_state(&self) -> AttackState {
        self.attack
    }

    pub const fn incoming(&self) -> &IncomingGarbageQueue {
        &self.incoming
    }

    pub const fn sent_lines(&self) -> u64 {
        self.sent_lines
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattleSession {
    players: [BattlePlayerState; 2],
    rules: BattleRules,
    garbage_multiplier: GarbageMultiplierState,
    frame: u64,
    result: BattleResult,
}

impl BattleSession {
    /// Creates both players with the same piece seed and the same initial
    /// garbage-RNG seed. The streams remain independent and diverge only when
    /// their garbage events consume different samples.
    pub fn new(seed: u64, rules: BattleRules) -> Result<Self, BattleError> {
        Self::with_boards(seed, rules, [Board::empty(), Board::empty()])
    }

    pub fn with_boards(
        seed: u64,
        rules: BattleRules,
        boards: [Board; 2],
    ) -> Result<Self, BattleError> {
        rules.validate()?;
        let one = FrameSession::with_board(seed, rules.game, boards[0]).map_err(|source| {
            BattleError::Session {
                player: PlayerId::One,
                source,
            }
        })?;
        let two = FrameSession::with_board(seed, rules.game, boards[1]).map_err(|source| {
            BattleError::Session {
                player: PlayerId::Two,
                source,
            }
        })?;
        let result = result_from_terminals(one.is_terminal(), two.is_terminal());
        let garbage_multiplier =
            GarbageMultiplierState::new(rules.garbage.multiplier).map_err(|source| {
                BattleError::Garbage {
                    player: PlayerId::One,
                    source,
                }
            })?;
        Ok(Self {
            players: [
                BattlePlayerState::new(one, seed),
                BattlePlayerState::new(two, seed),
            ],
            rules,
            garbage_multiplier,
            frame: 0,
            result,
        })
    }

    pub const fn frame(&self) -> u64 {
        self.frame
    }

    pub const fn result(&self) -> BattleResult {
        self.result
    }

    pub const fn rules(&self) -> &BattleRules {
        &self.rules
    }

    pub const fn garbage_multiplier(&self) -> super::AttackMultiplier {
        self.garbage_multiplier.current()
    }

    pub fn player(&self, player: PlayerId) -> &BattlePlayerState {
        &self.players[player.index()]
    }

    /// Test/custom-match injection point. Normal matches populate these queues
    /// only from the opponent's uncancelled attack packets.
    pub fn enqueue_incoming(
        &mut self,
        player: PlayerId,
        packet: IncomingGarbagePacket,
    ) -> Result<(), BattleError> {
        self.players[player.index()]
            .incoming
            .enqueue(packet)
            .map_err(|source| BattleError::Garbage { player, source })
    }

    /// Advances both players through one shared frame. The operation is
    /// transactional: if either side encounters an error, neither side keeps a
    /// partial frame mutation.
    pub fn step(
        &mut self,
        player_one_edges: &[InputEdge],
        player_two_edges: &[InputEdge],
    ) -> Result<BattleFrameOutcome, BattleError> {
        let checkpoint = self.clone();
        match self.step_inner(player_one_edges, player_two_edges) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                *self = checkpoint;
                Err(error)
            }
        }
    }

    /// Advances player one from real frame input while player two locks one
    /// model-selected reachable afterstate on the same shared frame.
    ///
    /// This is the narrow adapter used by the local human-versus-model tool.
    /// It keeps simultaneous attack/cancellation ordering authoritative while
    /// preserving the model's placement-level action space.
    pub fn step_player_two_placement(
        &mut self,
        player_one_edges: &[InputEdge],
        player_two_action: PlacementAction,
    ) -> Result<BattleFrameOutcome, BattleError> {
        let checkpoint = self.clone();
        match self.step_player_two_placement_inner(player_one_edges, player_two_action) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                *self = checkpoint;
                Err(error)
            }
        }
    }

    /// Applies one simultaneous reachable afterstate per player and advances
    /// the battle clock by a shared fixed cadence. This local learning adapter
    /// preserves attack, cancellation, garbage travel/insertion, spawn and
    /// terminal order while deliberately omitting raw movement timing.
    pub fn step_placements(
        &mut self,
        actions: [PlacementAction; 2],
        frames_per_placement: u32,
    ) -> Result<BattlePlacementOutcome, BattleError> {
        let checkpoint = self.clone();
        match self.step_placements_inner(actions, frames_per_placement) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                *self = checkpoint;
                Err(error)
            }
        }
    }

    fn step_placements_inner(
        &mut self,
        actions: [PlacementAction; 2],
        frames_per_placement: u32,
    ) -> Result<BattlePlacementOutcome, BattleError> {
        if frames_per_placement == 0 {
            return Err(BattleError::ZeroPlacementCadence);
        }
        if self.result != BattleResult::Ongoing {
            return Err(BattleError::RoundOver(self.result));
        }
        if self.players[0].session.frame() != self.frame
            || self.players[1].session.frame() != self.frame
        {
            return Err(BattleError::FrameDesync {
                battle: self.frame,
                player_one: self.players[0].session.frame(),
                player_two: self.players[1].session.frame(),
            });
        }

        let frame = self.frame;
        let cadence = u64::from(frames_per_placement);
        let next_frame = frame
            .checked_add(cadence)
            .ok_or(BattleError::FrameOverflow)?;
        let [one_handling, two_handling] = self.rules.handling;
        let attack_rules = self.rules.attack;
        let garbage_rules = self.rules.garbage;
        let multiplier = self.garbage_multiplier.current();
        let [one, two] = &mut self.players;

        let (one_hold, one_locked) = one
            .session
            .lock_afterstate_deferred(
                actions[0].used_hold,
                actions[0].placement,
                actions[0].last_action,
            )
            .map_err(|source| BattleError::Session {
                player: PlayerId::One,
                source,
            })?;
        let (two_hold, two_locked) = two
            .session
            .lock_afterstate_deferred(
                actions[1].used_hold,
                actions[1].placement,
                actions[1].last_action,
            )
            .map_err(|source| BattleError::Session {
                player: PlayerId::Two,
                source,
            })?;

        let (one_attack, one_cancel) = resolve_player_attack(
            PlayerId::One,
            one,
            Some(one_locked),
            multiplier,
            attack_rules,
            garbage_rules,
        )?;
        let (two_attack, two_cancel) = resolve_player_attack(
            PlayerId::Two,
            two,
            Some(two_locked),
            multiplier,
            attack_rules,
            garbage_rules,
        )?;
        let one_outgoing = one_cancel.map_or_else(AttackPackets::empty, |value| value.outgoing);
        let two_outgoing = two_cancel.map_or_else(AttackPackets::empty, |value| value.outgoing);
        let (one_transmitted, two_transmitted) =
            cross_cancel_simultaneous(one_outgoing, two_outgoing)?;
        enqueue_generated_packets(
            PlayerId::Two,
            &mut two.incoming,
            one_transmitted,
            frame,
            garbage_rules,
        )?;
        enqueue_generated_packets(
            PlayerId::One,
            &mut one.incoming,
            two_transmitted,
            frame,
            garbage_rules,
        )?;

        let one_insertion =
            insert_for_player(PlayerId::One, one, Some(one_locked), frame, garbage_rules)?;
        let two_insertion =
            insert_for_player(PlayerId::Two, two, Some(two_locked), frame, garbage_rules)?;
        let one_spawn = finish_player_spawn(PlayerId::One, one, Some(one_locked), one_handling)?;
        let two_spawn = finish_player_spawn(PlayerId::Two, two, Some(two_locked), two_handling)?;

        self.result = result_from_terminals(one.session.is_terminal(), two.session.is_terminal());
        for elapsed in 0..cadence {
            self.garbage_multiplier
                .advance_end_of_frame(frame + elapsed, garbage_rules.multiplier)
                .map_err(|source| BattleError::Garbage {
                    player: PlayerId::One,
                    source,
                })?;
        }
        one.session.advance_placement_clock(cadence);
        two.session.advance_placement_clock(cadence);
        self.frame = next_frame;

        Ok(BattlePlacementOutcome {
            frame,
            frames_per_placement,
            player_one: BattlePlacementPlayerOutcome {
                locked: one_locked,
                hold_applied: one_hold,
                attack: one_attack.expect("placement always resolves attack"),
                cancellation: one_cancel.expect("placement always resolves cancellation"),
                insertion: one_insertion.expect("placement always checks insertion"),
                spawn: one_spawn,
                transmitted: one_transmitted,
            },
            player_two: BattlePlacementPlayerOutcome {
                locked: two_locked,
                hold_applied: two_hold,
                attack: two_attack.expect("placement always resolves attack"),
                cancellation: two_cancel.expect("placement always resolves cancellation"),
                insertion: two_insertion.expect("placement always checks insertion"),
                spawn: two_spawn,
                transmitted: two_transmitted,
            },
            result: self.result,
        })
    }

    fn step_inner(
        &mut self,
        player_one_edges: &[InputEdge],
        player_two_edges: &[InputEdge],
    ) -> Result<BattleFrameOutcome, BattleError> {
        if self.result != BattleResult::Ongoing {
            return Err(BattleError::RoundOver(self.result));
        }
        if self.players[0].session.frame() != self.frame
            || self.players[1].session.frame() != self.frame
        {
            return Err(BattleError::FrameDesync {
                battle: self.frame,
                player_one: self.players[0].session.frame(),
                player_two: self.players[1].session.frame(),
            });
        }

        let frame = self.frame;
        let timing = self
            .rules
            .timing
            .rules_at_frame(frame)
            .map_err(BattleError::TimingSchedule)?;
        let [one_handling, two_handling] = self.rules.handling;
        let [one, two] = &mut self.players;
        let one_lock = one
            .session
            .step_until_lock(timing, one_handling, player_one_edges)
            .map_err(|source| BattleError::Session {
                player: PlayerId::One,
                source,
            })?;
        let two_lock = two
            .session
            .step_until_lock(timing, two_handling, player_two_edges)
            .map_err(|source| BattleError::Session {
                player: PlayerId::Two,
                source,
            })?;

        self.resolve_frame(frame, one_lock, two_lock)
    }

    fn step_player_two_placement_inner(
        &mut self,
        player_one_edges: &[InputEdge],
        player_two_action: PlacementAction,
    ) -> Result<BattleFrameOutcome, BattleError> {
        self.validate_frame_start()?;
        let frame = self.frame;
        let timing = self
            .rules
            .timing
            .rules_at_frame(frame)
            .map_err(BattleError::TimingSchedule)?;
        let [one_handling, _] = self.rules.handling;
        let [one, two] = &mut self.players;
        let one_lock = one
            .session
            .step_until_lock(timing, one_handling, player_one_edges)
            .map_err(|source| BattleError::Session {
                player: PlayerId::One,
                source,
            })?;
        let (hold_applied, locked) = two
            .session
            .lock_afterstate_deferred(
                player_two_action.used_hold,
                player_two_action.placement,
                player_two_action.last_action,
            )
            .map_err(|source| BattleError::Session {
                player: PlayerId::Two,
                source,
            })?;
        two.session.advance_placement_clock(1);
        let two_lock = SessionLockFrameOutcome {
            frame,
            normalized: NormalizedFrame {
                actions: Vec::new(),
                hold_requested: player_two_action.used_hold,
                hold_action_index: player_two_action.used_hold.then_some(0),
            },
            timing: None,
            locked: Some(locked),
            hold_applied,
            terminal: two.session.is_terminal(),
        };

        self.resolve_frame(frame, one_lock, two_lock)
    }

    fn validate_frame_start(&self) -> Result<(), BattleError> {
        if self.result != BattleResult::Ongoing {
            return Err(BattleError::RoundOver(self.result));
        }
        if self.players[0].session.frame() != self.frame
            || self.players[1].session.frame() != self.frame
        {
            return Err(BattleError::FrameDesync {
                battle: self.frame,
                player_one: self.players[0].session.frame(),
                player_two: self.players[1].session.frame(),
            });
        }
        Ok(())
    }

    fn resolve_frame(
        &mut self,
        frame: u64,
        one_lock: SessionLockFrameOutcome,
        two_lock: SessionLockFrameOutcome,
    ) -> Result<BattleFrameOutcome, BattleError> {
        let [one_handling, two_handling] = self.rules.handling;
        let attack_rules = self.rules.attack;
        let garbage_rules = self.rules.garbage;
        let multiplier = self.garbage_multiplier.current();
        let [one, two] = &mut self.players;

        let (one_attack, one_cancel) = resolve_player_attack(
            PlayerId::One,
            one,
            one_lock.locked,
            multiplier,
            attack_rules,
            garbage_rules,
        )?;
        let (two_attack, two_cancel) = resolve_player_attack(
            PlayerId::Two,
            two,
            two_lock.locked,
            multiplier,
            attack_rules,
            garbage_rules,
        )?;

        let one_outgoing = one_cancel.map_or_else(AttackPackets::empty, |value| value.outgoing);
        let two_outgoing = two_cancel.map_or_else(AttackPackets::empty, |value| value.outgoing);
        let (one_transmitted, two_transmitted) =
            cross_cancel_simultaneous(one_outgoing, two_outgoing)?;
        enqueue_generated_packets(
            PlayerId::Two,
            &mut two.incoming,
            one_transmitted,
            frame,
            garbage_rules,
        )?;
        enqueue_generated_packets(
            PlayerId::One,
            &mut one.incoming,
            two_transmitted,
            frame,
            garbage_rules,
        )?;

        let one_insertion =
            insert_for_player(PlayerId::One, one, one_lock.locked, frame, garbage_rules)?;
        let two_insertion =
            insert_for_player(PlayerId::Two, two, two_lock.locked, frame, garbage_rules)?;

        let one_spawn = finish_player_spawn(PlayerId::One, one, one_lock.locked, one_handling)?;
        let two_spawn = finish_player_spawn(PlayerId::Two, two, two_lock.locked, two_handling)?;

        self.result = result_from_terminals(one.session.is_terminal(), two.session.is_terminal());
        self.garbage_multiplier
            .advance_end_of_frame(frame, garbage_rules.multiplier)
            .map_err(|source| BattleError::Garbage {
                player: PlayerId::One,
                source,
            })?;
        self.frame = self.frame.saturating_add(1);

        Ok(BattleFrameOutcome {
            frame,
            player_one: BattlePlayerFrameOutcome {
                lock: one_lock,
                attack: one_attack,
                cancellation: one_cancel,
                insertion: one_insertion,
                spawn: one_spawn,
                transmitted: one_transmitted,
            },
            player_two: BattlePlayerFrameOutcome {
                lock: two_lock,
                attack: two_attack,
                cancellation: two_cancel,
                insertion: two_insertion,
                spawn: two_spawn,
                transmitted: two_transmitted,
            },
            result: self.result,
        })
    }
}

fn resolve_player_attack(
    id: PlayerId,
    player: &mut BattlePlayerState,
    locked: Option<LockedPlacement>,
    multiplier: super::AttackMultiplier,
    attack_rules: AttackRules,
    garbage_rules: GarbageRules,
) -> Result<(Option<AttackOutcome>, Option<GarbageCancellationOutcome>), BattleError> {
    let Some(locked) = locked else {
        return Ok((None, None));
    };
    let attack = resolve_attack(
        player.attack,
        locked.clear,
        AttackContext {
            cleared_garbage: locked.cleared_garbage,
            multiplier,
        },
        attack_rules,
    )
    .map_err(|source| BattleError::Attack { player: id, source })?;
    player.attack = attack.state;
    let cancellation = cancel_attack_packets(
        &mut player.incoming,
        attack.packets,
        locked.pieces_placed,
        player.sent_lines,
        garbage_rules,
    )
    .map_err(|source| BattleError::Garbage { player: id, source })?;
    player.sent_lines = cancellation.sent_lines_after;
    Ok((Some(attack), Some(cancellation)))
}

/// Zero passthrough acknowledges both players' already-sent packets when the
/// corresponding same-frame attacks arrive. Removing the common prefix before
/// enqueueing reproduces that symmetric acknowledgement without player-order
/// bias and without consuming either receiver's hole RNG.
fn cross_cancel_simultaneous(
    one: AttackPackets,
    two: AttackPackets,
) -> Result<(AttackPackets, AttackPackets), BattleError> {
    let common = one.total().min(two.total());
    Ok((
        discard_attack_prefix(one, common)?,
        discard_attack_prefix(two, common)?,
    ))
}

fn discard_attack_prefix(
    packets: AttackPackets,
    mut amount: u64,
) -> Result<AttackPackets, BattleError> {
    let mut remaining = AttackPackets::empty();
    for packet in packets.as_slice() {
        let cancelled = u64::from(packet.lines).min(amount);
        amount -= cancelled;
        let lines = packet.lines
            - u32::try_from(cancelled).map_err(|_| BattleError::Attack {
                player: PlayerId::One,
                source: AttackError::CounterOverflow,
            })?;
        remaining
            .push(packet.kind, lines)
            .map_err(|source| BattleError::Attack {
                player: PlayerId::One,
                source,
            })?;
    }
    Ok(remaining)
}

fn enqueue_generated_packets(
    receiver: PlayerId,
    incoming: &mut IncomingGarbageQueue,
    packets: AttackPackets,
    frame: u64,
    rules: GarbageRules,
) -> Result<(), BattleError> {
    for packet in packets.as_slice() {
        let generated =
            IncomingGarbagePacket::after_travel_generated(packet.lines, frame, false, rules)
                .map_err(|source| BattleError::Garbage {
                    player: receiver,
                    source,
                })?;
        incoming
            .enqueue(generated)
            .map_err(|source| BattleError::Garbage {
                player: receiver,
                source,
            })?;
    }
    Ok(())
}

fn insert_for_player(
    id: PlayerId,
    player: &mut BattlePlayerState,
    locked: Option<LockedPlacement>,
    frame: u64,
    rules: GarbageRules,
) -> Result<Option<GarbageInsertionOutcome>, BattleError> {
    let Some(locked) = locked else {
        return Ok(None);
    };
    if rules.combo_blocking && locked.cleared.count() > 0 {
        return Ok(Some(GarbageInsertionOutcome {
            inserted: 0,
            overflowed_buffer: false,
            blocked_by_clear: true,
        }));
    }

    let mut inserted = 0_u8;
    let mut overflowed_buffer = false;
    while inserted < rules.garbage_cap {
        let Some(hole_column) = player
            .incoming
            .take_ready_line(frame, rules)
            .map_err(|source| BattleError::Garbage { player: id, source })?
        else {
            break;
        };
        let pushed = player
            .session
            .push_garbage_before_spawn(usize::from(hole_column))
            .map_err(|source| BattleError::Session { player: id, source })?;
        inserted += 1;
        overflowed_buffer |= pushed.overflowed_buffer;
        if player.session.is_terminal() {
            break;
        }
    }
    Ok(Some(GarbageInsertionOutcome {
        inserted,
        overflowed_buffer,
        blocked_by_clear: false,
    }))
}

fn finish_player_spawn(
    id: PlayerId,
    player: &mut BattlePlayerState,
    locked: Option<LockedPlacement>,
    handling: HandlingRules,
) -> Result<Option<SessionSpawnOutcome>, BattleError> {
    if locked.is_none() || player.session.is_terminal() {
        return Ok(None);
    }
    player
        .session
        .finish_pending_spawn(handling)
        .map(Some)
        .map_err(|source| BattleError::Session { player: id, source })
}

const fn result_from_terminals(one: bool, two: bool) -> BattleResult {
    match (one, two) {
        (false, false) => BattleResult::Ongoing,
        (false, true) => BattleResult::PlayerOneWin,
        (true, false) => BattleResult::PlayerTwoWin,
        (true, true) => BattleResult::Draw,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattlePlayerFrameOutcome {
    pub lock: SessionLockFrameOutcome,
    pub attack: Option<AttackOutcome>,
    pub cancellation: Option<GarbageCancellationOutcome>,
    pub insertion: Option<GarbageInsertionOutcome>,
    pub spawn: Option<SessionSpawnOutcome>,
    /// Packet sequence that remains after same-frame zero passthrough.
    pub transmitted: AttackPackets,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattleFrameOutcome {
    pub frame: u64,
    pub player_one: BattlePlayerFrameOutcome,
    pub player_two: BattlePlayerFrameOutcome,
    pub result: BattleResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattlePlacementPlayerOutcome {
    pub locked: LockedPlacement,
    pub hold_applied: bool,
    pub attack: AttackOutcome,
    pub cancellation: GarbageCancellationOutcome,
    pub insertion: GarbageInsertionOutcome,
    pub spawn: Option<SessionSpawnOutcome>,
    pub transmitted: AttackPackets,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattlePlacementOutcome {
    pub frame: u64,
    pub frames_per_placement: u32,
    pub player_one: BattlePlacementPlayerOutcome,
    pub player_two: BattlePlacementPlayerOutcome,
    pub result: BattleResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BattleError {
    AttackConfiguration(AttackConfigError),
    GarbageConfiguration(GarbageConfigError),
    TimingSchedule(TimingScheduleError),
    Session {
        player: PlayerId,
        source: SessionStepError,
    },
    Attack {
        player: PlayerId,
        source: AttackError,
    },
    Garbage {
        player: PlayerId,
        source: GarbageError,
    },
    FrameDesync {
        battle: u64,
        player_one: u64,
        player_two: u64,
    },
    RoundOver(BattleResult),
    ZeroPlacementCadence,
    FrameOverflow,
}

impl fmt::Display for BattleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttackConfiguration(error) => {
                write!(formatter, "invalid attack configuration: {error:?}")
            }
            Self::GarbageConfiguration(error) => {
                write!(formatter, "invalid garbage configuration: {error:?}")
            }
            Self::TimingSchedule(error) => {
                write!(formatter, "invalid timing schedule: {error:?}")
            }
            Self::Session { player, source } => {
                write!(formatter, "player {player:?} session failed: {source}")
            }
            Self::Attack { player, source } => {
                write!(formatter, "player {player:?} attack failed: {source}")
            }
            Self::Garbage { player, source } => {
                write!(formatter, "player {player:?} garbage failed: {source}")
            }
            Self::FrameDesync {
                battle,
                player_one,
                player_two,
            } => write!(
                formatter,
                "battle frame {battle} differs from player frames {player_one}/{player_two}"
            ),
            Self::RoundOver(result) => write!(formatter, "battle round already ended: {result:?}"),
            Self::ZeroPlacementCadence => {
                formatter.write_str("placement cadence must be at least one frame")
            }
            Self::FrameOverflow => formatter.write_str("battle frame counter overflowed"),
        }
    }
}

impl std::error::Error for BattleError {}

#[cfg(test)]
mod tests {
    use super::{
        BattleError, BattleResult, BattleRules, BattleSession, PlacementAction, PlayerId,
        cross_cancel_simultaneous,
    };
    use crate::{
        AttackPacketKind, AttackPackets, AttackRules, ComboRule, GarbageMessinessRules,
        GarbageMultiplierSchedule, GarbageRules, IncomingGarbagePacket,
    };
    use engine_core::{
        Board, GameConfig, GameState, Gravity, HandlingRules, InputButton, InputEdge, PieceKind,
        SoftDropMode, TimingRules, TimingSchedule, WIDTH,
    };

    fn rules() -> BattleRules {
        BattleRules {
            game: GameConfig::default(),
            timing: TimingSchedule::fixed(TimingRules::new(
                Gravity::new(0, 1).unwrap(),
                30,
                15,
                true,
                true,
            )),
            handling: [HandlingRules::new(10, 2, 0, SoftDropMode::Disabled); 2],
            attack: AttackRules {
                normal_clear: [0, 0, 1, 2, 4],
                mini_spin: [0, 0, 1, 2, 4],
                full_spin: [0, 2, 4, 6, 10],
                combo: ComboRule::None,
                back_to_back_bonus: 1,
                back_to_back_charging: false,
                back_to_back_charge_at: 4,
                back_to_back_charge_base: 0,
                perfect_clear_attack: 10,
                perfect_clear_back_to_back: 0,
                perfect_clear_back_to_back_sends: false,
                perfect_clear_back_to_back_dupes: false,
                perfect_clear_charges: false,
                garbage_clear_special_bonus: 0,
            },
            garbage: GarbageRules {
                travel_frames: 0,
                garbage_cap: 8,
                opener_phase_pieces: 0,
                combo_blocking: true,
                messiness: GarbageMessinessRules::tetra_league_observed(),
                multiplier: GarbageMultiplierSchedule::tetra_league_observed(),
            },
        }
    }

    fn hard_drop() -> [InputEdge; 1] {
        [InputEdge::press(InputButton::HardDrop)]
    }

    fn first_placement(battle: &BattleSession, player: PlayerId) -> PlacementAction {
        let placement = battle
            .player(player)
            .session()
            .game()
            .reachable_placements()
            .into_iter()
            .next()
            .expect("reachable placement");
        PlacementAction {
            used_hold: false,
            placement: placement.state,
            last_action: placement.last_action,
        }
    }

    #[test]
    fn placement_adapter_advances_both_players_by_shared_cadence() {
        let mut battle = BattleSession::new(73, rules()).expect("valid battle");
        let actions = [
            first_placement(&battle, PlayerId::One),
            first_placement(&battle, PlayerId::Two),
        ];

        let outcome = battle.step_placements(actions, 12).expect("placement step");

        assert_eq!(outcome.frame, 0);
        assert_eq!(outcome.frames_per_placement, 12);
        assert_eq!(battle.frame(), 12);
        assert_eq!(
            battle
                .player(PlayerId::One)
                .session()
                .game()
                .pieces_placed(),
            1
        );
        assert_eq!(
            battle
                .player(PlayerId::Two)
                .session()
                .game()
                .pieces_placed(),
            1
        );
    }

    #[test]
    fn zero_placement_cadence_is_rejected_transactionally() {
        let mut battle = BattleSession::new(79, rules()).expect("valid battle");
        let before = battle.clone();
        let actions = [
            first_placement(&battle, PlayerId::One),
            first_placement(&battle, PlayerId::Two),
        ];

        assert_eq!(
            battle.step_placements(actions, 0),
            Err(BattleError::ZeroPlacementCadence)
        );
        assert_eq!(battle, before);
    }

    #[test]
    fn simultaneous_zero_passthrough_is_symmetric_and_preserves_packet_order() {
        let mut one = AttackPackets::empty();
        one.push(AttackPacketKind::Surge, 2).unwrap();
        one.push(AttackPacketKind::Clear, 4).unwrap();
        let mut two = AttackPackets::empty();
        two.push(AttackPacketKind::Clear, 3).unwrap();

        let (one, two) = cross_cancel_simultaneous(one, two).expect("valid cancellation");
        assert_eq!(one.as_slice().len(), 1);
        assert_eq!(one.as_slice()[0].kind, AttackPacketKind::Clear);
        assert_eq!(one.as_slice()[0].lines, 3);
        assert!(two.is_empty());
    }

    #[test]
    fn simultaneous_lock_attacks_cancel_inside_battle_transaction() {
        let (seed, probe) = (1_u64..=100)
            .find_map(|seed| {
                let game = GameState::new(seed, GameConfig::default()).ok()?;
                (game.active().kind == PieceKind::O).then_some((seed, game))
            })
            .expect("an O opener seed");
        let active = probe.active();
        let full = (1_u16 << WIDTH) - 1;
        let gap = (1_u16 << (active.x + 1)) | (1_u16 << (active.x + 2));
        let mut rows = [0_u16; engine_core::HEIGHT];
        rows[0] = full & !gap;
        rows[1] = full & !gap;
        let board = Board::from_rows(rows).expect("valid double-clear board");
        let mut battle = BattleSession::with_boards(seed, rules(), [board, board])
            .expect("valid mirrored battle");

        let outcome = battle.step(&hard_drop(), &hard_drop()).expect("lock frame");

        assert!(outcome.player_one.transmitted.is_empty());
        assert!(outcome.player_two.transmitted.is_empty());
        assert_eq!(battle.player(PlayerId::One).incoming().pending_lines(), 0);
        assert_eq!(battle.player(PlayerId::Two).incoming().pending_lines(), 0);
        assert_eq!(outcome.result, BattleResult::Ongoing);
    }

    #[test]
    fn ready_garbage_is_inserted_only_at_a_lock_boundary() {
        let mut battle = BattleSession::new(91, rules()).expect("valid battle");
        battle
            .enqueue_incoming(
                PlayerId::One,
                IncomingGarbagePacket::after_travel_generated(2, 0, false, rules().garbage)
                    .unwrap(),
            )
            .unwrap();

        let idle = battle.step(&[], &[]).expect("idle frame");
        assert!(idle.player_one.insertion.is_none());
        assert_eq!(battle.player(PlayerId::One).incoming().ready_lines(1), 2);

        let lock = battle.step(&hard_drop(), &hard_drop()).expect("lock frame");
        assert_eq!(lock.player_one.insertion.unwrap().inserted, 2);
        assert_eq!(battle.player(PlayerId::One).incoming().pending_lines(), 0);
        assert_eq!(
            battle
                .player(PlayerId::One)
                .session()
                .game()
                .board()
                .garbage_rows()[0]
                .count_ones(),
            9
        );
    }

    #[test]
    fn battle_advances_js_multiplier_only_after_frame_end() {
        let mut battle_rules = rules();
        battle_rules.garbage.multiplier.margin_frames = 0;
        let mut battle = BattleSession::new(89, battle_rules).expect("valid battle");

        battle.step(&[], &[]).expect("frame zero");
        assert_eq!(battle.garbage_multiplier().value(), 1.0);
        battle.step(&[], &[]).expect("frame one");

        assert_eq!(
            battle.garbage_multiplier().value().to_bits(),
            (1.0_f64 + 0.008_f64 / 60.0).to_bits()
        );
    }

    #[test]
    fn simultaneous_block_out_is_a_draw() {
        let seed = 97;
        let config = GameConfig::default();
        let probe = GameState::new(seed, config).expect("valid probe game");
        let active_cells = probe.active().cells();
        let next_spawn = config.spawn.piece(probe.preview()[0]);
        let blocker = next_spawn
            .cells()
            .into_iter()
            .find(|cell| !active_cells.contains(cell))
            .expect("different consecutive spawn footprints");
        let mut board = Board::empty();
        board
            .set_cell(blocker.0 as usize, blocker.1 as usize, true)
            .expect("valid blocker");
        let mut battle =
            BattleSession::with_boards(seed, rules(), [board, board]).expect("valid battle");

        let outcome = battle.step(&hard_drop(), &hard_drop()).expect("lock frame");
        assert_eq!(outcome.result, BattleResult::Draw);
        assert!(battle.player(PlayerId::One).session().is_terminal());
        assert!(battle.player(PlayerId::Two).session().is_terminal());
    }

    #[test]
    fn one_sided_block_out_awards_the_other_player() {
        let seed = 101;
        let config = GameConfig::default();
        let probe = GameState::new(seed, config).expect("valid probe game");
        let active_cells = probe.active().cells();
        let next_spawn = config.spawn.piece(probe.preview()[0]);
        let blocker = next_spawn
            .cells()
            .into_iter()
            .find(|cell| !active_cells.contains(cell))
            .expect("different consecutive spawn footprints");
        let mut blocked_board = Board::empty();
        blocked_board
            .set_cell(blocker.0 as usize, blocker.1 as usize, true)
            .expect("valid blocker");
        let mut battle = BattleSession::with_boards(seed, rules(), [blocked_board, Board::empty()])
            .expect("valid battle");

        let outcome = battle.step(&hard_drop(), &hard_drop()).expect("lock frame");
        assert_eq!(outcome.result, BattleResult::PlayerTwoWin);
        assert!(battle.player(PlayerId::One).session().is_terminal());
        assert!(!battle.player(PlayerId::Two).session().is_terminal());
    }
}
