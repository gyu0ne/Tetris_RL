//! Versioned TETR.IO mechanics profiles with field-level provenance.
//!
//! Public-client observations may make a profile executable for local research,
//! but only reference replay fixtures can make it conformance-ready. Those two
//! states are deliberately represented separately.

#![forbid(unsafe_code)]

use engine_core::{Gravity, HandlingRules, SoftDropMode, TimingConfigError, TimingRules};

pub const TARGET_PROFILE_ID: &str = "tetrio-beta-1.7.8-tetra-league-season-2";
pub const RESEARCH_ACCESS_DATE: &str = "2026-08-24";
pub const PLAYER_HANDLING_SCHEMA_VERSION: u16 = 1;

const OFFICIAL_PATCH_NOTES: &str = "https://tetr.io/about/patchnotes/";
const WIKI_MECHANICS: &str = "https://tetrio.wiki.gg/wiki/Mechanics";
const CURRENT_CLIENT_ASSET: &str =
    "https://tetr.io/js/tetrio.js?hv=63ab5c7c7.efa161fa8f91.20260810T191705";
const CLIENT_OPTIONS_FIXTURE: &str = "client-options-hv-63ab5c7c7-20260824";

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
    pub fn core_rules_at_frame(
        self,
        elapsed_frames: u64,
    ) -> Result<TimingRules, ProfileActivationError> {
        Ok(TimingRules::new(
            self.gravity_at_frame(elapsed_frames)?,
            self.lock_delay_frames,
            self.max_lock_resets,
            self.reset_on_lateral_move,
            self.reset_on_rotation,
        ))
    }

    pub fn gravity_at_frame(self, elapsed_frames: u64) -> Result<Gravity, ProfileActivationError> {
        let increase_frames = elapsed_frames.saturating_sub(u64::from(self.gravity_margin_frames));
        add_scaled_per_second(
            self.initial_gravity,
            self.gravity_increase_per_second,
            increase_frames,
            self.tick_rate_hz,
            self.gravity_cap,
        )
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileActivationError {
    MissingRequiredFields(Vec<&'static str>),
    InvalidTimingConfiguration(TimingConfigError),
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

fn add_scaled_per_second(
    base: Gravity,
    increase_per_second: Gravity,
    elapsed_frames: u64,
    tick_rate_hz: u16,
    cap: Gravity,
) -> Result<Gravity, ProfileActivationError> {
    if tick_rate_hz == 0 {
        return Err(ProfileActivationError::ZeroTickRate);
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
        .ok_or(ProfileActivationError::TimingArithmeticOverflow)?;
    let base_scaled = base_numerator
        .checked_mul(increase_denominator)
        .and_then(|value| value.checked_mul(ticks))
        .and_then(|value| value.checked_mul(u128::from(cap.denominator())))
        .ok_or(ProfileActivationError::TimingArithmeticOverflow)?;
    let increase_per_frame_scaled = increase_numerator
        .checked_mul(base_denominator)
        .and_then(|value| value.checked_mul(u128::from(cap.denominator())))
        .ok_or(ProfileActivationError::TimingArithmeticOverflow)?;
    let cap_scaled = u128::from(cap.numerator())
        .checked_mul(base_denominator)
        .and_then(|value| value.checked_mul(increase_denominator))
        .and_then(|value| value.checked_mul(ticks))
        .ok_or(ProfileActivationError::TimingArithmeticOverflow)?;

    // Reduce by one schedule-wide factor. Per-frame reduction would change the
    // accumulator's unit while gravity rises and corrupt the carried remainder.
    let schedule_divisor = gcd(
        gcd(base_scaled, increase_per_frame_scaled),
        gcd(common_denominator, cap_scaled),
    );
    let denominator = common_denominator / schedule_divisor;
    let base_scaled = base_scaled / schedule_divisor;
    let increase_per_frame_scaled = increase_per_frame_scaled / schedule_divisor;
    let cap_scaled = cap_scaled / schedule_divisor;
    let increase_scaled = increase_per_frame_scaled
        .checked_mul(elapsed)
        .ok_or(ProfileActivationError::TimingArithmeticOverflow)?;
    let numerator = base_scaled
        .checked_add(increase_scaled)
        .ok_or(ProfileActivationError::TimingArithmeticOverflow)?
        .min(cap_scaled);

    let fixed_numerator =
        u32::try_from(numerator).map_err(|_| ProfileActivationError::TimingArithmeticOverflow)?;
    let fixed_denominator =
        u32::try_from(denominator).map_err(|_| ProfileActivationError::TimingArithmeticOverflow)?;
    Gravity::new(fixed_numerator, fixed_denominator)
        .map_err(ProfileActivationError::InvalidTimingConfiguration)
}

const fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
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
    use engine_core::SoftDropMode;

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
        let after_one_second = timing.gravity_at_frame(7260).unwrap();
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
}
