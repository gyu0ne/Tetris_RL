//! Versioned TETR.IO mechanics profiles with field-level provenance.
//!
//! A profile draft cannot become executable while required literals remain
//! unconfirmed. This prevents research notes or historical defaults from being
//! mistaken for a conformant TETR.IO ruleset.

#![forbid(unsafe_code)]

use engine_core::{Gravity, TimingConfigError, TimingRules};

pub const TARGET_PROFILE_ID: &str = "tetrio-beta-1.7.8-tetra-league-season-2";
pub const RESEARCH_ACCESS_DATE: &str = "2026-08-24";

const OFFICIAL_PATCH_NOTES: &str = "https://tetr.io/about/patchnotes/";
const OFFICIAL_HANDLING_FAQ: &str = "https://github.com/tetrio/faq/blob/main/mechanics.html";
const WIKI_MECHANICS: &str = "https://tetrio.wiki.gg/wiki/Mechanics";
const WIKI_TETRA_LEAGUE: &str = "https://tetrio.wiki.gg/wiki/TETRA_LEAGUE";

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
    pub gravity_numerator: Sourced<u32>,
    pub gravity_denominator: Sourced<u32>,
    pub lock_delay_frames: Sourced<u16>,
    pub max_lock_resets: Sourced<u16>,
    pub reset_on_lateral_move: Sourced<bool>,
    pub reset_on_rotation: Sourced<bool>,
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
}

impl TetrioRulesDraft {
    pub const fn tetra_league_beta_1_7_8_season_2() -> Self {
        let mechanics_observed = Evidence::new(
            WIKI_MECHANICS,
            Confidence::Observed,
            None,
            "Community-maintained description; current replay fixtures are still required.",
        );
        let exact_timing_missing = Evidence::new(
            OFFICIAL_HANDLING_FAQ,
            Confidence::Unconfirmed,
            None,
            "The public handling description does not establish the pinned TL frame literal.",
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
            rotation_system: Sourced::new(
                Some(RotationSystem::SrsPlus),
                "enum",
                mechanics_observed,
            ),
            spin_system: Sourced::new(
                Some(SpinSystem::AllMiniPlus),
                "enum",
                Evidence::new(
                    OFFICIAL_PATCH_NOTES,
                    Confidence::Confirmed,
                    None,
                    "BETA 1.5.0 changed multiplayer defaults to All-Mini+.",
                ),
            ),
            timing: TimingProfileDraft {
                tick_rate_hz: Sourced::new(
                    Some(60),
                    "frames/second",
                    Evidence::new(
                        WIKI_TETRA_LEAGUE,
                        Confidence::Observed,
                        None,
                        "Observed frame convention; requires replay timestamp confirmation.",
                    ),
                ),
                gravity_numerator: Sourced::new(
                    None,
                    "cells/frame numerator",
                    exact_timing_missing,
                ),
                gravity_denominator: Sourced::new(
                    None,
                    "cells/frame denominator",
                    exact_timing_missing,
                ),
                lock_delay_frames: Sourced::new(None, "frames", exact_timing_missing),
                max_lock_resets: Sourced::new(None, "resets", exact_timing_missing),
                reset_on_lateral_move: Sourced::new(None, "boolean", exact_timing_missing),
                reset_on_rotation: Sourced::new(None, "boolean", exact_timing_missing),
            },
        }
    }

    pub fn missing_required_timing_fields(self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.timing.gravity_numerator.value.is_none() {
            missing.push("timing.gravity_numerator");
        }
        if self.timing.gravity_denominator.value.is_none() {
            missing.push("timing.gravity_denominator");
        }
        if self.timing.lock_delay_frames.value.is_none() {
            missing.push("timing.lock_delay_frames");
        }
        if self.timing.max_lock_resets.value.is_none() {
            missing.push("timing.max_lock_resets");
        }
        if self.timing.reset_on_lateral_move.value.is_none() {
            missing.push("timing.reset_on_lateral_move");
        }
        if self.timing.reset_on_rotation.value.is_none() {
            missing.push("timing.reset_on_rotation");
        }
        missing
    }

    pub fn try_timing_rules(self) -> Result<TimingRules, ProfileActivationError> {
        let missing = self.missing_required_timing_fields();
        if !missing.is_empty() {
            return Err(ProfileActivationError::MissingRequiredFields(missing));
        }

        let numerator = required(self.timing.gravity_numerator.value)?;
        let denominator = required(self.timing.gravity_denominator.value)?;
        let gravity = Gravity::new(numerator, denominator)
            .map_err(ProfileActivationError::InvalidTimingConfiguration)?;

        Ok(TimingRules::new(
            gravity,
            required(self.timing.lock_delay_frames.value)?,
            required(self.timing.max_lock_resets.value)?,
            required(self.timing.reset_on_lateral_move.value)?,
            required(self.timing.reset_on_rotation.value)?,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileActivationError {
    MissingRequiredFields(Vec<&'static str>),
    InvalidTimingConfiguration(TimingConfigError),
    InternalValidationInvariant,
}

fn required<T>(value: Option<T>) -> Result<T, ProfileActivationError> {
    value.ok_or(ProfileActivationError::InternalValidationInvariant)
}

#[cfg(test)]
mod tests {
    use super::{
        Confidence, Evidence, ProfileActivationError, Sourced, TARGET_PROFILE_ID, TetrioRulesDraft,
    };

    #[test]
    fn pinned_target_refuses_activation_while_literals_are_unconfirmed() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let expected = profile.missing_required_timing_fields();

        assert_eq!(profile.profile_id, TARGET_PROFILE_ID);
        assert_eq!(expected.len(), 6);
        assert_eq!(
            profile.try_timing_rules(),
            Err(ProfileActivationError::MissingRequiredFields(expected))
        );
    }

    #[test]
    fn every_target_field_carries_source_metadata() {
        let profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();

        assert!(profile.version_evidence.source_url.starts_with("https://"));
        assert!(!profile.version_evidence.accessed_on.is_empty());
        assert_eq!(profile.version_evidence.confidence, Confidence::Confirmed);
        assert_eq!(
            profile.board_width.evidence.confidence,
            Confidence::Observed
        );
        assert_eq!(
            profile.timing.lock_delay_frames.evidence.confidence,
            Confidence::Unconfirmed
        );
    }

    #[test]
    fn complete_fixture_backed_draft_can_activate() {
        let mut profile = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2();
        let fixture = Evidence::new(
            "https://example.invalid/local-conformance-fixture",
            Confidence::Confirmed,
            Some("test-fixture-001"),
            "Synthetic unit-test fixture, not a TETR.IO claim.",
        );
        profile.timing.gravity_numerator = Sourced::new(Some(1), "cells/frame numerator", fixture);
        profile.timing.gravity_denominator =
            Sourced::new(Some(2), "cells/frame denominator", fixture);
        profile.timing.lock_delay_frames = Sourced::new(Some(30), "frames", fixture);
        profile.timing.max_lock_resets = Sourced::new(Some(15), "resets", fixture);
        profile.timing.reset_on_lateral_move = Sourced::new(Some(true), "boolean", fixture);
        profile.timing.reset_on_rotation = Sourced::new(Some(true), "boolean", fixture);

        let timing = profile
            .try_timing_rules()
            .expect("complete draft activates");
        assert_eq!(timing.gravity.numerator(), 1);
        assert_eq!(timing.gravity.denominator(), 2);
        assert_eq!(timing.lock_delay_frames, 30);
    }
}
