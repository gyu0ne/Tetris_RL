use crate::{FrameSnapshot, SnapshotDifference, compare_snapshot};
use versus::{
    AttackMultiplier, AttackOutcome, AttackPackets, AttackState, BattleFrameOutcome,
    BattlePlayerFrameOutcome, BattlePlayerState, BattleResult, BattleSession,
    GarbageCancellationOutcome, GarbageInsertionOutcome, IncomingGarbagePacket, PlayerId,
};

/// Engine-neutral event projection. Lock and spawn effects are already
/// represented by the post-frame game snapshot, so only battle-visible attack
/// and garbage transitions are duplicated here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BattlePlayerEventsSnapshot {
    pub attack: Option<AttackOutcome>,
    pub cancellation: Option<GarbageCancellationOutcome>,
    pub insertion: Option<GarbageInsertionOutcome>,
    pub transmitted: AttackPackets,
}

impl From<&BattlePlayerFrameOutcome> for BattlePlayerEventsSnapshot {
    fn from(outcome: &BattlePlayerFrameOutcome) -> Self {
        Self {
            attack: outcome.attack,
            cancellation: outcome.cancellation,
            insertion: outcome.insertion,
            transmitted: outcome.transmitted,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BattleEventsSnapshot {
    pub frame: u64,
    pub player_one: BattlePlayerEventsSnapshot,
    pub player_two: BattlePlayerEventsSnapshot,
    pub result: BattleResult,
}

impl From<&BattleFrameOutcome> for BattleEventsSnapshot {
    fn from(outcome: &BattleFrameOutcome) -> Self {
        Self {
            frame: outcome.frame,
            player_one: (&outcome.player_one).into(),
            player_two: (&outcome.player_two).into(),
            result: outcome.result,
        }
    }
}

/// Observable 1v1 state for one player at a shared battle frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattlePlayerSnapshot {
    pub game: FrameSnapshot,
    pub attack: AttackState,
    pub incoming: Vec<IncomingGarbagePacket>,
    pub sent_lines: u64,
}

impl BattlePlayerSnapshot {
    fn from_player(player: &BattlePlayerState) -> Self {
        let (head, tail) = player.incoming().as_slices();
        Self {
            game: FrameSnapshot::from_session(player.session()),
            attack: player.attack_state(),
            incoming: head.iter().chain(tail).copied().collect(),
            sent_lines: player.sent_lines(),
        }
    }
}

/// Canonical shared-frame snapshot for score-free TETRA LEAGUE-style 1v1.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattleSnapshot {
    pub frame: u64,
    pub player_one: BattlePlayerSnapshot,
    pub player_two: BattlePlayerSnapshot,
    pub garbage_multiplier: AttackMultiplier,
    pub result: BattleResult,
    /// Events emitted while advancing the immediately preceding shared frame.
    pub events: Option<BattleEventsSnapshot>,
}

impl BattleSnapshot {
    pub fn from_battle(battle: &BattleSession) -> Self {
        Self {
            frame: battle.frame(),
            player_one: BattlePlayerSnapshot::from_player(battle.player(PlayerId::One)),
            player_two: BattlePlayerSnapshot::from_player(battle.player(PlayerId::Two)),
            garbage_multiplier: battle.garbage_multiplier(),
            result: battle.result(),
            events: None,
        }
    }

    pub fn after_step(battle: &BattleSession, outcome: &BattleFrameOutcome) -> Self {
        let mut snapshot = Self::from_battle(battle);
        snapshot.events = Some(outcome.into());
        snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleSnapshotDifference {
    FrameNumber {
        expected: u64,
        actual: u64,
    },
    PlayerOneGame(SnapshotDifference),
    PlayerTwoGame(SnapshotDifference),
    PlayerOneAttack {
        expected: AttackState,
        actual: AttackState,
    },
    PlayerTwoAttack {
        expected: AttackState,
        actual: AttackState,
    },
    PlayerOneIncoming {
        expected: Vec<IncomingGarbagePacket>,
        actual: Vec<IncomingGarbagePacket>,
    },
    PlayerTwoIncoming {
        expected: Vec<IncomingGarbagePacket>,
        actual: Vec<IncomingGarbagePacket>,
    },
    PlayerOneSentLines {
        expected: u64,
        actual: u64,
    },
    PlayerTwoSentLines {
        expected: u64,
        actual: u64,
    },
    GarbageMultiplier {
        expected: AttackMultiplier,
        actual: AttackMultiplier,
    },
    Result {
        expected: BattleResult,
        actual: BattleResult,
    },
    Events {
        expected: Option<Box<BattleEventsSnapshot>>,
        actual: Option<Box<BattleEventsSnapshot>>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattleFrameMismatch {
    pub index: usize,
    pub expected_frame: u64,
    pub actual_frame: u64,
    pub difference: BattleSnapshotDifference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BattleConformanceMismatch {
    Frame(Box<BattleFrameMismatch>),
    TraceLength {
        matched_frames: usize,
        expected: usize,
        actual: usize,
    },
}

/// Returns the first deterministic 1v1 mismatch, including attack and garbage state.
pub fn compare_battle_traces(
    expected: &[BattleSnapshot],
    actual: &[BattleSnapshot],
) -> Result<(), BattleConformanceMismatch> {
    for (index, (expected_frame, actual_frame)) in expected.iter().zip(actual).enumerate() {
        if let Some(difference) = compare_battle_snapshot(expected_frame, actual_frame) {
            return Err(BattleConformanceMismatch::Frame(Box::new(
                BattleFrameMismatch {
                    index,
                    expected_frame: expected_frame.frame,
                    actual_frame: actual_frame.frame,
                    difference,
                },
            )));
        }
    }

    if expected.len() != actual.len() {
        return Err(BattleConformanceMismatch::TraceLength {
            matched_frames: expected.len().min(actual.len()),
            expected: expected.len(),
            actual: actual.len(),
        });
    }

    Ok(())
}

fn compare_battle_snapshot(
    expected: &BattleSnapshot,
    actual: &BattleSnapshot,
) -> Option<BattleSnapshotDifference> {
    if expected.frame != actual.frame {
        return Some(BattleSnapshotDifference::FrameNumber {
            expected: expected.frame,
            actual: actual.frame,
        });
    }
    if let Some(difference) = compare_snapshot(&expected.player_one.game, &actual.player_one.game) {
        return Some(BattleSnapshotDifference::PlayerOneGame(difference));
    }
    if let Some(difference) = compare_snapshot(&expected.player_two.game, &actual.player_two.game) {
        return Some(BattleSnapshotDifference::PlayerTwoGame(difference));
    }
    if expected.player_one.attack != actual.player_one.attack {
        return Some(BattleSnapshotDifference::PlayerOneAttack {
            expected: expected.player_one.attack,
            actual: actual.player_one.attack,
        });
    }
    if expected.player_two.attack != actual.player_two.attack {
        return Some(BattleSnapshotDifference::PlayerTwoAttack {
            expected: expected.player_two.attack,
            actual: actual.player_two.attack,
        });
    }
    if expected.player_one.incoming != actual.player_one.incoming {
        return Some(BattleSnapshotDifference::PlayerOneIncoming {
            expected: expected.player_one.incoming.clone(),
            actual: actual.player_one.incoming.clone(),
        });
    }
    if expected.player_two.incoming != actual.player_two.incoming {
        return Some(BattleSnapshotDifference::PlayerTwoIncoming {
            expected: expected.player_two.incoming.clone(),
            actual: actual.player_two.incoming.clone(),
        });
    }
    if expected.player_one.sent_lines != actual.player_one.sent_lines {
        return Some(BattleSnapshotDifference::PlayerOneSentLines {
            expected: expected.player_one.sent_lines,
            actual: actual.player_one.sent_lines,
        });
    }
    if expected.player_two.sent_lines != actual.player_two.sent_lines {
        return Some(BattleSnapshotDifference::PlayerTwoSentLines {
            expected: expected.player_two.sent_lines,
            actual: actual.player_two.sent_lines,
        });
    }
    if expected.garbage_multiplier != actual.garbage_multiplier {
        return Some(BattleSnapshotDifference::GarbageMultiplier {
            expected: expected.garbage_multiplier,
            actual: actual.garbage_multiplier,
        });
    }
    if expected.result != actual.result {
        return Some(BattleSnapshotDifference::Result {
            expected: expected.result,
            actual: actual.result,
        });
    }
    if expected.events != actual.events {
        return Some(BattleSnapshotDifference::Events {
            expected: expected.events.map(Box::new),
            actual: actual.events.map(Box::new),
        });
    }
    None
}
