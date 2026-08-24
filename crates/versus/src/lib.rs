//! Deterministic, score-free 1v1 attack transitions.
//!
//! `engine-core` reports a [`ClearEvent`]. This crate applies an explicit rules
//! profile and emits ordered garbage attack packets. It deliberately contains
//! no 40 LINES or BLITZ score accounting.

#![forbid(unsafe_code)]

use engine_core::{ClearEvent, SpinClassification};
use std::fmt;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AttackContext {
    /// True when at least one cleared row contained garbage cells.
    pub cleared_garbage: bool,
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

    let surge_attack = if cleared
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
    let combo_attack = apply_combo(attack_before_combo, combo, rules.combo)?;
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
        u32::from(rules.perfect_clear_attack)
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

fn apply_combo(attack: u32, combo: u32, rule: ComboRule) -> Result<u32, AttackError> {
    if combo <= 1 {
        return Ok(attack);
    }
    match rule {
        ComboRule::None => Ok(attack),
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
                return u32::try_from(
                    zero_base_min_combo_index
                        .iter()
                        .take_while(|threshold| combo_index >= **threshold)
                        .count(),
                )
                .map_err(|_| AttackError::CounterOverflow);
            }

            let factor = u64::from(increment_denominator)
                .checked_add(
                    u64::from(increment_numerator)
                        .checked_mul(u64::from(combo_index))
                        .ok_or(AttackError::CounterOverflow)?,
                )
                .ok_or(AttackError::CounterOverflow)?;
            let scaled = u64::from(attack)
                .checked_mul(factor)
                .ok_or(AttackError::CounterOverflow)?
                / u64::from(increment_denominator);
            u32::try_from(scaled).map_err(|_| AttackError::CounterOverflow)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        AttackContext, AttackPacket, AttackPacketKind, AttackRules, AttackState, ComboRule,
        resolve_attack,
    };
    use engine_core::{ClearEvent, PieceKind, RotationDirection, SpinClassification, SpinOutcome};

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
            },
            rules(),
        )
        .expect("garbage clear bonus");

        assert_eq!(outcome.base_attack, 4);
        assert_eq!(outcome.special_bonus, 1);
        assert_eq!(outcome.clear_attack, 5);
    }
}
