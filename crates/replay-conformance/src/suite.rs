use crate::{
    BattleConformanceMismatch, BattleSnapshot, ConformanceMismatch, FrameSnapshot,
    compare_battle_traces, compare_traces,
};
use std::collections::HashSet;

/// Atomic mechanics claims that must each be covered by at least one exact
/// version-pinned reference trace. These are coverage labels, not self-issued
/// operator approval or substitutes for the trace comparison itself.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MechanicClaim {
    BoardAndClearGeometry,
    PieceRandomizer,
    SpawnHoldAndPreview,
    MovementAndReachability,
    RotationAndKicks,
    DasArrDcdAndSoftDrop,
    GravityAndLocking,
    IrsIhsAndSpawnOrder,
    SpinAndPerfectClear,
    TopOutClassification,
    AttackComboAndPerfectClear,
    BackToBackChargingAndSurge,
    OpenerCancellation,
    GarbageTransitAndCancellation,
    GarbageCapHoleAndInsertion,
    MarginMultiplier,
    ClutchAndGarbageOut,
    SimultaneousBattleScheduling,
    RoundTerminalResult,
    DeterministicReplay,
}

pub const REQUIRED_MECHANIC_CLAIMS: [MechanicClaim; 20] = [
    MechanicClaim::BoardAndClearGeometry,
    MechanicClaim::PieceRandomizer,
    MechanicClaim::SpawnHoldAndPreview,
    MechanicClaim::MovementAndReachability,
    MechanicClaim::RotationAndKicks,
    MechanicClaim::DasArrDcdAndSoftDrop,
    MechanicClaim::GravityAndLocking,
    MechanicClaim::IrsIhsAndSpawnOrder,
    MechanicClaim::SpinAndPerfectClear,
    MechanicClaim::TopOutClassification,
    MechanicClaim::AttackComboAndPerfectClear,
    MechanicClaim::BackToBackChargingAndSurge,
    MechanicClaim::OpenerCancellation,
    MechanicClaim::GarbageTransitAndCancellation,
    MechanicClaim::GarbageCapHoleAndInsertion,
    MechanicClaim::MarginMultiplier,
    MechanicClaim::ClutchAndGarbageOut,
    MechanicClaim::SimultaneousBattleScheduling,
    MechanicClaim::RoundTerminalResult,
    MechanicClaim::DeterministicReplay,
];

impl MechanicClaim {
    pub const fn requires_battle_trace(self) -> bool {
        matches!(
            self,
            Self::AttackComboAndPerfectClear
                | Self::BackToBackChargingAndSurge
                | Self::OpenerCancellation
                | Self::GarbageTransitAndCancellation
                | Self::GarbageCapHoleAndInsertion
                | Self::MarginMultiplier
                | Self::ClutchAndGarbageOut
                | Self::SimultaneousBattleScheduling
                | Self::RoundTerminalResult
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BoardAndClearGeometry => "board_and_clear_geometry",
            Self::PieceRandomizer => "piece_randomizer",
            Self::SpawnHoldAndPreview => "spawn_hold_and_preview",
            Self::MovementAndReachability => "movement_and_reachability",
            Self::RotationAndKicks => "rotation_and_kicks",
            Self::DasArrDcdAndSoftDrop => "das_arr_dcd_and_soft_drop",
            Self::GravityAndLocking => "gravity_and_locking",
            Self::IrsIhsAndSpawnOrder => "irs_ihs_and_spawn_order",
            Self::SpinAndPerfectClear => "spin_and_perfect_clear",
            Self::TopOutClassification => "top_out_classification",
            Self::AttackComboAndPerfectClear => "attack_combo_and_perfect_clear",
            Self::BackToBackChargingAndSurge => "back_to_back_charging_and_surge",
            Self::OpenerCancellation => "opener_cancellation",
            Self::GarbageTransitAndCancellation => "garbage_transit_and_cancellation",
            Self::GarbageCapHoleAndInsertion => "garbage_cap_hole_and_insertion",
            Self::MarginMultiplier => "margin_multiplier",
            Self::ClutchAndGarbageOut => "clutch_and_garbage_out",
            Self::SimultaneousBattleScheduling => "simultaneous_battle_scheduling",
            Self::RoundTerminalResult => "round_terminal_result",
            Self::DeterministicReplay => "deterministic_replay",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceEvidence {
    pub target_profile: String,
    pub reference_build: String,
    pub source: String,
    pub artifact_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionalCaseKind {
    Boundary,
    RandomizedBattle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionalConformancePolicy {
    pub minimum_randomized_battle_cases: usize,
}

impl Default for FunctionalConformancePolicy {
    fn default() -> Self {
        Self {
            minimum_randomized_battle_cases: 10_000,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ReferenceTrace<'a> {
    Solo {
        expected: &'a [FrameSnapshot],
        actual: &'a [FrameSnapshot],
    },
    Battle {
        expected: &'a [BattleSnapshot],
        actual: &'a [BattleSnapshot],
    },
}

#[derive(Clone, Debug)]
pub struct FunctionalConformanceCase<'a> {
    pub id: String,
    pub kind: FunctionalCaseKind,
    pub evidence: ReferenceEvidence,
    pub claims: Vec<MechanicClaim>,
    pub trace: ReferenceTrace<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionalConformanceStatus {
    Incomplete,
    Divergent,
    Conformant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidCaseReason {
    EmptyId,
    DuplicateId,
    EmptyReferenceMetadata,
    TargetProfileMismatch { expected: String, actual: String },
    InvalidArtifactSha256,
    NoClaims,
    EmptyTrace,
    BattleClaimRequiresBattleTrace(MechanicClaim),
    RandomizedCaseRequiresBattleTrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionalCaseFailure {
    Invalid {
        case_id: String,
        reason: InvalidCaseReason,
    },
    SoloMismatch {
        case_id: String,
        mismatch: ConformanceMismatch,
    },
    BattleMismatch {
        case_id: String,
        mismatch: BattleConformanceMismatch,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimCoverage {
    pub claim: MechanicClaim,
    pub passing_case_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionalConformanceReport {
    pub target_profile: String,
    pub status: FunctionalConformanceStatus,
    pub compared_cases: usize,
    pub compared_frames: usize,
    pub randomized_battle_cases: usize,
    pub required_randomized_battle_cases: usize,
    pub coverage: Vec<ClaimCoverage>,
    pub missing_claims: Vec<MechanicClaim>,
    pub failures: Vec<FunctionalCaseFailure>,
}

/// Evaluates exact observable equivalence for a declared fixture corpus.
///
/// `Conformant` means every required mechanics claim has at least one passing,
/// version-pinned reference case, the randomized-battle floor is met, and the
/// entire supplied corpus has zero mismatch. It deliberately does not mean
/// approval by TETR.IO's operator.
pub fn evaluate_functional_conformance(
    target_profile: &str,
    cases: &[FunctionalConformanceCase<'_>],
) -> FunctionalConformanceReport {
    evaluate_functional_conformance_with_policy(
        target_profile,
        cases,
        FunctionalConformancePolicy::default(),
    )
}

pub fn evaluate_functional_conformance_with_policy(
    target_profile: &str,
    cases: &[FunctionalConformanceCase<'_>],
    policy: FunctionalConformancePolicy,
) -> FunctionalConformanceReport {
    let mut seen_ids = HashSet::new();
    let mut passing_claims: Vec<(MechanicClaim, String)> = Vec::new();
    let mut failures = Vec::new();
    let mut compared_cases = 0;
    let mut compared_frames = 0;
    let mut randomized_battle_cases = 0;

    for case in cases {
        if let Some(reason) = validate_case(target_profile, case, &mut seen_ids) {
            failures.push(FunctionalCaseFailure::Invalid {
                case_id: case.id.clone(),
                reason,
            });
            continue;
        }

        let comparison = match case.trace {
            ReferenceTrace::Solo { expected, actual } => {
                compared_frames += expected.len().min(actual.len());
                compare_traces(expected, actual).map_err(|mismatch| {
                    FunctionalCaseFailure::SoloMismatch {
                        case_id: case.id.clone(),
                        mismatch,
                    }
                })
            }
            ReferenceTrace::Battle { expected, actual } => {
                compared_frames += expected.len().min(actual.len());
                compare_battle_traces(expected, actual).map_err(|mismatch| {
                    FunctionalCaseFailure::BattleMismatch {
                        case_id: case.id.clone(),
                        mismatch,
                    }
                })
            }
        };
        compared_cases += 1;

        match comparison {
            Ok(()) => {
                if case.kind == FunctionalCaseKind::RandomizedBattle {
                    randomized_battle_cases += 1;
                }
                passing_claims.extend(
                    case.claims
                        .iter()
                        .copied()
                        .map(|claim| (claim, case.id.clone())),
                );
            }
            Err(failure) => failures.push(failure),
        }
    }

    let coverage: Vec<_> = REQUIRED_MECHANIC_CLAIMS
        .iter()
        .copied()
        .map(|claim| ClaimCoverage {
            claim,
            passing_case_ids: passing_claims
                .iter()
                .filter_map(|(covered, id)| (*covered == claim).then_some(id.clone()))
                .collect(),
        })
        .collect();
    let missing_claims = coverage
        .iter()
        .filter_map(|coverage| {
            coverage
                .passing_case_ids
                .is_empty()
                .then_some(coverage.claim)
        })
        .collect::<Vec<_>>();
    let status = if !failures.is_empty() {
        FunctionalConformanceStatus::Divergent
    } else if !missing_claims.is_empty()
        || randomized_battle_cases < policy.minimum_randomized_battle_cases
    {
        FunctionalConformanceStatus::Incomplete
    } else {
        FunctionalConformanceStatus::Conformant
    };

    FunctionalConformanceReport {
        target_profile: target_profile.to_owned(),
        status,
        compared_cases,
        compared_frames,
        randomized_battle_cases,
        required_randomized_battle_cases: policy.minimum_randomized_battle_cases,
        coverage,
        missing_claims,
        failures,
    }
}

fn validate_case(
    target_profile: &str,
    case: &FunctionalConformanceCase<'_>,
    seen_ids: &mut HashSet<String>,
) -> Option<InvalidCaseReason> {
    if case.id.trim().is_empty() {
        return Some(InvalidCaseReason::EmptyId);
    }
    if !seen_ids.insert(case.id.clone()) {
        return Some(InvalidCaseReason::DuplicateId);
    }
    if case.evidence.reference_build.trim().is_empty() || case.evidence.source.trim().is_empty() {
        return Some(InvalidCaseReason::EmptyReferenceMetadata);
    }
    if case.evidence.target_profile != target_profile {
        return Some(InvalidCaseReason::TargetProfileMismatch {
            expected: target_profile.to_owned(),
            actual: case.evidence.target_profile.clone(),
        });
    }
    if !is_sha256(&case.evidence.artifact_sha256) {
        return Some(InvalidCaseReason::InvalidArtifactSha256);
    }
    if case.claims.is_empty() {
        return Some(InvalidCaseReason::NoClaims);
    }
    match case.trace {
        ReferenceTrace::Solo { expected, actual } => {
            if expected.is_empty() || actual.is_empty() {
                return Some(InvalidCaseReason::EmptyTrace);
            }
            if let Some(claim) = case
                .claims
                .iter()
                .copied()
                .find(|claim| claim.requires_battle_trace())
            {
                return Some(InvalidCaseReason::BattleClaimRequiresBattleTrace(claim));
            }
            if case.kind == FunctionalCaseKind::RandomizedBattle {
                return Some(InvalidCaseReason::RandomizedCaseRequiresBattleTrace);
            }
        }
        ReferenceTrace::Battle { expected, actual } => {
            if expected.is_empty() || actual.is_empty() {
                return Some(InvalidCaseReason::EmptyTrace);
            }
        }
    }
    None
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
