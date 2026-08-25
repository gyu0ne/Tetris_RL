//! Deterministic, score-free 1v1 attack transitions.
//!
//! `engine-core` reports a [`ClearEvent`]. This crate applies an explicit rules
//! profile and emits ordered garbage attack packets. It deliberately contains
//! no 40 LINES or BLITZ score accounting.

#![forbid(unsafe_code)]

mod battle;

pub use battle::{
    BattleError, BattleFrameOutcome, BattlePlayerFrameOutcome, BattlePlayerState, BattleResult,
    BattleRules, BattleSession, PlayerId,
};

use engine_core::{
    Board, BoardError, ClearEvent, MinStd, PlacementOutcome, SpinClassification, WIDTH,
};
use std::{collections::VecDeque, fmt};

const MAX_ATTACK_PACKETS: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComboRule {
    None,
    /// Multiplies nonzero attack by a rational linear combo factor. When the
    /// pre-combo attack is zero, `zero_base_min_combo_index[k - 1]` is the
    /// first combo index whose rounded-down logarithmic minimum sends `k`.
    Multiplier {
        increment_numerator: u32,
        increment_denominator: u32,
        zero_base_min_combo_index: &'static [u32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackRules {
    pub normal_clear: [u16; 5],
    pub mini_spin: [u16; 5],
    pub full_spin: [u16; 5],
    pub combo: ComboRule,
    pub back_to_back_bonus: u16,
    pub back_to_back_charging: bool,
    pub back_to_back_charge_at: u32,
    pub back_to_back_charge_base: u32,
    pub perfect_clear_attack: u16,
    pub perfect_clear_back_to_back: u32,
    pub perfect_clear_back_to_back_sends: bool,
    pub perfect_clear_back_to_back_dupes: bool,
    pub perfect_clear_charges: bool,
    pub garbage_clear_special_bonus: u16,
}

impl AttackRules {
    pub fn validate(self) -> Result<(), AttackConfigError> {
        if let ComboRule::Multiplier {
            increment_denominator,
            zero_base_min_combo_index,
            ..
        } = self.combo
        {
            if increment_denominator == 0 {
                return Err(AttackConfigError::ZeroComboDenominator);
            }
            if zero_base_min_combo_index
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(AttackConfigError::NonIncreasingComboThresholds);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AttackState {
    pub combo: u32,
    pub back_to_back: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackContext {
    /// True when at least one cleared row contained garbage cells.
    pub cleared_garbage: bool,
    /// Effective garbage multiplier before this frame's end-of-frame increase.
    pub multiplier: AttackMultiplier,
}

impl Default for AttackContext {
    fn default() -> Self {
        Self {
            cleared_garbage: false,
            multiplier: AttackMultiplier::one(),
        }
    }
}

impl From<PlacementOutcome> for AttackContext {
    fn from(outcome: PlacementOutcome) -> Self {
        Self {
            cleared_garbage: outcome.cleared_garbage,
            multiplier: AttackMultiplier::one(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackMultiplier {
    /// Exact IEEE-754 payload used by the pinned JavaScript client.
    value_bits: u64,
}

impl AttackMultiplier {
    pub const fn one() -> Self {
        Self {
            value_bits: 0x3ff0_0000_0000_0000,
        }
    }

    pub fn new(numerator: u64, denominator: u64) -> Result<Self, AttackConfigError> {
        if denominator == 0 {
            return Err(AttackConfigError::ZeroGarbageMultiplierDenominator);
        }
        let value = numerator as f64 / denominator as f64;
        if !value.is_finite() {
            return Err(AttackConfigError::NonFiniteGarbageMultiplier);
        }
        Ok(Self::from_f64(value))
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.value_bits)
    }

    /// Reconstructs the exact finite IEEE-754 payload emitted by a pinned
    /// reference client. This is intentionally narrower than accepting an
    /// arbitrary floating-point value through the rules API.
    pub fn from_ieee_bits(value_bits: u64) -> Result<Self, AttackConfigError> {
        if !f64::from_bits(value_bits).is_finite() {
            return Err(AttackConfigError::NonFiniteGarbageMultiplier);
        }
        Ok(Self { value_bits })
    }

    /// Returns the exact payload used for deterministic trace interchange.
    pub const fn ieee_bits(self) -> u64 {
        self.value_bits
    }

    const fn from_f64(value: f64) -> Self {
        Self {
            value_bits: value.to_bits(),
        }
    }

    fn scale_floor(self, value: u32) -> Result<u32, AttackError> {
        floor_js_attack(f64::from(value) * self.value())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttackPacketKind {
    Surge,
    Clear,
    PerfectClear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackPacket {
    pub kind: AttackPacketKind,
    pub lines: u32,
}

impl AttackPacket {
    const EMPTY: Self = Self {
        kind: AttackPacketKind::Clear,
        lines: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackPackets {
    values: [AttackPacket; MAX_ATTACK_PACKETS],
    len: u8,
}

impl AttackPackets {
    pub const fn empty() -> Self {
        Self {
            values: [AttackPacket::EMPTY; MAX_ATTACK_PACKETS],
            len: 0,
        }
    }

    pub fn as_slice(&self) -> &[AttackPacket] {
        &self.values[..usize::from(self.len)]
    }

    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn total(&self) -> u64 {
        self.as_slice()
            .iter()
            .map(|packet| u64::from(packet.lines))
            .sum()
    }

    /// Builds an ordered packet sequence for reference-trace input while
    /// preserving the runtime rule that zero-sized packets are omitted.
    pub fn try_from_slice(packets: &[AttackPacket]) -> Result<Self, AttackError> {
        let mut result = Self::empty();
        for packet in packets {
            result.push(packet.kind, packet.lines)?;
        }
        Ok(result)
    }

    fn push(&mut self, kind: AttackPacketKind, lines: u32) -> Result<(), AttackError> {
        if lines == 0 {
            return Ok(());
        }
        let index = usize::from(self.len);
        let slot = self
            .values
            .get_mut(index)
            .ok_or(AttackError::PacketCapacityExceeded)?;
        *slot = AttackPacket { kind, lines };
        self.len += 1;
        Ok(())
    }
}

impl Default for AttackPackets {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackOutcome {
    pub state: AttackState,
    pub packets: AttackPackets,
    pub base_attack: u32,
    pub clear_attack: u32,
    pub back_to_back_bonus: u32,
    pub special_bonus: u32,
    pub surge_attack: u32,
    pub perfect_clear_attack: u32,
    pub difficult: bool,
    pub back_to_back: bool,
}

impl AttackOutcome {
    pub fn total_attack(self) -> u64 {
        self.packets.total()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageRules {
    /// Frames between sending an attack and making it eligible to rise.
    pub travel_frames: u32,
    /// Maximum ready garbage lines that may rise after one tanking placement.
    pub garbage_cap: u8,
    /// Last placed-piece count eligible for opener double cancellation.
    pub opener_phase_pieces: u64,
    /// When true, any line clear blocks garbage insertion for that placement.
    pub combo_blocking: bool,
    /// Hole-change behavior and its exact RNG-consumption contract.
    pub messiness: GarbageMessinessRules,
    /// Frame-derived attack scaling used after the TL garbage margin.
    pub multiplier: GarbageMultiplierSchedule,
}

impl GarbageRules {
    pub fn validate(self) -> Result<(), GarbageConfigError> {
        if self.garbage_cap == 0 {
            return Err(GarbageConfigError::ZeroGarbageCap);
        }
        self.messiness.validate()?;
        self.multiplier.validate()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageMultiplierSchedule {
    pub initial_numerator: u64,
    pub initial_denominator: u64,
    pub increase_numerator_per_second: u64,
    pub increase_denominator_per_second: u64,
    pub margin_frames: u64,
    pub tick_rate_hz: u32,
}

impl GarbageMultiplierSchedule {
    pub const fn tetra_league_observed() -> Self {
        Self {
            initial_numerator: 1,
            initial_denominator: 1,
            increase_numerator_per_second: 1,
            increase_denominator_per_second: 125,
            margin_frames: 10_800,
            tick_rate_hz: 60,
        }
    }

    fn validate(self) -> Result<(), GarbageConfigError> {
        if self.initial_denominator == 0 {
            return Err(GarbageConfigError::ZeroInitialMultiplierDenominator);
        }
        if self.increase_denominator_per_second == 0 {
            return Err(GarbageConfigError::ZeroMultiplierIncreaseDenominator);
        }
        if self.tick_rate_hz == 0 {
            return Err(GarbageConfigError::ZeroTickRate);
        }
        Ok(())
    }
}

/// Stateful because repeated JavaScript `+=` rounding is observably different
/// from an exact rational or from one multiplication by elapsed frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageMultiplierState {
    current: AttackMultiplier,
}

impl GarbageMultiplierState {
    pub fn new(schedule: GarbageMultiplierSchedule) -> Result<Self, GarbageError> {
        schedule.validate()?;
        let value = schedule.initial_numerator as f64 / schedule.initial_denominator as f64;
        if !value.is_finite() {
            return Err(GarbageError::CounterOverflow);
        }
        Ok(Self {
            current: AttackMultiplier::from_f64(value),
        })
    }

    pub const fn current(self) -> AttackMultiplier {
        self.current
    }

    /// The client applies attacks before incrementing the multiplier at the
    /// end of a frame, and only when `frame > margin`.
    pub fn advance_end_of_frame(
        &mut self,
        frame: u64,
        schedule: GarbageMultiplierSchedule,
    ) -> Result<(), GarbageError> {
        schedule.validate()?;
        if frame <= schedule.margin_frames {
            return Ok(());
        }
        let increase = schedule.increase_numerator_per_second as f64
            / schedule.increase_denominator_per_second as f64
            / f64::from(schedule.tick_rate_hz);
        let next = self.current.value() + increase;
        if !next.is_finite() || next < 0.0 {
            return Err(GarbageError::CounterOverflow);
        }
        self.current = AttackMultiplier::from_f64(next);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageMessinessRules {
    pub change_numerator: u32,
    pub change_denominator: u32,
    pub inner_numerator: u32,
    pub inner_denominator: u32,
    pub no_same: bool,
    pub center: bool,
}

impl GarbageMessinessRules {
    /// Current TETRA LEAGUE change-on-attack behavior: a packet keeps one hole,
    /// then always rerolls when that packet is depleted or fully cancelled.
    pub const fn tetra_league_observed() -> Self {
        Self {
            change_numerator: 1,
            change_denominator: 1,
            inner_numerator: 0,
            inner_denominator: 1,
            no_same: false,
            center: false,
        }
    }

    fn validate(self) -> Result<(), GarbageConfigError> {
        if self.change_denominator == 0 {
            return Err(GarbageConfigError::ZeroMessinessChangeDenominator);
        }
        if self.inner_denominator == 0 {
            return Err(GarbageConfigError::ZeroMessinessInnerDenominator);
        }
        let edge_exclusion = usize::from(self.center) * ((WIDTH + 2) / 5);
        let available = WIDTH.saturating_sub(2 * edge_exclusion);
        if available == 0 || (self.no_same && available < 2) {
            return Err(GarbageConfigError::NoAvailableGarbageHole);
        }
        Ok(())
    }
}

/// Receiver-side hole state. TETR.IO initializes a second MINSTD stream with
/// the game seed; it is independent from the piece bag stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageHoleGenerator {
    rng: MinStd,
    last_column: Option<u8>,
    has_changed_column: bool,
}

impl GarbageHoleGenerator {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: MinStd::new(seed),
            last_column: None,
            has_changed_column: false,
        }
    }

    pub const fn rng_state(self) -> u32 {
        self.rng.state()
    }

    pub const fn last_column(self) -> Option<u8> {
        self.last_column
    }

    pub const fn has_changed_column(self) -> bool {
        self.has_changed_column
    }

    fn line_column(
        &mut self,
        explicit: Option<u8>,
        rules: GarbageMessinessRules,
    ) -> Result<u8, GarbageError> {
        let column = if let Some(column) = explicit {
            column
        } else {
            let should_change = match self.last_column {
                None => true,
                Some(_) => self
                    .rng
                    .chance(rules.inner_numerator, rules.inner_denominator),
            };
            if should_change && !self.has_changed_column {
                self.reroll(rules)?;
                self.has_changed_column = true;
            }
            self.last_column.ok_or(GarbageError::MissingGeneratedHole)?
        };
        // TakeAllDamage resets this flag after every inserted line, including
        // packets carrying an explicit column.
        self.has_changed_column = false;
        Ok(column)
    }

    fn packet_finished(&mut self, rules: GarbageMessinessRules) -> Result<(), GarbageError> {
        if self
            .rng
            .chance(rules.change_numerator, rules.change_denominator)
        {
            self.reroll(rules)?;
            self.has_changed_column = true;
        }
        Ok(())
    }

    fn reroll(&mut self, rules: GarbageMessinessRules) -> Result<u8, GarbageError> {
        rules.validate()?;
        let edge_exclusion = usize::from(rules.center) * ((WIDTH + 2) / 5);
        let available = WIDTH - 2 * edge_exclusion;
        let mut column = if rules.no_same && self.last_column.is_some() {
            let mut candidate = edge_exclusion + self.rng.index(available - 1);
            if candidate >= usize::from(self.last_column.expect("checked above")) {
                candidate += 1;
            }
            candidate
        } else {
            edge_exclusion + self.rng.index(available)
        };
        // The no-same branch skips the previous absolute column. With centered
        // ranges the previous value is guaranteed to be inside that range.
        column = column.min(WIDTH - 1);
        let column = u8::try_from(column).map_err(|_| GarbageError::CounterOverflow)?;
        self.last_column = Some(column);
        Ok(column)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncomingGarbagePacket {
    pub lines: u32,
    /// Some packets (maps/tests/custom modes) carry a fixed column. Normal TL
    /// attacks use `None` and consume the receiver's hole RNG on tank/cancel.
    pub hole_column: Option<u8>,
    pub ready_at_frame: u64,
    /// Hardened lines stay in queue and are skipped during cancellation.
    pub hardened: bool,
}

impl IncomingGarbagePacket {
    pub fn after_travel(
        lines: u32,
        hole_column: u8,
        sent_at_frame: u64,
        hardened: bool,
        rules: GarbageRules,
    ) -> Result<Self, GarbageError> {
        rules.validate()?;
        if lines == 0 {
            return Err(GarbageError::ZeroLinePacket);
        }
        if usize::from(hole_column) >= WIDTH {
            return Err(GarbageError::HoleOutOfBounds(hole_column));
        }
        let ready_at_frame = sent_at_frame
            .checked_add(u64::from(rules.travel_frames))
            .ok_or(GarbageError::CounterOverflow)?;
        Ok(Self {
            lines,
            hole_column: Some(hole_column),
            ready_at_frame,
            hardened,
        })
    }

    pub fn after_travel_generated(
        lines: u32,
        sent_at_frame: u64,
        hardened: bool,
        rules: GarbageRules,
    ) -> Result<Self, GarbageError> {
        rules.validate()?;
        if lines == 0 {
            return Err(GarbageError::ZeroLinePacket);
        }
        let ready_at_frame = sent_at_frame
            .checked_add(u64::from(rules.travel_frames))
            .ok_or(GarbageError::CounterOverflow)?;
        Ok(Self {
            lines,
            hole_column: None,
            ready_at_frame,
            hardened,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncomingGarbageQueue {
    packets: VecDeque<IncomingGarbagePacket>,
    holes: GarbageHoleGenerator,
}

impl IncomingGarbageQueue {
    pub fn new(seed: u64) -> Self {
        Self {
            packets: VecDeque::new(),
            holes: GarbageHoleGenerator::new(seed),
        }
    }

    pub fn enqueue(&mut self, packet: IncomingGarbagePacket) -> Result<(), GarbageError> {
        if packet.lines == 0 {
            return Err(GarbageError::ZeroLinePacket);
        }
        if let Some(column) = packet.hole_column
            && usize::from(column) >= WIDTH
        {
            return Err(GarbageError::HoleOutOfBounds(column));
        }
        self.packets.push_back(packet);
        Ok(())
    }

    pub fn as_slices(&self) -> (&[IncomingGarbagePacket], &[IncomingGarbagePacket]) {
        self.packets.as_slices()
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub const fn hole_generator(&self) -> &GarbageHoleGenerator {
        &self.holes
    }

    pub fn pending_lines(&self) -> u64 {
        self.packets
            .iter()
            .map(|packet| u64::from(packet.lines))
            .sum()
    }

    pub fn ready_lines(&self, frame: u64) -> u64 {
        self.packets
            .iter()
            .filter(|packet| packet.ready_at_frame <= frame)
            .map(|packet| u64::from(packet.lines))
            .sum()
    }

    fn cancel(&mut self, amount: u32, rules: GarbageRules) -> Result<u32, GarbageError> {
        let mut remaining = amount;
        for packet in &mut self.packets {
            if remaining == 0 {
                break;
            }
            if packet.hardened {
                continue;
            }
            let cancelled = packet.lines.min(remaining);
            packet.lines -= cancelled;
            remaining -= cancelled;
            if packet.lines == 0 {
                self.holes.packet_finished(rules.messiness)?;
            }
        }
        self.packets.retain(|packet| packet.lines > 0);
        Ok(amount - remaining)
    }

    fn take_ready_line(
        &mut self,
        frame: u64,
        rules: GarbageRules,
    ) -> Result<Option<u8>, GarbageError> {
        let index = self
            .packets
            .iter()
            .position(|packet| packet.ready_at_frame <= frame);
        let Some(index) = index else {
            return Ok(None);
        };
        let packet = self
            .packets
            .get_mut(index)
            .ok_or(GarbageError::CounterOverflow)?;
        let hole_column = self
            .holes
            .line_column(packet.hole_column, rules.messiness)?;
        packet.lines -= 1;
        if packet.lines == 0 {
            self.packets.remove(index);
            self.holes.packet_finished(rules.messiness)?;
        }
        Ok(Some(hole_column))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageCancellationOutcome {
    pub outgoing: AttackPackets,
    pub attack_cancelled: u32,
    pub opener_bonus_cancelled: u32,
    pub sent_lines_after: u64,
}

/// Cancels incoming garbage in the same packet order used by the attack
/// resolver. The attack portion is consumed before opener-only cancellation,
/// so only the uncancelled attack remainder can be sent to the opponent.
pub fn cancel_attack_packets(
    incoming: &mut IncomingGarbageQueue,
    packets: AttackPackets,
    pieces_placed: u64,
    sent_lines_before: u64,
    rules: GarbageRules,
) -> Result<GarbageCancellationOutcome, GarbageError> {
    rules.validate()?;
    let mut outgoing = AttackPackets::empty();
    let mut attack_cancelled = 0_u32;
    let mut opener_bonus_cancelled = 0_u32;
    let mut sent_lines_after = sent_lines_before;

    for packet in packets.as_slice() {
        let opener_bonus = pieces_placed <= rules.opener_phase_pieces
            && incoming.pending_lines() >= sent_lines_after;
        let cancelled = incoming.cancel(packet.lines, rules)?;
        attack_cancelled = attack_cancelled
            .checked_add(cancelled)
            .ok_or(GarbageError::CounterOverflow)?;
        let remaining = packet.lines - cancelled;

        if opener_bonus {
            let bonus_cancelled = incoming.cancel(packet.lines, rules)?;
            opener_bonus_cancelled = opener_bonus_cancelled
                .checked_add(bonus_cancelled)
                .ok_or(GarbageError::CounterOverflow)?;
        }

        outgoing.push(packet.kind, remaining)?;
        sent_lines_after = sent_lines_after
            .checked_add(u64::from(remaining))
            .ok_or(GarbageError::CounterOverflow)?;
    }

    Ok(GarbageCancellationOutcome {
        outgoing,
        attack_cancelled,
        opener_bonus_cancelled,
        sent_lines_after,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageInsertionOutcome {
    pub inserted: u8,
    pub overflowed_buffer: bool,
    pub blocked_by_clear: bool,
}

/// Raises ready garbage after a placement. Under combo blocking, every line
/// clear blocks insertion; a non-clear placement may take up to the rule cap.
pub fn insert_ready_garbage(
    incoming: &mut IncomingGarbageQueue,
    board: &mut Board,
    frame: u64,
    cleared_lines: u8,
    rules: GarbageRules,
) -> Result<GarbageInsertionOutcome, GarbageError> {
    rules.validate()?;
    if rules.combo_blocking && cleared_lines > 0 {
        return Ok(GarbageInsertionOutcome {
            inserted: 0,
            overflowed_buffer: false,
            blocked_by_clear: true,
        });
    }

    let mut inserted = 0_u8;
    let mut overflowed_buffer = false;
    while inserted < rules.garbage_cap {
        let Some(hole_column) = incoming.take_ready_line(frame, rules)? else {
            break;
        };
        let pushed = board.push_garbage_line(usize::from(hole_column))?;
        overflowed_buffer |= pushed.overflowed_buffer;
        inserted += 1;
    }

    Ok(GarbageInsertionOutcome {
        inserted,
        overflowed_buffer,
        blocked_by_clear: false,
    })
}

pub fn resolve_attack(
    previous: AttackState,
    clear: ClearEvent,
    context: AttackContext,
    rules: AttackRules,
) -> Result<AttackOutcome, AttackError> {
    rules.validate()?;
    if usize::from(clear.lines) >= rules.normal_clear.len() {
        return Err(AttackError::UnsupportedClearLines(clear.lines));
    }
    if clear.perfect_clear && clear.lines == 0 {
        return Err(AttackError::PerfectClearWithoutLineClear);
    }

    let cleared = clear.lines > 0;
    let difficult = cleared && (clear.lines >= 4 || clear.spin.is_some());
    let combo = if cleared {
        previous
            .combo
            .checked_add(1)
            .ok_or(AttackError::CounterOverflow)?
    } else {
        0
    };

    let mut back_to_back_increment = u32::from(difficult);
    if clear.perfect_clear {
        back_to_back_increment = back_to_back_increment
            .checked_add(rules.perfect_clear_back_to_back)
            .ok_or(AttackError::CounterOverflow)?;
        if !rules.perfect_clear_back_to_back_dupes {
            back_to_back_increment = rules
                .perfect_clear_back_to_back
                .max(back_to_back_increment.saturating_sub(rules.perfect_clear_back_to_back));
        }
        if rules.perfect_clear_charges {
            let charge = rules
                .back_to_back_charge_at
                .saturating_add(1)
                .saturating_sub(previous.back_to_back);
            back_to_back_increment = back_to_back_increment.max(charge);
        }
    }

    let next_back_to_back = if back_to_back_increment > 0 {
        previous
            .back_to_back
            .checked_add(back_to_back_increment)
            .ok_or(AttackError::CounterOverflow)?
    } else if cleared {
        0
    } else {
        previous.back_to_back
    };
    let back_to_back = back_to_back_increment > 0 && next_back_to_back > 1;

    let raw_surge_attack = if cleared
        && back_to_back_increment == 0
        && rules.back_to_back_charging
        && previous.back_to_back > rules.back_to_back_charge_at
    {
        previous
            .back_to_back
            .checked_sub(rules.back_to_back_charge_at)
            .and_then(|value| value.checked_add(rules.back_to_back_charge_base))
            .ok_or(AttackError::CounterOverflow)?
    } else {
        0
    };
    let surge_attack = context.multiplier.scale_floor(raw_surge_attack)?;

    let mut packets = AttackPackets::empty();
    push_surge_packets(&mut packets, surge_attack)?;

    let base_attack = base_attack(clear, rules)?;
    let sends_back_to_back = rules.perfect_clear_back_to_back_sends
        || !(clear.perfect_clear && back_to_back_increment == rules.perfect_clear_back_to_back);
    let back_to_back_bonus = if back_to_back && sends_back_to_back {
        u32::from(rules.back_to_back_bonus)
    } else {
        0
    };
    let attack_before_combo = base_attack
        .checked_add(back_to_back_bonus)
        .ok_or(AttackError::CounterOverflow)?;
    let combo_attack = apply_combo_and_multiplier_floor(
        attack_before_combo,
        combo,
        rules.combo,
        context.multiplier,
    )?;
    let special_bonus = if context.cleared_garbage && difficult {
        u32::from(rules.garbage_clear_special_bonus)
    } else {
        0
    };
    let clear_attack = combo_attack
        .checked_add(special_bonus)
        .ok_or(AttackError::CounterOverflow)?;
    packets.push(AttackPacketKind::Clear, clear_attack)?;

    let perfect_clear_attack = if clear.perfect_clear {
        context
            .multiplier
            .scale_floor(u32::from(rules.perfect_clear_attack))?
    } else {
        0
    };
    packets.push(AttackPacketKind::PerfectClear, perfect_clear_attack)?;

    Ok(AttackOutcome {
        state: AttackState {
            combo,
            back_to_back: next_back_to_back,
        },
        packets,
        base_attack,
        clear_attack,
        back_to_back_bonus,
        special_bonus,
        surge_attack,
        perfect_clear_attack,
        difficult,
        back_to_back,
    })
}

fn base_attack(clear: ClearEvent, rules: AttackRules) -> Result<u32, AttackError> {
    let index = usize::from(clear.lines);
    let table = match clear.spin.map(|spin| spin.classification) {
        None => &rules.normal_clear,
        Some(SpinClassification::Mini) => &rules.mini_spin,
        Some(SpinClassification::Full) => &rules.full_spin,
    };
    table
        .get(index)
        .copied()
        .map(u32::from)
        .ok_or(AttackError::UnsupportedClearLines(clear.lines))
}

fn apply_combo_and_multiplier_floor(
    attack: u32,
    combo: u32,
    rule: ComboRule,
    multiplier: AttackMultiplier,
) -> Result<u32, AttackError> {
    if combo <= 1 {
        return multiplier.scale_floor(attack);
    }
    match rule {
        ComboRule::None => multiplier.scale_floor(attack),
        ComboRule::Multiplier {
            increment_numerator,
            increment_denominator,
            zero_base_min_combo_index,
        } => {
            if increment_denominator == 0 {
                return Err(AttackConfigError::ZeroComboDenominator.into());
            }
            let combo_index = combo - 1;
            if attack == 0 {
                if multiplier == AttackMultiplier::one() {
                    return u32::try_from(
                        zero_base_min_combo_index
                            .iter()
                            .take_while(|threshold| combo_index >= **threshold)
                            .count(),
                    )
                    .map_err(|_| AttackError::CounterOverflow);
                }
                // The zero-base minifier is logarithmic in the client. Once
                // the time-varying multiplier is not one, integer thresholds
                // alone lose the fractional part needed before final floor.
                let value = (1.25_f64 * f64::from(combo_index)).ln_1p() * multiplier.value();
                return floor_js_attack(value);
            }

            let factor = 1.0
                + (f64::from(increment_numerator) / f64::from(increment_denominator))
                    * f64::from(combo_index);
            let combo_scaled = f64::from(attack) * factor;
            floor_js_attack(combo_scaled * multiplier.value())
        }
    }
}

fn floor_js_attack(value: f64) -> Result<u32, AttackError> {
    if !value.is_finite() || value < 0.0 {
        return Err(AttackError::CounterOverflow);
    }
    let floored = value.floor();
    if floored > f64::from(u32::MAX) {
        return Err(AttackError::CounterOverflow);
    }
    Ok(floored as u32)
}

fn push_surge_packets(packets: &mut AttackPackets, surge_attack: u32) -> Result<(), AttackError> {
    if surge_attack == 0 {
        return Ok(());
    }
    // Positive JavaScript Math.round(surge / 3) is exactly (surge + 1) / 3.
    let rounded_third = surge_attack
        .checked_add(1)
        .ok_or(AttackError::CounterOverflow)?
        / 3;
    let final_packet = surge_attack
        .checked_sub(
            rounded_third
                .checked_mul(2)
                .ok_or(AttackError::CounterOverflow)?,
        )
        .ok_or(AttackError::CounterOverflow)?;
    packets.push(AttackPacketKind::Surge, rounded_third)?;
    packets.push(AttackPacketKind::Surge, rounded_third)?;
    packets.push(AttackPacketKind::Surge, final_packet)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttackConfigError {
    ZeroComboDenominator,
    NonIncreasingComboThresholds,
    ZeroGarbageMultiplierDenominator,
    NonFiniteGarbageMultiplier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarbageConfigError {
    ZeroGarbageCap,
    ZeroMessinessChangeDenominator,
    ZeroMessinessInnerDenominator,
    NoAvailableGarbageHole,
    ZeroInitialMultiplierDenominator,
    ZeroMultiplierIncreaseDenominator,
    ZeroTickRate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttackError {
    InvalidConfiguration(AttackConfigError),
    UnsupportedClearLines(u8),
    PerfectClearWithoutLineClear,
    CounterOverflow,
    PacketCapacityExceeded,
}

impl From<AttackConfigError> for AttackError {
    fn from(error: AttackConfigError) -> Self {
        Self::InvalidConfiguration(error)
    }
}

impl fmt::Display for AttackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(error) => {
                write!(formatter, "invalid attack configuration: {error:?}")
            }
            Self::UnsupportedClearLines(lines) => {
                write!(
                    formatter,
                    "attack table does not support {lines} cleared lines"
                )
            }
            Self::PerfectClearWithoutLineClear => {
                write!(
                    formatter,
                    "perfect clear requires at least one cleared line"
                )
            }
            Self::CounterOverflow => write!(formatter, "attack-state counter overflow"),
            Self::PacketCapacityExceeded => write!(formatter, "attack packet capacity exceeded"),
        }
    }
}

impl std::error::Error for AttackError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarbageError {
    InvalidConfiguration(GarbageConfigError),
    Attack(AttackError),
    Board(BoardError),
    ZeroLinePacket,
    HoleOutOfBounds(u8),
    MissingGeneratedHole,
    CounterOverflow,
}

impl From<GarbageConfigError> for GarbageError {
    fn from(error: GarbageConfigError) -> Self {
        Self::InvalidConfiguration(error)
    }
}

impl From<AttackError> for GarbageError {
    fn from(error: AttackError) -> Self {
        Self::Attack(error)
    }
}

impl From<BoardError> for GarbageError {
    fn from(error: BoardError) -> Self {
        Self::Board(error)
    }
}

impl fmt::Display for GarbageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(error) => {
                write!(formatter, "invalid garbage configuration: {error:?}")
            }
            Self::Attack(error) => write!(formatter, "attack packet error: {error}"),
            Self::Board(error) => write!(formatter, "board insertion error: {error}"),
            Self::ZeroLinePacket => write!(formatter, "incoming garbage packet has zero lines"),
            Self::HoleOutOfBounds(column) => {
                write!(
                    formatter,
                    "incoming garbage hole {column} is outside the board"
                )
            }
            Self::MissingGeneratedHole => {
                write!(formatter, "garbage RNG did not produce a hole column")
            }
            Self::CounterOverflow => write!(formatter, "garbage-state counter overflow"),
        }
    }
}

impl std::error::Error for GarbageError {}

#[cfg(test)]
mod tests {
    use super::{
        AttackContext, AttackError, AttackMultiplier, AttackPacket, AttackPacketKind,
        AttackPackets, AttackRules, AttackState, ComboRule, GarbageMessinessRules,
        GarbageMultiplierSchedule, GarbageMultiplierState, GarbageRules, IncomingGarbagePacket,
        IncomingGarbageQueue, cancel_attack_packets, insert_ready_garbage, resolve_attack,
    };
    use engine_core::{
        Board, ClearEvent, PieceKind, RotationDirection, SpinClassification, SpinOutcome,
    };

    const ZERO_BASE_THRESHOLDS: &[u32] = &[
        2,
        6,
        16,
        43,
        118,
        322,
        877,
        2_384,
        6_482,
        17_621,
        47_899,
        130_204,
        353_930,
        962_083,
        2_615_214,
        7_108_888,
        19_323_962,
        52_527_975,
        142_785_840,
        388_132_156,
        1_055_052_587,
        2_867_930_277,
    ];

    fn rules() -> AttackRules {
        AttackRules {
            normal_clear: [0, 0, 1, 2, 4],
            mini_spin: [0, 0, 1, 2, 4],
            full_spin: [0, 2, 4, 6, 10],
            combo: ComboRule::Multiplier {
                increment_numerator: 1,
                increment_denominator: 4,
                zero_base_min_combo_index: ZERO_BASE_THRESHOLDS,
            },
            back_to_back_bonus: 1,
            back_to_back_charging: true,
            back_to_back_charge_at: 4,
            back_to_back_charge_base: 0,
            perfect_clear_attack: 5,
            perfect_clear_back_to_back: 1,
            perfect_clear_back_to_back_sends: false,
            perfect_clear_back_to_back_dupes: true,
            perfect_clear_charges: false,
            garbage_clear_special_bonus: 1,
        }
    }

    fn garbage_rules() -> GarbageRules {
        GarbageRules {
            travel_frames: 20,
            garbage_cap: 8,
            opener_phase_pieces: 14,
            combo_blocking: true,
            messiness: GarbageMessinessRules::tetra_league_observed(),
            multiplier: GarbageMultiplierSchedule::tetra_league_observed(),
        }
    }

    fn packets(values: &[(AttackPacketKind, u32)]) -> AttackPackets {
        let mut packets = AttackPackets::empty();
        for (kind, lines) in values {
            packets.push(*kind, *lines).expect("test packet capacity");
        }
        packets
    }

    fn incoming(lines: u32, hole_column: u8, sent_at_frame: u64) -> IncomingGarbagePacket {
        IncomingGarbagePacket::after_travel(
            lines,
            hole_column,
            sent_at_frame,
            false,
            garbage_rules(),
        )
        .expect("valid incoming packet")
    }

    fn clear(lines: u8) -> ClearEvent {
        ClearEvent::new(PieceKind::I, lines, None, false)
    }

    fn spin(lines: u8, classification: SpinClassification) -> ClearEvent {
        ClearEvent::new(
            PieceKind::T,
            lines,
            Some(SpinOutcome {
                piece: PieceKind::T,
                classification,
                direction: RotationDirection::Clockwise,
                kick_index: 0,
            }),
            false,
        )
    }

    #[test]
    fn clear_table_matches_observed_tl_base_attacks() {
        let cases = [(1, 0), (2, 1), (3, 2), (4, 4)];
        for (lines, expected) in cases {
            let outcome = resolve_attack(
                AttackState::default(),
                clear(lines),
                AttackContext::default(),
                rules(),
            )
            .expect("valid attack");
            assert_eq!(outcome.base_attack, expected);
        }

        let mini = resolve_attack(
            AttackState::default(),
            spin(2, SpinClassification::Mini),
            AttackContext::default(),
            rules(),
        )
        .expect("mini attack");
        let full = resolve_attack(
            AttackState::default(),
            spin(2, SpinClassification::Full),
            AttackContext::default(),
            rules(),
        )
        .expect("full attack");
        assert_eq!((mini.base_attack, full.base_attack), (1, 4));
    }

    #[test]
    fn fourteen_double_combo_matches_generated_snapshot() {
        let mut state = AttackState::default();
        let mut total = 0;
        let mut attacks = Vec::new();
        for _ in 0..14 {
            let outcome = resolve_attack(state, clear(2), AttackContext::default(), rules())
                .expect("combo attack");
            state = outcome.state;
            total += outcome.total_attack();
            attacks.push(outcome.total_attack());
        }

        assert_eq!(attacks, [1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4]);
        assert_eq!(total, 32);
        assert_eq!(state.combo, 14);
    }

    #[test]
    fn flat_b2b_t_spin_double_sequence_matches_generated_snapshot() {
        let mut state = AttackState::default();
        let mut attacks = Vec::new();
        for _ in 0..5 {
            let outcome = resolve_attack(
                state,
                spin(2, SpinClassification::Full),
                AttackContext::default(),
                rules(),
            )
            .expect("b2b attack");
            state = outcome.state;
            attacks.push(outcome.total_attack());
        }

        assert_eq!(attacks, [4, 6, 7, 8, 10]);
        assert_eq!(state.back_to_back, 5);
    }

    #[test]
    fn b2b_break_releases_surge_before_current_clear_packet() {
        let outcome = resolve_attack(
            AttackState {
                combo: 5,
                back_to_back: 5,
            },
            clear(1),
            AttackContext::default(),
            rules(),
        )
        .expect("surge release");

        assert_eq!(outcome.surge_attack, 1);
        assert_eq!(outcome.clear_attack, 1);
        assert_eq!(outcome.total_attack(), 2);
        assert_eq!(
            outcome.packets.as_slice(),
            [
                AttackPacket {
                    kind: AttackPacketKind::Surge,
                    lines: 1,
                },
                AttackPacket {
                    kind: AttackPacketKind::Clear,
                    lines: 1,
                },
            ]
        );
        assert_eq!(outcome.state.back_to_back, 0);
    }

    #[test]
    fn surge_uses_three_js_rounded_packets_and_omits_zero_packets() {
        let outcome = resolve_attack(
            AttackState {
                combo: 0,
                back_to_back: 9,
            },
            clear(1),
            AttackContext::default(),
            rules(),
        )
        .expect("five-line surge");

        assert_eq!(outcome.surge_attack, 5);
        assert_eq!(
            outcome.packets.as_slice(),
            [
                AttackPacket {
                    kind: AttackPacketKind::Surge,
                    lines: 2,
                },
                AttackPacket {
                    kind: AttackPacketKind::Surge,
                    lines: 2,
                },
                AttackPacket {
                    kind: AttackPacketKind::Surge,
                    lines: 1,
                },
            ]
        );
    }

    #[test]
    fn no_line_resets_combo_but_preserves_b2b() {
        let outcome = resolve_attack(
            AttackState {
                combo: 4,
                back_to_back: 3,
            },
            clear(0),
            AttackContext::default(),
            rules(),
        )
        .expect("no clear");

        assert_eq!(outcome.state.combo, 0);
        assert_eq!(outcome.state.back_to_back, 3);
        assert!(outcome.packets.is_empty());
    }

    #[test]
    fn perfect_clear_is_a_separate_packet_and_advances_b2b() {
        let mut event = spin(2, SpinClassification::Full);
        event.perfect_clear = true;
        let outcome = resolve_attack(
            AttackState::default(),
            event,
            AttackContext::default(),
            rules(),
        )
        .expect("perfect clear");

        assert_eq!(outcome.state.back_to_back, 2);
        assert_eq!(outcome.total_attack(), 10);
        assert_eq!(
            outcome.packets.as_slice(),
            [
                AttackPacket {
                    kind: AttackPacketKind::Clear,
                    lines: 5,
                },
                AttackPacket {
                    kind: AttackPacketKind::PerfectClear,
                    lines: 5,
                },
            ]
        );
    }

    #[test]
    fn difficult_garbage_clear_gets_post_rounding_special_bonus() {
        let outcome = resolve_attack(
            AttackState::default(),
            clear(4),
            AttackContext {
                cleared_garbage: true,
                ..AttackContext::default()
            },
            rules(),
        )
        .expect("garbage clear bonus");

        assert_eq!(outcome.base_attack, 4);
        assert_eq!(outcome.special_bonus, 1);
        assert_eq!(outcome.clear_attack, 5);
    }

    #[test]
    fn margin_multiplier_scales_each_client_packet_before_special_bonus() {
        let multiplier = AttackMultiplier::new(3, 2).expect("valid multiplier");
        let context = AttackContext {
            cleared_garbage: true,
            multiplier,
        };

        let difficult = resolve_attack(AttackState::default(), clear(4), context, rules())
            .expect("scaled difficult clear");
        assert_eq!(difficult.base_attack, 4);
        assert_eq!(difficult.special_bonus, 1);
        assert_eq!(difficult.clear_attack, 7);

        let mut all_clear = clear(4);
        all_clear.perfect_clear = true;
        let perfect = resolve_attack(AttackState::default(), all_clear, context, rules())
            .expect("scaled perfect clear");
        assert_eq!(perfect.clear_attack, 8);
        assert_eq!(perfect.perfect_clear_attack, 7);
    }

    #[test]
    fn margin_schedule_observes_end_of_frame_ieee_addition() {
        let schedule = GarbageMultiplierSchedule::tetra_league_observed();
        let mut state = GarbageMultiplierState::new(schedule).expect("valid schedule");
        for frame in 0..=10_800 {
            assert_eq!(state.current(), AttackMultiplier::one());
            state
                .advance_end_of_frame(frame, schedule)
                .expect("valid update");
        }

        state
            .advance_end_of_frame(10_801, schedule)
            .expect("first end-of-frame update");
        assert_eq!(
            state.current().value().to_bits(),
            (1.0_f64 + 0.008_f64 / 60.0).to_bits()
        );

        for frame in 10_802..=18_300 {
            state
                .advance_end_of_frame(frame, schedule)
                .expect("valid update");
        }
        assert_eq!(state.current().value_bits, 0x3fff_ffff_ffff_fe10);
        assert_eq!(state.current().value().floor(), 1.0);
        assert!(state.current().value() < 2.0);
    }

    #[test]
    fn trace_interchange_preserves_finite_multiplier_payload() {
        let bits = 0x3fff_ffff_ffff_fe10;
        let multiplier = AttackMultiplier::from_ieee_bits(bits).expect("finite payload");
        assert_eq!(multiplier.ieee_bits(), bits);
        assert!(AttackMultiplier::from_ieee_bits(f64::NAN.to_bits()).is_err());
    }

    #[test]
    fn trace_packet_constructor_preserves_order_and_capacity() {
        let source = [
            AttackPacket {
                kind: AttackPacketKind::Surge,
                lines: 2,
            },
            AttackPacket {
                kind: AttackPacketKind::Clear,
                lines: 3,
            },
        ];
        assert_eq!(
            AttackPackets::try_from_slice(&source)
                .expect("valid packets")
                .as_slice(),
            source
        );

        let too_many = [AttackPacket {
            kind: AttackPacketKind::Clear,
            lines: 1,
        }; 6];
        assert_eq!(
            AttackPackets::try_from_slice(&too_many),
            Err(AttackError::PacketCapacityExceeded)
        );
    }

    #[test]
    fn margin_attack_floor_matches_bun_ieee_fixture() {
        let multiplier = AttackMultiplier {
            value_bits: 0x3fff_ffff_ffff_fe10,
        };

        let nonzero = resolve_attack(
            AttackState {
                combo: 9,
                back_to_back: 0,
            },
            clear(2),
            AttackContext {
                cleared_garbage: false,
                multiplier,
            },
            rules(),
        )
        .expect("margin combo");
        assert_eq!(nonzero.clear_attack, 6);

        let zero_base = resolve_attack(
            AttackState {
                combo: 100,
                back_to_back: 0,
            },
            clear(1),
            AttackContext {
                cleared_garbage: false,
                multiplier,
            },
            rules(),
        )
        .expect("margin zero-base combo");
        assert_eq!(zero_base.clear_attack, 9);
    }

    #[test]
    fn late_zero_base_combo_keeps_fraction_until_final_floor() {
        let outcome = resolve_attack(
            AttackState {
                combo: 3,
                back_to_back: 0,
            },
            clear(1),
            AttackContext {
                cleared_garbage: false,
                multiplier: AttackMultiplier::new(3, 2).expect("valid multiplier"),
            },
            rules(),
        )
        .expect("late combo");

        // combo index 3: floor(ln(1 + 1.25*3) * 1.5) = 2.
        assert_eq!(outcome.state.combo, 4);
        assert_eq!(outcome.clear_attack, 2);
    }

    #[test]
    fn opener_cancellation_is_applied_per_ordered_attack_packet() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue.enqueue(incoming(10, 3, 0)).expect("valid queue");
        let attack = packets(&[(AttackPacketKind::Surge, 4), (AttackPacketKind::Clear, 4)]);

        let outcome = cancel_attack_packets(&mut queue, attack, 14, 0, garbage_rules())
            .expect("valid cancellation");

        assert_eq!(outcome.attack_cancelled, 6);
        assert_eq!(outcome.opener_bonus_cancelled, 4);
        assert_eq!(outcome.sent_lines_after, 2);
        assert_eq!(
            outcome.outgoing.as_slice(),
            [AttackPacket {
                kind: AttackPacketKind::Clear,
                lines: 2,
            }]
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn opener_condition_uses_round_sent_total_and_piece_boundary() {
        let mut active = IncomingGarbageQueue::new(0);
        active.enqueue(incoming(4, 2, 0)).expect("valid queue");
        let attack = packets(&[(AttackPacketKind::Clear, 2)]);

        let at_boundary = cancel_attack_packets(&mut active, attack, 14, 4, garbage_rules())
            .expect("opener boundary cancellation");
        assert_eq!(at_boundary.attack_cancelled, 2);
        assert_eq!(at_boundary.opener_bonus_cancelled, 2);
        assert_eq!(at_boundary.sent_lines_after, 4);

        let mut expired = IncomingGarbageQueue::new(0);
        expired.enqueue(incoming(4, 2, 0)).expect("valid queue");
        let after_boundary = cancel_attack_packets(&mut expired, attack, 15, 4, garbage_rules())
            .expect("post-opener cancellation");
        assert_eq!(after_boundary.attack_cancelled, 2);
        assert_eq!(after_boundary.opener_bonus_cancelled, 0);
        assert_eq!(expired.pending_lines(), 2);
    }

    #[test]
    fn cancellation_conserves_attack_pending_and_sent_lines() {
        for pieces_placed in [14, 15] {
            for pending in 1..=12 {
                for first in 1..=5 {
                    for second in 1..=5 {
                        for sent_before in [0, 3, 9] {
                            let mut queue = IncomingGarbageQueue::new(0);
                            queue.enqueue(incoming(pending, 4, 0)).expect("valid queue");
                            let attack = packets(&[
                                (AttackPacketKind::Surge, first),
                                (AttackPacketKind::Clear, second),
                            ]);
                            let pending_before = queue.pending_lines();
                            let raw_attack = attack.total();

                            let outcome = cancel_attack_packets(
                                &mut queue,
                                attack,
                                pieces_placed,
                                sent_before,
                                garbage_rules(),
                            )
                            .expect("valid conservation case");

                            assert_eq!(
                                raw_attack,
                                u64::from(outcome.attack_cancelled) + outcome.outgoing.total()
                            );
                            assert_eq!(
                                pending_before - queue.pending_lines(),
                                u64::from(
                                    outcome.attack_cancelled + outcome.opener_bonus_cancelled
                                )
                            );
                            assert_eq!(
                                outcome.sent_lines_after - sent_before,
                                outcome.outgoing.total()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn zero_passthrough_cancels_packets_before_travel_finishes() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue
            .enqueue(incoming(3, 1, 100))
            .expect("in-transit packet");

        let outcome = cancel_attack_packets(
            &mut queue,
            packets(&[(AttackPacketKind::Clear, 2)]),
            20,
            0,
            garbage_rules(),
        )
        .expect("in-transit cancellation");

        assert_eq!(outcome.attack_cancelled, 2);
        assert_eq!(queue.pending_lines(), 1);
        assert_eq!(queue.ready_lines(119), 0);
    }

    #[test]
    fn hardened_packets_are_skipped_without_blocking_later_cancellation() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue
            .enqueue(IncomingGarbagePacket {
                hardened: true,
                ..incoming(3, 0, 0)
            })
            .expect("hardened packet");
        queue.enqueue(incoming(3, 9, 0)).expect("normal packet");

        let outcome = cancel_attack_packets(
            &mut queue,
            packets(&[(AttackPacketKind::Clear, 2)]),
            20,
            0,
            garbage_rules(),
        )
        .expect("skip hardened packet");

        assert_eq!(outcome.attack_cancelled, 2);
        assert_eq!(queue.pending_lines(), 4);
        let (front, back) = queue.as_slices();
        let queued: Vec<_> = front.iter().chain(back).copied().collect();
        assert_eq!(queued[0].lines, 3);
        assert!(queued[0].hardened);
        assert_eq!(queued[1].lines, 1);
    }

    #[test]
    fn travel_combo_blocking_and_cap_gate_board_insertion() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue.enqueue(incoming(10, 3, 0)).expect("valid queue");
        let mut board = Board::empty();

        let before_arrival = insert_ready_garbage(&mut queue, &mut board, 19, 0, garbage_rules())
            .expect("pre-arrival insertion");
        assert_eq!(before_arrival.inserted, 0);

        let blocked = insert_ready_garbage(&mut queue, &mut board, 20, 1, garbage_rules())
            .expect("combo-blocked insertion");
        assert!(blocked.blocked_by_clear);
        assert_eq!(queue.pending_lines(), 10);

        let inserted = insert_ready_garbage(&mut queue, &mut board, 20, 0, garbage_rules())
            .expect("capped insertion");
        assert_eq!(inserted.inserted, 8);
        assert!(!inserted.overflowed_buffer);
        assert_eq!(queue.pending_lines(), 2);
        let expected = ((1_u16 << 10) - 1) & !(1 << 3);
        for row in 0..8 {
            assert_eq!(board.row(row), Some(expected));
            assert_eq!(board.garbage_rows()[row], expected);
        }
    }

    #[test]
    fn ready_packets_insert_in_queue_order() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue.enqueue(incoming(1, 1, 0)).expect("first packet");
        queue.enqueue(incoming(1, 8, 0)).expect("second packet");
        let mut board = Board::empty();

        let outcome = insert_ready_garbage(&mut queue, &mut board, 20, 0, garbage_rules())
            .expect("ordered insertion");

        let full = (1_u16 << 10) - 1;
        assert_eq!(outcome.inserted, 2);
        assert_eq!(board.row(0), Some(full & !(1 << 8)));
        assert_eq!(board.row(1), Some(full & !(1 << 1)));
        assert!(queue.is_empty());
    }

    #[test]
    fn generated_packet_matches_client_hole_and_rng_consumption() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue
            .enqueue(
                IncomingGarbagePacket::after_travel_generated(3, 0, false, garbage_rules())
                    .expect("generated packet"),
            )
            .expect("valid queue");
        let mut board = Board::empty();

        let outcome = insert_ready_garbage(&mut queue, &mut board, 20, 0, garbage_rules())
            .expect("generated insertion");

        assert_eq!(outcome.inserted, 3);
        let hole_nine = ((1_u16 << 10) - 1) & !(1 << 9);
        assert_eq!(&board.rows()[..3], &[hole_nine; 3]);
        // draw 1 chooses the first hole; draws 2-3 perform the unconditional
        // inner=0 checks; draw 4 checks change=1; draw 5 picks the next hole.
        assert_eq!(queue.hole_generator().rng_state(), 1_003_374_717);
        assert_eq!(queue.hole_generator().last_column(), Some(4));
        assert!(queue.hole_generator().has_changed_column());
    }

    #[test]
    fn complete_cancellation_consumes_packet_boundary_rng() {
        let mut queue = IncomingGarbageQueue::new(0);
        queue
            .enqueue(
                IncomingGarbagePacket::after_travel_generated(3, 0, false, garbage_rules())
                    .expect("generated packet"),
            )
            .expect("valid queue");

        let outcome = cancel_attack_packets(
            &mut queue,
            packets(&[(AttackPacketKind::Clear, 3)]),
            15,
            0,
            garbage_rules(),
        )
        .expect("full cancellation");

        assert_eq!(outcome.attack_cancelled, 3);
        assert!(queue.is_empty());
        // Even without a rise, packet depletion consumes the change test and
        // the next-hole selection exactly as current FightLines does.
        assert_eq!(queue.hole_generator().rng_state(), 1_865_008_398);
        assert_eq!(queue.hole_generator().last_column(), Some(8));
        assert!(queue.hole_generator().has_changed_column());
    }
}
