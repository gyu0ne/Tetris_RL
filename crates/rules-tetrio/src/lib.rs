//! Versioned TETR.IO mechanics profiles with field-level provenance.
//!
//! Public-client observations may make a profile executable for local research,
//! but only reference replay fixtures can make it conformance-ready. Those two
//! states are deliberately represented separately.

#![forbid(unsafe_code)]

use engine_core::{
    GameConfig, Gravity, HandlingRules, LinearGravityTiming, SoftDropMode, SpinRules,
    TimingConfigError, TimingRules, TimingSchedule, TimingScheduleError,
};
use versus::{
    AttackConfigError, AttackRules, BattleRules, ComboRule, GarbageConfigError,
    GarbageMessinessRules, GarbageMultiplierSchedule, GarbageRules,
};

pub const TARGET_PROFILE_ID: &str = "tetrio-beta-1.7.8-tetra-league-season-2";
pub const RESEARCH_ACCESS_DATE: &str = "2026-08-24";
pub const PLAYER_HANDLING_SCHEMA_VERSION: u16 = 1;

const OFFICIAL_PATCH_NOTES: &str = "https://tetr.io/about/patchnotes/";
const WIKI_MECHANICS: &str = "https://tetrio.wiki.gg/wiki/Mechanics";
const CURRENT_CLIENT_ASSET: &str =
    "https://tetr.io/js/tetrio.js?hv=63ab5c7c7.efa161fa8f91.20260810T191705";
const CLIENT_OPTIONS_FIXTURE: &str = "client-options-hv-63ab5c7c7-20260824";
const CLIENT_FIREPOWER_FIXTURE: &str = "client-firepower-hv-63ab5c7c7-20260824";
const CLIENT_GARBAGE_FIXTURE: &str = "client-garbage-hv-63ab5c7c7-20260824";

static MULTIPLIER_ZERO_BASE_THRESHOLDS: [u32; 22] = [
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Confidence {
    Confirmed,
    Observed,
    Unconfirmed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Evidence {
    pub source_url: &'static str,
    pub accessed_on: &'static str,
    pub confidence: Confidence,
    pub fixture_id: Option<&'static str>,
    pub note: &'static str,
}

impl Evidence {
    pub const fn new(
        source_url: &'static str,
        confidence: Confidence,
        fixture_id: Option<&'static str>,
        note: &'static str,
    ) -> Self {
        Self {
            source_url,
            accessed_on: RESEARCH_ACCESS_DATE,
            confidence,
            fixture_id,
            note,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sourced<T> {
    pub value: Option<T>,
    pub unit: &'static str,
    pub evidence: Evidence,
}

impl<T> Sourced<T> {
    pub const fn new(value: Option<T>, unit: &'static str, evidence: Evidence) -> Self {
        Self {
            value,
            unit,
            evidence,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RotationSystem {
    SrsPlus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpinSystem {
    AllMiniPlus,
}

impl SpinSystem {
    pub const fn core_rules(self) -> SpinRules {
        match self {
            Self::AllMiniPlus => SpinRules::all_mini_plus_observed(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingProfileDraft {
    pub tick_rate_hz: Sourced<u16>,
    pub are_frames: Sourced<u16>,
    pub line_clear_are_frames: Sourced<u16>,
    pub gravity_numerator: Sourced<u32>,
    pub gravity_denominator: Sourced<u32>,
    pub gravity_increase_numerator_per_second: Sourced<u32>,
    pub gravity_increase_denominator_per_second: Sourced<u32>,
    pub gravity_margin_frames: Sourced<u32>,
    pub gravity_cap_numerator: Sourced<u32>,
    pub gravity_cap_denominator: Sourced<u32>,
    pub lock_delay_frames: Sourced<u16>,
    pub max_lock_resets: Sourced<u16>,
    pub reset_on_lateral_move: Sourced<bool>,
    pub reset_on_rotation: Sourced<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttackProfileDraft {
    pub normal_clear: Sourced<[u16; 5]>,
    pub mini_spin: Sourced<[u16; 5]>,
    pub full_spin: Sourced<[u16; 5]>,
    pub combo: Sourced<ComboRule>,
    pub back_to_back_bonus: Sourced<u16>,
    pub back_to_back_charging: Sourced<bool>,
    pub back_to_back_charge_at: Sourced<u32>,
    pub back_to_back_charge_base: Sourced<u32>,
    pub perfect_clear_attack: Sourced<u16>,
    pub perfect_clear_back_to_back: Sourced<u32>,
    pub perfect_clear_back_to_back_sends: Sourced<bool>,
    pub perfect_clear_back_to_back_dupes: Sourced<bool>,
    pub perfect_clear_charges: Sourced<bool>,
    pub garbage_clear_special_bonus: Sourced<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GarbageProfileDraft {
    pub travel_frames: Sourced<u32>,
    pub garbage_cap: Sourced<u8>,
    pub opener_phase_pieces: Sourced<u64>,
    pub combo_blocking: Sourced<bool>,
    pub multiplier: Sourced<GarbageMultiplierSchedule>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoomHandlingProfileDraft {
    pub enforced: Sourced<bool>,
    pub inactive_arr_frames: Sourced<u16>,
    pub inactive_das_frames: Sourced<u16>,
    pub inactive_sdf_value: Sourced<u16>,
}

impl RoomHandlingProfileDraft {
    pub const fn requires_player_handling(self) -> bool {
        matches!(self.enforced.value, Some(false))
    }
}

/// Replay/config-derived effective handling after unit conversion to frames.
///
/// TETRA LEAGUE does not enforce room handling in the observed target profile,
/// so this record must accompany every replay fixture. Raw UI/client values are
/// converted by a versioned adapter before constructing this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerHandlingProfile {
    pub schema_version: u16,
    pub das_frames: u16,
    pub arr_frames: u16,
    pub dcd_frames: u16,
    pub soft_drop: SoftDropMode,
}

impl PlayerHandlingProfile {
    pub const fn normalized(
        das_frames: u16,
        arr_frames: u16,
        dcd_frames: u16,
        soft_drop: SoftDropMode,
    ) -> Self {
        Self {
            schema_version: PLAYER_HANDLING_SCHEMA_VERSION,
            das_frames,
            arr_frames,
            dcd_frames,
            soft_drop,
        }
    }

    pub const fn core_rules(self) -> HandlingRules {
        HandlingRules::new(
            self.das_frames,
            self.arr_frames,
            self.dcd_frames,
            self.soft_drop,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TetrioRulesDraft {
    pub profile_id: &'static str,
    pub upstream_version: &'static str,
    pub mode: &'static str,
    pub version_evidence: Evidence,
    pub board_width: Sourced<u8>,
    pub board_height: Sourced<u8>,
    pub rotation_system: Sourced<RotationSystem>,
    pub spin_system: Sourced<SpinSystem>,
    pub timing: TimingProfileDraft,
    pub attack: AttackProfileDraft,
    pub garbage: GarbageProfileDraft,
    pub room_handling: RoomHandlingProfileDraft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveTimingProfile {
    pub tick_rate_hz: u16,
    pub are_frames: u16,
    pub line_clear_are_frames: u16,
    pub initial_gravity: Gravity,
    pub gravity_increase_per_second: Gravity,
    pub gravity_margin_frames: u32,
    pub gravity_cap: Gravity,
    pub lock_delay_frames: u16,
    pub max_lock_resets: u16,
    pub reset_on_lateral_move: bool,
    pub reset_on_rotation: bool,
}

impl ActiveTimingProfile {
    pub fn core_schedule(self) -> Result<TimingSchedule, ProfileActivationError> {
        let base_rules = TimingRules::new(
            self.initial_gravity,
            self.lock_delay_frames,
            self.max_lock_resets,
            self.reset_on_lateral_move,
            self.reset_on_rotation,
        );
        LinearGravityTiming::new(
            base_rules,
            self.gravity_increase_per_second,
            u64::from(self.gravity_margin_frames),
            u32::from(self.tick_rate_hz),
            self.gravity_cap,
        )
        .map(TimingSchedule::linear_gravity)
        .map_err(map_timing_schedule_error)
    }

    pub fn core_rules_at_frame(
        self,
        elapsed_frames: u64,
    ) -> Result<TimingRules, ProfileActivationError> {
        self.core_schedule()?
            .rules_at_frame(elapsed_frames)
            .map_err(map_timing_schedule_error)
    }

    pub fn gravity_at_frame(self, elapsed_frames: u64) -> Result<Gravity, ProfileActivationError> {
        Ok(self.core_rules_at_frame(elapsed_frames)?.gravity)
    }
}

impl TetrioRulesDraft {
    pub const fn tetra_league_beta_1_7_8_season_2() -> Self {
        let mechanics_observed = Evidence::new(
            WIKI_MECHANICS,
            Confidence::Observed,
            None,
            "Community-maintained description; reference replay fixtures are still required.",
        );
        let client_options = Evidence::new(
            CURRENT_CLIENT_ASSET,
            Confidence::Observed,
            Some(CLIENT_OPTIONS_FIXTURE),
            "Reproducibly extracted TL options; all 31 fields matched the 2026-05-04 asset snapshot.",
        );
        let reset_behavior = Evidence::new(
            WIKI_MECHANICS,
            Confidence::Observed,
            Some(CLIENT_OPTIONS_FIXTURE),
            "Wiki documents move/rotation resets; client options independently expose a 15-reset cap.",
        );
        let client_firepower = Evidence::new(
            CURRENT_CLIENT_ASSET,
            Confidence::Observed,
            Some(CLIENT_FIREPOWER_FIXTURE),
            "Current client tables and 53 generated clear/combo/B2B/All-Clear cases; snapshot SHA-256 b92d2446e42752a8ba86d873696a83cee0d99223d4bdafc1355a22cabbb3206b.",
        );
        let client_garbage = Evidence::new(
            CURRENT_CLIENT_ASSET,
            Confidence::Observed,
            Some(CLIENT_GARBAGE_FIXTURE),
            "Current TL preset plus FightLines/TakeAllDamage control flow; bundle SHA-256 aab6d586aaaef57f84553cbd60237604832be420fa2b27773b6e697f66b84d66.",
        );

        Self {
            profile_id: TARGET_PROFILE_ID,
            upstream_version: "BETA 1.7.8",
            mode: "TETRA LEAGUE Season 2",
            version_evidence: Evidence::new(
                OFFICIAL_PATCH_NOTES,
                Confidence::Confirmed,
                None,
                "Official patch history identifies the pinned upstream version.",
            ),
            board_width: Sourced::new(Some(10), "cells", mechanics_observed),
            board_height: Sourced::new(Some(40), "cells", mechanics_observed),
            rotation_system: Sourced::new(Some(RotationSystem::SrsPlus), "enum", client_options),
            spin_system: Sourced::new(
                Some(SpinSystem::AllMiniPlus),
                "enum",
                Evidence::new(
                    OFFICIAL_PATCH_NOTES,
                    Confidence::Confirmed,
                    Some(CLIENT_OPTIONS_FIXTURE),
                    "BETA 1.5.0 changed multiplayer defaults; current client snapshot still selects All-Mini+.",
                ),
            ),
            timing: TimingProfileDraft {
                tick_rate_hz: Sourced::new(Some(60), "frames/second", client_options),
                are_frames: Sourced::new(Some(0), "frames", client_options),
                line_clear_are_frames: Sourced::new(Some(0), "frames", client_options),
                gravity_numerator: Sourced::new(Some(1), "cells/frame numerator", client_options),
                gravity_denominator: Sourced::new(
                    Some(50),
                    "cells/frame denominator",
                    client_options,
                ),
                gravity_increase_numerator_per_second: Sourced::new(
                    Some(7),
                    "G/second numerator",
                    client_options,
                ),
                gravity_increase_denominator_per_second: Sourced::new(
                    Some(2000),
                    "G/second denominator",
                    client_options,
                ),
                gravity_margin_frames: Sourced::new(Some(7200), "frames", client_options),
                gravity_cap_numerator: Sourced::new(
                    Some(20),
                    "cells/frame numerator",
                    client_options,
                ),
                gravity_cap_denominator: Sourced::new(
                    Some(1),
                    "cells/frame denominator",
                    client_options,
                ),
                lock_delay_frames: Sourced::new(Some(30), "frames", client_options),
                max_lock_resets: Sourced::new(Some(15), "resets", client_options),
                reset_on_lateral_move: Sourced::new(Some(true), "boolean", reset_behavior),
                reset_on_rotation: Sourced::new(Some(true), "boolean", reset_behavior),
            },
            attack: AttackProfileDraft {
                normal_clear: Sourced::new(
                    Some([0, 0, 1, 2, 4]),
                    "garbage lines",
                    client_firepower,
                ),
                mini_spin: Sourced::new(Some([0, 0, 1, 2, 4]), "garbage lines", client_firepower),
                full_spin: Sourced::new(Some([0, 2, 4, 6, 10]), "garbage lines", client_firepower),
                combo: Sourced::new(
                    Some(ComboRule::Multiplier {
                        increment_numerator: 1,
                        increment_denominator: 4,
                        zero_base_min_combo_index: &MULTIPLIER_ZERO_BASE_THRESHOLDS,
                    }),
                    "integer transition rule",
                    client_firepower,
                ),
                back_to_back_bonus: Sourced::new(Some(1), "garbage lines", client_firepower),
                back_to_back_charging: Sourced::new(Some(true), "boolean", client_firepower),
                back_to_back_charge_at: Sourced::new(Some(4), "B2B count", client_firepower),
                back_to_back_charge_base: Sourced::new(Some(0), "garbage lines", client_firepower),
                perfect_clear_attack: Sourced::new(Some(5), "garbage lines", client_firepower),
                perfect_clear_back_to_back: Sourced::new(Some(1), "B2B count", client_firepower),
                perfect_clear_back_to_back_sends: Sourced::new(
                    Some(false),
                    "boolean",
                    client_firepower,
                ),
                perfect_clear_back_to_back_dupes: Sourced::new(
                    Some(true),
                    "boolean",
                    client_firepower,
                ),
                perfect_clear_charges: Sourced::new(Some(false), "boolean", client_firepower),
                garbage_clear_special_bonus: Sourced::new(
                    Some(1),
                    "garbage lines",
                    client_firepower,
                ),
            },
            garbage: GarbageProfileDraft {
                travel_frames: Sourced::new(Some(20), "frames", client_garbage),
                garbage_cap: Sourced::new(Some(8), "lines per tank", client_garbage),
                opener_phase_pieces: Sourced::new(Some(14), "placed pieces", client_garbage),
                combo_blocking: Sourced::new(Some(true), "boolean", client_garbage),
                multiplier: Sourced::new(
                    Some(GarbageMultiplierSchedule::tetra_league_observed()),
                    "exact frame schedule",
                    client_garbage,
                ),
            },
            room_handling: RoomHandlingProfileDraft {
                enforced: Sourced::new(Some(false), "boolean", client_options),
                inactive_arr_frames: Sourced::new(Some(2), "frames", client_options),
                inactive_das_frames: Sourced::new(Some(10), "frames", client_options),
                inactive_sdf_value: Sourced::new(Some(6), "setting value", client_options),
            },
        }
    }

    pub fn missing_required_timing_fields(self) -> Vec<&'static str> {
        required_timing_fields(self.timing)
            .into_iter()
            .filter_map(|(name, value_present, _)| (!value_present).then_some(name))
            .collect()
    }

    pub fn timing_conformance_blockers(self) -> Vec<&'static str> {
        required_timing_fields(self.timing)
            .into_iter()
            .filter_map(|(name, _, evidence)| {
                (evidence.confidence != Confidence::Confirmed || evidence.fixture_id.is_none())
                    .then_some(name)
            })
            .collect()
    }

    pub fn try_timing_profile(self) -> Result<ActiveTimingProfile, ProfileActivationError> {
        let missing = self.missing_required_timing_fields();
        if !missing.is_empty() {
            return Err(ProfileActivationError::MissingRequiredFields(missing));
        }

        Ok(ActiveTimingProfile {
            tick_rate_hz: required(self.timing.tick_rate_hz.value)?,
            are_frames: required(self.timing.are_frames.value)?,
            line_clear_are_frames: required(self.timing.line_clear_are_frames.value)?,
            initial_gravity: Gravity::new(
                required(self.timing.gravity_numerator.value)?,
                required(self.timing.gravity_denominator.value)?,
            )
            .map_err(ProfileActivationError::InvalidTimingConfiguration)?,
            gravity_increase_per_second: Gravity::new(
                required(self.timing.gravity_increase_numerator_per_second.value)?,
                required(self.timing.gravity_increase_denominator_per_second.value)?,
            )
            .map_err(ProfileActivationError::InvalidTimingConfiguration)?,
            gravity_margin_frames: required(self.timing.gravity_margin_frames.value)?,
            gravity_cap: Gravity::new(
                required(self.timing.gravity_cap_numerator.value)?,
                required(self.timing.gravity_cap_denominator.value)?,
            )
            .map_err(ProfileActivationError::InvalidTimingConfiguration)?,
            lock_delay_frames: required(self.timing.lock_delay_frames.value)?,
            max_lock_resets: required(self.timing.max_lock_resets.value)?,
            reset_on_lateral_move: required(self.timing.reset_on_lateral_move.value)?,
            reset_on_rotation: required(self.timing.reset_on_rotation.value)?,
        })
    }

    pub fn try_timing_rules(self) -> Result<TimingRules, ProfileActivationError> {
        self.try_timing_profile()?.core_rules_at_frame(0)
    }

    pub fn missing_required_attack_fields(self) -> Vec<&'static str> {
        required_attack_fields(self.attack)
            .into_iter()
            .filter_map(|(name, value_present, _)| (!value_present).then_some(name))
            .collect()
    }

    pub fn attack_conformance_blockers(self) -> Vec<&'static str> {
        required_attack_fields(self.attack)
            .into_iter()
            .filter_map(|(name, _, evidence)| {
                (evidence.confidence != Confidence::Confirmed || evidence.fixture_id.is_none())
                    .then_some(name)
            })
            .collect()
    }

    pub fn try_attack_rules(self) -> Result<AttackRules, ProfileActivationError> {
        let missing = self.missing_required_attack_fields();
        if !missing.is_empty() {
            return Err(ProfileActivationError::MissingRequiredFields(missing));
        }

        let rules = AttackRules {
            normal_clear: required(self.attack.normal_clear.value)?,
            mini_spin: required(self.attack.mini_spin.value)?,
            full_spin: required(self.attack.full_spin.value)?,
            combo: required(self.attack.combo.value)?,
            back_to_back_bonus: required(self.attack.back_to_back_bonus.value)?,
            back_to_back_charging: required(self.attack.back_to_back_charging.value)?,
            back_to_back_charge_at: required(self.attack.back_to_back_charge_at.value)?,
            back_to_back_charge_base: required(self.attack.back_to_back_charge_base.value)?,
            perfect_clear_attack: required(self.attack.perfect_clear_attack.value)?,
            perfect_clear_back_to_back: required(self.attack.perfect_clear_back_to_back.value)?,
            perfect_clear_back_to_back_sends: required(
                self.attack.perfect_clear_back_to_back_sends.value,
            )?,
            perfect_clear_back_to_back_dupes: required(
                self.attack.perfect_clear_back_to_back_dupes.value,
            )?,
            perfect_clear_charges: required(self.attack.perfect_clear_charges.value)?,
            garbage_clear_special_bonus: required(self.attack.garbage_clear_special_bonus.value)?,
        };
        rules
            .validate()
            .map_err(ProfileActivationError::InvalidAttackConfiguration)?;
        Ok(rules)
    }

    pub fn missing_required_garbage_fields(self) -> Vec<&'static str> {
        required_garbage_fields(self.garbage)
            .into_iter()
            .filter_map(|(name, value_present, _)| (!value_present).then_some(name))
            .collect()
    }

    pub fn garbage_conformance_blockers(self) -> Vec<&'static str> {
        required_garbage_fields(self.garbage)
            .into_iter()
            .filter_map(|(name, _, evidence)| {
                (evidence.confidence != Confidence::Confirmed || evidence.fixture_id.is_none())
                    .then_some(name)
            })
            .collect()
    }

    pub fn try_garbage_rules(self) -> Result<GarbageRules, ProfileActivationError> {
        let missing = self.missing_required_garbage_fields();
        if !missing.is_empty() {
            return Err(ProfileActivationError::MissingRequiredFields(missing));
        }

        let rules = GarbageRules {
            travel_frames: required(self.garbage.travel_frames.value)?,
            garbage_cap: required(self.garbage.garbage_cap.value)?,
            opener_phase_pieces: required(self.garbage.opener_phase_pieces.value)?,
            combo_blocking: required(self.garbage.combo_blocking.value)?,
            messiness: GarbageMessinessRules::tetra_league_observed(),
            multiplier: required(self.garbage.multiplier.value)?,
        };
        rules
            .validate()
            .map_err(ProfileActivationError::InvalidGarbageConfiguration)?;
        Ok(rules)
    }

    /// Activates the complete score-free local 1v1 rules. TL does not enforce
    /// room handling, so callers must pass both participants' effective
    /// serialized profiles explicitly.
    pub fn try_battle_rules(
        self,
        player_handling: [PlayerHandlingProfile; 2],
    ) -> Result<BattleRules, ProfileActivationError> {
        Ok(BattleRules {
            game: GameConfig::default(),
            timing: self.try_timing_profile()?.core_schedule()?,
            handling: [
                player_handling[0].core_rules(),
                player_handling[1].core_rules(),
            ],
            attack: self.try_attack_rules()?,
            garbage: self.try_garbage_rules()?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileActivationError {
    MissingRequiredFields(Vec<&'static str>),
    InvalidTimingConfiguration(TimingConfigError),
    InvalidAttackConfiguration(AttackConfigError),
    InvalidGarbageConfiguration(GarbageConfigError),
    ZeroTickRate,
    TimingArithmeticOverflow,
    InternalValidationInvariant,
}

fn required_timing_fields(timing: TimingProfileDraft) -> [(&'static str, bool, Evidence); 14] {
    [
        (
            "timing.tick_rate_hz",
            timing.tick_rate_hz.value.is_some(),
            timing.tick_rate_hz.evidence,
        ),
        (
            "timing.are_frames",
            timing.are_frames.value.is_some(),
            timing.are_frames.evidence,
        ),
        (
            "timing.line_clear_are_frames",
            timing.line_clear_are_frames.value.is_some(),
            timing.line_clear_are_frames.evidence,
        ),
        (
            "timing.gravity_numerator",
            timing.gravity_numerator.value.is_some(),
            timing.gravity_numerator.evidence,
        ),
        (
            "timing.gravity_denominator",
            timing.gravity_denominator.value.is_some(),
            timing.gravity_denominator.evidence,
        ),
        (
            "timing.gravity_increase_numerator_per_second",
            timing.gravity_increase_numerator_per_second.value.is_some(),
            timing.gravity_increase_numerator_per_second.evidence,
        ),
        (
            "timing.gravity_increase_denominator_per_second",
            timing
                .gravity_increase_denominator_per_second
                .value
                .is_some(),
            timing.gravity_increase_denominator_per_second.evidence,
        ),
        (
            "timing.gravity_margin_frames",
            timing.gravity_margin_frames.value.is_some(),
            timing.gravity_margin_frames.evidence,
        ),
        (
            "timing.gravity_cap_numerator",
            timing.gravity_cap_numerator.value.is_some(),
            timing.gravity_cap_numerator.evidence,
        ),
        (
            "timing.gravity_cap_denominator",
            timing.gravity_cap_denominator.value.is_some(),
            timing.gravity_cap_denominator.evidence,
        ),
        (
            "timing.lock_delay_frames",
            timing.lock_delay_frames.value.is_some(),
            timing.lock_delay_frames.evidence,
        ),
        (
            "timing.max_lock_resets",
            timing.max_lock_resets.value.is_some(),
            timing.max_lock_resets.evidence,
        ),
        (
            "timing.reset_on_lateral_move",
            timing.reset_on_lateral_move.value.is_some(),
            timing.reset_on_lateral_move.evidence,
        ),
        (
            "timing.reset_on_rotation",
            timing.reset_on_rotation.value.is_some(),
            timing.reset_on_rotation.evidence,
        ),
    ]
}

fn required_attack_fields(attack: AttackProfileDraft) -> [(&'static str, bool, Evidence); 14] {
    [
        (
            "attack.normal_clear",
            attack.normal_clear.value.is_some(),
            attack.normal_clear.evidence,
        ),
        (
            "attack.mini_spin",
            attack.mini_spin.value.is_some(),
            attack.mini_spin.evidence,
        ),
        (
            "attack.full_spin",
            attack.full_spin.value.is_some(),
            attack.full_spin.evidence,
        ),
        (
            "attack.combo",
            attack.combo.value.is_some(),
            attack.combo.evidence,
        ),
        (
            "attack.back_to_back_bonus",
            attack.back_to_back_bonus.value.is_some(),
            attack.back_to_back_bonus.evidence,
        ),
        (
            "attack.back_to_back_charging",
            attack.back_to_back_charging.value.is_some(),
            attack.back_to_back_charging.evidence,
        ),
        (
            "attack.back_to_back_charge_at",
            attack.back_to_back_charge_at.value.is_some(),
            attack.back_to_back_charge_at.evidence,
        ),
        (
            "attack.back_to_back_charge_base",
            attack.back_to_back_charge_base.value.is_some(),
            attack.back_to_back_charge_base.evidence,
        ),
        (
            "attack.perfect_clear_attack",
            attack.perfect_clear_attack.value.is_some(),
            attack.perfect_clear_attack.evidence,
        ),
        (
            "attack.perfect_clear_back_to_back",
            attack.perfect_clear_back_to_back.value.is_some(),
            attack.perfect_clear_back_to_back.evidence,
        ),
        (
            "attack.perfect_clear_back_to_back_sends",
            attack.perfect_clear_back_to_back_sends.value.is_some(),
            attack.perfect_clear_back_to_back_sends.evidence,
        ),
        (
            "attack.perfect_clear_back_to_back_dupes",
            attack.perfect_clear_back_to_back_dupes.value.is_some(),
            attack.perfect_clear_back_to_back_dupes.evidence,
        ),
        (
            "attack.perfect_clear_charges",
            attack.perfect_clear_charges.value.is_some(),
            attack.perfect_clear_charges.evidence,
        ),
        (
            "attack.garbage_clear_special_bonus",
            attack.garbage_clear_special_bonus.value.is_some(),
            attack.garbage_clear_special_bonus.evidence,
        ),
    ]
}

fn required_garbage_fields(garbage: GarbageProfileDraft) -> [(&'static str, bool, Evidence); 5] {
    [
        (
            "garbage.travel_frames",
            garbage.travel_frames.value.is_some(),
            garbage.travel_frames.evidence,
        ),
        (
            "garbage.garbage_cap",
            garbage.garbage_cap.value.is_some(),
            garbage.garbage_cap.evidence,
        ),
        (
            "garbage.opener_phase_pieces",
            garbage.opener_phase_pieces.value.is_some(),
            garbage.opener_phase_pieces.evidence,
        ),
        (
            "garbage.combo_blocking",
            garbage.combo_blocking.value.is_some(),
            garbage.combo_blocking.evidence,
        ),
        (
            "garbage.multiplier",
            garbage.multiplier.value.is_some(),
            garbage.multiplier.evidence,
        ),
    ]
}

const fn map_timing_schedule_error(error: TimingScheduleError) -> ProfileActivationError {
    match error {
        TimingScheduleError::ZeroTickRate => ProfileActivationError::ZeroTickRate,
        TimingScheduleError::ArithmeticOverflow => ProfileActivationError::TimingArithmeticOverflow,
        TimingScheduleError::InvalidGravity(error) => {
            ProfileActivationError::InvalidTimingConfiguration(error)
        }
    }
}

fn required<T>(value: Option<T>) -> Result<T, ProfileActivationError> {
    value.ok_or(ProfileActivationError::InternalValidationInvariant)
}

#[cfg(test)]
mod tests {
    use super::{
        Confidence, PLAYER_HANDLING_SCHEMA_VERSION, PlayerHandlingProfile, TARGET_PROFILE_ID,
        TetrioRulesDraft,
    };
    use engine_core::{ClearEvent, PieceKind, SoftDropMode, SpinMode};
    use versus::{AttackContext, AttackPacketKind, AttackState, resolve_attack};

    #[test]
    fn observed_target_has_no_missing_timing_literals_and_activates() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let timing = profile
            .try_timing_rules()
            .expect("observed profile activates");

        assert_eq!(profile.profile_id, TARGET_PROFILE_ID);
        assert!(profile.missing_required_timing_fields().is_empty());
        assert_eq!(timing.gravity.numerator(), 2400);
        assert_eq!(timing.gravity.denominator(), 120_000);
        assert_eq!(timing.lock_delay_frames, 30);
        assert_eq!(timing.max_lock_resets, 15);
        assert!(timing.reset_on_lateral_move);
        assert!(timing.reset_on_rotation);
    }

    #[test]
    fn gravity_schedule_uses_exact_rationals_and_caps_at_twenty_g() {
        let timing = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2()
            .try_timing_profile()
            .expect("observed profile activates");

        let initial = timing.gravity_at_frame(0).unwrap();
        let at_margin = timing.gravity_at_frame(7200).unwrap();
        assert_eq!(initial.numerator(), 2400);
        assert_eq!(initial.denominator(), 120_000);
        assert_eq!(at_margin, initial);
        let first_increased_fall_frame = timing.gravity_at_frame(7202).unwrap();
        assert_eq!(first_increased_fall_frame.numerator(), 2407);
        let after_one_second = timing.gravity_at_frame(7261).unwrap();
        assert_eq!(after_one_second.numerator(), 2820);
        assert_eq!(after_one_second.denominator(), initial.denominator());
        let capped = timing.gravity_at_frame(10_000_000).unwrap();
        assert_eq!(capped.numerator(), 2_400_000);
        assert_eq!(capped.denominator(), initial.denominator());
    }

    #[test]
    fn executable_observation_is_not_conformance_certification() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let blockers = profile.timing_conformance_blockers();

        assert!(!blockers.is_empty());
        assert!(blockers.contains(&"timing.gravity_numerator"));
        assert!(blockers.contains(&"timing.lock_delay_frames"));
    }

    #[test]
    fn client_observations_carry_snapshot_identity() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let evidence = profile.timing.lock_delay_frames.evidence;

        assert_eq!(evidence.confidence, Confidence::Observed);
        assert_eq!(
            evidence.fixture_id,
            Some("client-options-hv-63ab5c7c7-20260824")
        );
        assert!(evidence.source_url.contains("20260810T191705"));
    }

    #[test]
    fn tetra_league_requires_player_specific_handling() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();

        assert!(profile.room_handling.requires_player_handling());
        assert_eq!(profile.room_handling.inactive_arr_frames.value, Some(2));
        assert_eq!(profile.room_handling.inactive_das_frames.value, Some(10));
        assert_eq!(profile.room_handling.inactive_sdf_value.value, Some(6));
    }

    #[test]
    fn normalized_player_handling_maps_without_unit_guessing() {
        let profile = PlayerHandlingProfile::normalized(8, 0, 1, SoftDropMode::Sonic);
        let rules = profile.core_rules();

        assert_eq!(profile.schema_version, PLAYER_HANDLING_SCHEMA_VERSION);
        assert_eq!(rules.das_frames, 8);
        assert_eq!(rules.arr_frames, 0);
        assert_eq!(rules.dcd_frames, 1);
        assert_eq!(rules.soft_drop, SoftDropMode::Sonic);
    }

    #[test]
    fn observed_spin_profile_maps_to_all_mini_plus_core_rules() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let spin = profile
            .spin_system
            .value
            .expect("spin system is present")
            .core_rules();

        assert_eq!(spin.mode, SpinMode::AllMiniPlus);
        assert_eq!(spin.t_full_kick_upgrade_mask, 1 << 3);
    }

    #[test]
    fn observed_profile_builds_frame_dynamic_two_player_rules() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let rules = profile
            .try_battle_rules([
                PlayerHandlingProfile::normalized(8, 0, 1, SoftDropMode::Sonic),
                PlayerHandlingProfile::normalized(10, 2, 2, SoftDropMode::CellsPerFrame(6)),
            ])
            .expect("battle profile activates");

        assert_eq!(rules.handling[0].das_frames, 8);
        assert_eq!(rules.handling[1].das_frames, 10);
        assert_eq!(
            rules
                .timing
                .rules_at_frame(7_261)
                .unwrap()
                .gravity
                .numerator(),
            2_820
        );
        assert_eq!(rules.garbage.travel_frames, 20);
    }

    #[test]
    fn observed_attack_profile_activates_and_carries_current_fixture() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let rules = profile
            .try_attack_rules()
            .expect("attack profile activates");
        let outcome = resolve_attack(
            AttackState::default(),
            ClearEvent::new(PieceKind::I, 4, None, true),
            AttackContext::default(),
            rules,
        )
        .expect("quad perfect clear attack");

        assert!(profile.missing_required_attack_fields().is_empty());
        assert_eq!(profile.attack.normal_clear.value, Some([0, 0, 1, 2, 4]));
        assert_eq!(
            profile.attack.combo.evidence.fixture_id,
            Some("client-firepower-hv-63ab5c7c7-20260824")
        );
        assert_eq!(outcome.total_attack(), 10);
        assert_eq!(outcome.packets.as_slice()[0].kind, AttackPacketKind::Clear);
        assert_eq!(
            outcome.packets.as_slice()[1].kind,
            AttackPacketKind::PerfectClear
        );
        assert!(!profile.attack_conformance_blockers().is_empty());
    }

    #[test]
    fn observed_garbage_profile_activates_with_current_tl_values() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let rules = profile
            .try_garbage_rules()
            .expect("garbage profile activates");

        assert!(profile.missing_required_garbage_fields().is_empty());
        assert_eq!(rules.travel_frames, 20);
        assert_eq!(rules.garbage_cap, 8);
        assert_eq!(rules.opener_phase_pieces, 14);
        assert!(rules.combo_blocking);
        assert_eq!(
            profile.garbage.travel_frames.evidence.fixture_id,
            Some("client-garbage-hv-63ab5c7c7-20260824")
        );
        assert!(!profile.garbage_conformance_blockers().is_empty());
    }
}
