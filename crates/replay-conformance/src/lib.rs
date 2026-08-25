//! Engine-neutral frame snapshots for reference replay differential tests.
//!
//! This crate does not guess the undocumented `.ttrm` wire format. A
//! version-pinned adapter must first convert a user-owned replay/reference
//! capture into these canonical snapshots.

#![forbid(unsafe_code)]

use engine_core::{
    FrameSession, GameState, HEIGHT, LastAction, PieceKind, PieceState, TimingState, TopOutReason,
};

mod battle;
mod normalized;
mod suite;

pub use battle::*;
pub use normalized::*;
pub use suite::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimingSnapshot {
    pub fall_fraction_micros: u32,
    pub lock_elapsed_frames: u16,
    pub lock_resets_used: u16,
    pub locked: bool,
    pub last_action: LastAction,
}

impl From<&TimingState> for TimingSnapshot {
    fn from(state: &TimingState) -> Self {
        Self {
            fall_fraction_micros: state.fall_fraction_micros,
            lock_elapsed_frames: state.lock_elapsed_frames,
            lock_resets_used: state.lock_resets_used,
            locked: state.locked,
            last_action: state.last_action,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameSnapshot {
    pub frame: u64,
    pub board_rows: [u16; HEIGHT],
    pub garbage_rows: [u16; HEIGHT],
    pub active: PieceState,
    pub hold: Option<PieceKind>,
    pub preview: Vec<PieceKind>,
    pub top_out: Option<TopOutReason>,
    pub timing: Option<TimingSnapshot>,
}

impl FrameSnapshot {
    pub fn from_game(frame: u64, game: &GameState) -> Self {
        Self {
            frame,
            board_rows: *game.board().rows(),
            garbage_rows: *game.board().garbage_rows(),
            active: game.active(),
            hold: game.hold(),
            preview: game.preview(),
            top_out: game.top_out_reason(),
            timing: None,
        }
    }

    pub fn from_timed_game(frame: u64, game: &GameState, timing: &TimingState) -> Self {
        Self {
            frame,
            board_rows: *game.board().rows(),
            garbage_rows: *game.board().garbage_rows(),
            active: timing.piece,
            hold: game.hold(),
            preview: game.preview(),
            top_out: game.top_out_reason(),
            timing: Some(timing.into()),
        }
    }

    pub fn from_session(session: &FrameSession) -> Self {
        match session.timing() {
            Some(timing) => Self::from_timed_game(session.frame(), session.game(), timing),
            None => Self::from_game(session.frame(), session.game()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowDifference {
    pub row: usize,
    pub expected: u16,
    pub actual: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotDifference {
    FrameNumber {
        expected: u64,
        actual: u64,
    },
    BoardRows(Vec<RowDifference>),
    GarbageRows(Vec<RowDifference>),
    ActivePiece {
        expected: PieceState,
        actual: PieceState,
    },
    Hold {
        expected: Option<PieceKind>,
        actual: Option<PieceKind>,
    },
    Preview {
        expected: Vec<PieceKind>,
        actual: Vec<PieceKind>,
    },
    TopOut {
        expected: Option<TopOutReason>,
        actual: Option<TopOutReason>,
    },
    Timing {
        expected: Option<TimingSnapshot>,
        actual: Option<TimingSnapshot>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameMismatch {
    pub index: usize,
    pub expected_frame: u64,
    pub actual_frame: u64,
    pub difference: SnapshotDifference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConformanceMismatch {
    Frame(FrameMismatch),
    TraceLength {
        matched_frames: usize,
        expected: usize,
        actual: usize,
    },
}

/// Returns the first deterministic mismatch between a reference and local trace.
pub fn compare_traces(
    expected: &[FrameSnapshot],
    actual: &[FrameSnapshot],
) -> Result<(), ConformanceMismatch> {
    for (index, (expected_frame, actual_frame)) in expected.iter().zip(actual).enumerate() {
        if let Some(difference) = compare_snapshot(expected_frame, actual_frame) {
            return Err(ConformanceMismatch::Frame(FrameMismatch {
                index,
                expected_frame: expected_frame.frame,
                actual_frame: actual_frame.frame,
                difference,
            }));
        }
    }

    if expected.len() != actual.len() {
        return Err(ConformanceMismatch::TraceLength {
            matched_frames: expected.len().min(actual.len()),
            expected: expected.len(),
            actual: actual.len(),
        });
    }

    Ok(())
}

pub fn compare_snapshot(
    expected: &FrameSnapshot,
    actual: &FrameSnapshot,
) -> Option<SnapshotDifference> {
    if expected.frame != actual.frame {
        return Some(SnapshotDifference::FrameNumber {
            expected: expected.frame,
            actual: actual.frame,
        });
    }

    if expected.board_rows != actual.board_rows {
        let differences = expected
            .board_rows
            .iter()
            .zip(actual.board_rows)
            .enumerate()
            .filter_map(|(row, (expected, actual))| {
                (*expected != actual).then_some(RowDifference {
                    row,
                    expected: *expected,
                    actual,
                })
            })
            .collect();
        return Some(SnapshotDifference::BoardRows(differences));
    }
    if expected.garbage_rows != actual.garbage_rows {
        let differences = expected
            .garbage_rows
            .iter()
            .zip(actual.garbage_rows)
            .enumerate()
            .filter_map(|(row, (expected, actual))| {
                (*expected != actual).then_some(RowDifference {
                    row,
                    expected: *expected,
                    actual,
                })
            })
            .collect();
        return Some(SnapshotDifference::GarbageRows(differences));
    }
    if expected.active != actual.active {
        return Some(SnapshotDifference::ActivePiece {
            expected: expected.active,
            actual: actual.active,
        });
    }
    if expected.hold != actual.hold {
        return Some(SnapshotDifference::Hold {
            expected: expected.hold,
            actual: actual.hold,
        });
    }
    if expected.preview != actual.preview {
        return Some(SnapshotDifference::Preview {
            expected: expected.preview.clone(),
            actual: actual.preview.clone(),
        });
    }
    if expected.top_out != actual.top_out {
        return Some(SnapshotDifference::TopOut {
            expected: expected.top_out,
            actual: actual.top_out,
        });
    }
    if expected.timing != actual.timing {
        return Some(SnapshotDifference::Timing {
            expected: expected.timing,
            actual: actual.timing,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        BattlePlayerSnapshot, BattleSnapshot, BattleSnapshotDifference, ConformanceMismatch,
        FrameSnapshot, FunctionalCaseFailure, FunctionalCaseKind, FunctionalConformanceCase,
        FunctionalConformancePolicy, FunctionalConformanceStatus, InvalidCaseReason, MechanicClaim,
        REQUIRED_MECHANIC_CLAIMS, ReferenceEvidence, ReferenceTrace, SnapshotDifference,
        TimingSnapshot, compare_battle_traces, compare_traces, evaluate_functional_conformance,
        evaluate_functional_conformance_with_policy,
    };
    use engine_core::{GameConfig, GameState, LastAction};
    use versus::{AttackMultiplier, AttackState, BattleResult, IncomingGarbagePacket};

    fn snapshot(frame: u64) -> FrameSnapshot {
        let game = GameState::new(41, GameConfig::default()).expect("valid game");
        FrameSnapshot::from_game(frame, &game)
    }

    fn battle_snapshot(frame: u64) -> BattleSnapshot {
        let player = BattlePlayerSnapshot {
            game: snapshot(frame),
            attack: AttackState::default(),
            incoming: Vec::new(),
            sent_lines: 0,
        };
        BattleSnapshot {
            frame,
            player_one: player.clone(),
            player_two: player,
            garbage_multiplier: AttackMultiplier::one(),
            result: BattleResult::Ongoing,
            events: None,
        }
    }

    fn evidence(target_profile: &str) -> ReferenceEvidence {
        ReferenceEvidence {
            target_profile: target_profile.to_owned(),
            reference_build: "TETR.IO BETA 1.7.8".to_owned(),
            source: "sanitized user-owned reference capture".to_owned(),
            artifact_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn identical_traces_match() {
        let expected = vec![snapshot(0), snapshot(1)];
        assert_eq!(compare_traces(&expected, &expected), Ok(()));
    }

    #[test]
    fn first_board_difference_reports_exact_row_bits() {
        let expected = vec![snapshot(10)];
        let mut actual = expected.clone();
        actual[0].board_rows[3] = 0b0000_0011;

        let mismatch = compare_traces(&expected, &actual).expect_err("must differ");
        let ConformanceMismatch::Frame(frame) = mismatch else {
            panic!("expected frame mismatch");
        };
        let SnapshotDifference::BoardRows(rows) = frame.difference else {
            panic!("expected row mismatch");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].row, 3);
        assert_eq!(rows[0].expected, 0);
        assert_eq!(rows[0].actual, 3);
    }

    #[test]
    fn garbage_provenance_difference_is_not_hidden_by_equal_occupancy() {
        let mut expected = vec![snapshot(12)];
        let mut actual = expected.clone();
        expected[0].board_rows[2] = 0b0000_0011;
        actual[0].board_rows[2] = 0b0000_0011;
        actual[0].garbage_rows[2] = 0b0000_0001;

        let mismatch = compare_traces(&expected, &actual).expect_err("provenance must differ");
        let ConformanceMismatch::Frame(frame) = mismatch else {
            panic!("expected frame mismatch");
        };
        let SnapshotDifference::GarbageRows(rows) = frame.difference else {
            panic!("expected garbage row mismatch");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].row, 2);
        assert_eq!(rows[0].expected, 0);
        assert_eq!(rows[0].actual, 1);
    }

    #[test]
    fn timing_and_length_mismatches_are_distinct() {
        let expected = vec![snapshot(0), snapshot(1)];
        let mut timing_actual = expected.clone();
        timing_actual[0].timing = Some(TimingSnapshot {
            fall_fraction_micros: 1,
            lock_elapsed_frames: 0,
            lock_resets_used: 0,
            locked: false,
            last_action: LastAction::None,
        });
        assert!(matches!(
            compare_traces(&expected, &timing_actual),
            Err(ConformanceMismatch::Frame(_))
        ));
        assert_eq!(
            compare_traces(&expected, &expected[..1]),
            Err(ConformanceMismatch::TraceLength {
                matched_frames: 1,
                expected: 2,
                actual: 1,
            })
        );
    }

    #[test]
    fn battle_trace_reports_exact_queue_difference() {
        let expected = vec![battle_snapshot(8)];
        let mut actual = expected.clone();
        actual[0].player_two.incoming.push(IncomingGarbagePacket {
            lines: 2,
            hole_column: Some(4),
            ready_at_frame: 28,
            hardened: false,
        });

        let mismatch = compare_battle_traces(&expected, &actual).expect_err("must differ");
        let super::BattleConformanceMismatch::Frame(frame) = mismatch else {
            panic!("expected frame mismatch");
        };
        assert!(matches!(
            frame.difference,
            BattleSnapshotDifference::PlayerTwoIncoming { .. }
        ));
    }

    #[test]
    fn all_required_claims_and_exact_traces_are_conformant() {
        let target = "tetrio-beta-1.7.8-tl-s2";
        let solo = vec![snapshot(0)];
        let battle = vec![battle_snapshot(0)];
        let solo_claims = REQUIRED_MECHANIC_CLAIMS
            .iter()
            .copied()
            .filter(|claim| !claim.requires_battle_trace())
            .collect();
        let battle_claims = REQUIRED_MECHANIC_CLAIMS
            .iter()
            .copied()
            .filter(|claim| claim.requires_battle_trace())
            .collect();
        let cases = vec![
            FunctionalConformanceCase {
                id: "solo-reference".to_owned(),
                kind: FunctionalCaseKind::Boundary,
                evidence: evidence(target),
                claims: solo_claims,
                trace: ReferenceTrace::Solo {
                    expected: &solo,
                    actual: &solo,
                },
            },
            FunctionalConformanceCase {
                id: "battle-reference".to_owned(),
                kind: FunctionalCaseKind::RandomizedBattle,
                evidence: evidence(target),
                claims: battle_claims,
                trace: ReferenceTrace::Battle {
                    expected: &battle,
                    actual: &battle,
                },
            },
        ];

        let report = evaluate_functional_conformance_with_policy(
            target,
            &cases,
            FunctionalConformancePolicy {
                minimum_randomized_battle_cases: 1,
            },
        );
        assert_eq!(report.status, FunctionalConformanceStatus::Conformant);
        assert_eq!(report.compared_cases, 2);
        assert_eq!(report.compared_frames, 2);
        assert!(report.missing_claims.is_empty());
        assert!(report.failures.is_empty());
    }

    #[test]
    fn a_mismatch_is_divergent_and_does_not_cover_its_claim() {
        let target = "tetrio-beta-1.7.8-tl-s2";
        let expected = vec![snapshot(0)];
        let mut actual = expected.clone();
        actual[0].board_rows[0] = 1;
        let cases = vec![FunctionalConformanceCase {
            id: "geometry-reference".to_owned(),
            kind: FunctionalCaseKind::Boundary,
            evidence: evidence(target),
            claims: vec![MechanicClaim::BoardAndClearGeometry],
            trace: ReferenceTrace::Solo {
                expected: &expected,
                actual: &actual,
            },
        }];

        let report = evaluate_functional_conformance(target, &cases);
        assert_eq!(report.status, FunctionalConformanceStatus::Divergent);
        assert!(
            report
                .missing_claims
                .contains(&MechanicClaim::BoardAndClearGeometry)
        );
        assert!(matches!(
            report.failures.as_slice(),
            [FunctionalCaseFailure::SoloMismatch { .. }]
        ));
    }

    #[test]
    fn battle_claim_cannot_be_covered_by_a_solo_trace() {
        let target = "tetrio-beta-1.7.8-tl-s2";
        let trace = vec![snapshot(0)];
        let cases = vec![FunctionalConformanceCase {
            id: "wrong-trace-kind".to_owned(),
            kind: FunctionalCaseKind::Boundary,
            evidence: evidence(target),
            claims: vec![MechanicClaim::GarbageTransitAndCancellation],
            trace: ReferenceTrace::Solo {
                expected: &trace,
                actual: &trace,
            },
        }];

        let report = evaluate_functional_conformance(target, &cases);
        assert_eq!(report.status, FunctionalConformanceStatus::Divergent);
        assert!(matches!(
            report.failures.as_slice(),
            [FunctionalCaseFailure::Invalid {
                reason: InvalidCaseReason::BattleClaimRequiresBattleTrace(
                    MechanicClaim::GarbageTransitAndCancellation
                ),
                ..
            }]
        ));
    }

    #[test]
    fn default_policy_requires_ten_thousand_randomized_battle_cases() {
        let target = "tetrio-beta-1.7.8-tl-s2";
        let battle = vec![battle_snapshot(0)];
        let cases = vec![FunctionalConformanceCase {
            id: "one-random-seed".to_owned(),
            kind: FunctionalCaseKind::RandomizedBattle,
            evidence: evidence(target),
            claims: REQUIRED_MECHANIC_CLAIMS.to_vec(),
            trace: ReferenceTrace::Battle {
                expected: &battle,
                actual: &battle,
            },
        }];

        let report = evaluate_functional_conformance(target, &cases);
        assert_eq!(report.status, FunctionalConformanceStatus::Incomplete);
        assert_eq!(report.randomized_battle_cases, 1);
        assert_eq!(report.required_randomized_battle_cases, 10_000);
    }
}
