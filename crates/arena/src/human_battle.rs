use crate::versus_arena::{
    VERSUS_CANDIDATE_FEATURE_COUNT, VersusChoice, VersusClosedLoopError,
    enumerate_player_candidates, new_battle, selected_action,
};
use engine_core::{InputEdge, PieceKind, VISIBLE_HEIGHT};
use versus::{BattleFrameOutcome, BattleResult, BattleSession, PlayerId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanBattleCandidateBatch {
    pub features: Vec<[i32; VERSUS_CANDIDATE_FEATURE_COUNT]>,
    pub due: bool,
    pub done: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanBattlePlayerSnapshot {
    pub board_rows: Vec<u16>,
    pub garbage_rows: Vec<u16>,
    pub active: Option<(&'static str, Vec<(i16, i16)>)>,
    pub hold: Option<&'static str>,
    pub preview: Vec<&'static str>,
    pub pieces_placed: u64,
    pub pending_garbage: u64,
    pub ready_garbage: u64,
    pub sent_lines: u64,
    pub combo: u32,
    pub back_to_back: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HumanBattleSnapshot {
    pub frame: u64,
    pub result: &'static str,
    pub next_bot_frame: u64,
    pub frames_per_placement: u32,
    pub players: [HumanBattlePlayerSnapshot; 2],
}

/// One real-time human (player one) against one placement-policy bot (player
/// two). Human inputs advance every authoritative frame. The bot locks exactly
/// one selected reachable afterstate at the configured cadence.
pub struct HumanBattle {
    battle: BattleSession,
    frames_per_placement: u32,
    next_bot_frame: u64,
    bot_choices: Option<Vec<VersusChoice>>,
}

impl HumanBattle {
    pub fn new(seed: u64, frames_per_placement: u32) -> Result<Self, VersusClosedLoopError> {
        if frames_per_placement == 0 {
            return Err(VersusClosedLoopError::ZeroCadence);
        }
        Ok(Self {
            battle: new_battle(seed)?,
            frames_per_placement,
            next_bot_frame: 0,
            bot_choices: None,
        })
    }

    pub fn bot_candidates(&mut self) -> Result<HumanBattleCandidateBatch, VersusClosedLoopError> {
        let done = self.battle.result() != BattleResult::Ongoing;
        let due = !done && self.battle.frame() >= self.next_bot_frame;
        if due && self.bot_choices.is_none() {
            self.bot_choices = Some(enumerate_player_candidates(&self.battle, PlayerId::Two)?);
        }
        let features = if due {
            self.bot_choices
                .as_ref()
                .expect("due candidates are initialized")
                .iter()
                .map(|choice| choice.features)
                .collect()
        } else {
            Vec::new()
        };
        Ok(HumanBattleCandidateBatch {
            features,
            due,
            done,
        })
    }

    pub fn step(
        &mut self,
        human_edges: &[InputEdge],
        bot_selection: Option<usize>,
    ) -> Result<BattleFrameOutcome, VersusClosedLoopError> {
        let due = self.battle.frame() >= self.next_bot_frame;
        let outcome = if due {
            if self.bot_choices.is_none() {
                self.bot_choices = Some(enumerate_player_candidates(&self.battle, PlayerId::Two)?);
            }
            let action = selected_action(&self.bot_choices, bot_selection, 1)?;
            self.battle.step_player_two_placement(human_edges, action)?
        } else {
            if bot_selection.is_some() {
                return Err(VersusClosedLoopError::SelectionBeforeCadence {
                    frame: self.battle.frame(),
                    next_frame: self.next_bot_frame,
                });
            }
            self.battle.step(human_edges, &[])?
        };
        if due {
            self.bot_choices = None;
            self.next_bot_frame = self
                .next_bot_frame
                .saturating_add(u64::from(self.frames_per_placement));
        }
        Ok(outcome)
    }

    pub fn snapshot(&self) -> HumanBattleSnapshot {
        HumanBattleSnapshot {
            frame: self.battle.frame(),
            result: result_name(self.battle.result()),
            next_bot_frame: self.next_bot_frame,
            frames_per_placement: self.frames_per_placement,
            players: [
                player_snapshot(&self.battle, PlayerId::One),
                player_snapshot(&self.battle, PlayerId::Two),
            ],
        }
    }
}

fn player_snapshot(battle: &BattleSession, id: PlayerId) -> HumanBattlePlayerSnapshot {
    let player = battle.player(id);
    let session = player.session();
    let game = session.game();
    HumanBattlePlayerSnapshot {
        board_rows: game.board().rows()[..VISIBLE_HEIGHT].to_vec(),
        garbage_rows: game.board().garbage_rows()[..VISIBLE_HEIGHT].to_vec(),
        active: session.timing().map(|timing| {
            (
                piece_name(timing.piece.kind),
                timing.piece.cells().into_iter().collect(),
            )
        }),
        hold: game.hold().map(piece_name),
        preview: game.preview().into_iter().map(piece_name).collect(),
        pieces_placed: game.pieces_placed(),
        pending_garbage: player.incoming().pending_lines(),
        ready_garbage: player.incoming().ready_lines(battle.frame()),
        sent_lines: player.sent_lines(),
        combo: player.attack_state().combo,
        back_to_back: player.attack_state().back_to_back,
    }
}

const fn result_name(result: BattleResult) -> &'static str {
    match result {
        BattleResult::Ongoing => "ongoing",
        BattleResult::PlayerOneWin => "human_win",
        BattleResult::PlayerTwoWin => "model_win",
        BattleResult::Draw => "draw",
    }
}

const fn piece_name(piece: PieceKind) -> &'static str {
    match piece {
        PieceKind::I => "I",
        PieceKind::J => "J",
        PieceKind::L => "L",
        PieceKind::O => "O",
        PieceKind::S => "S",
        PieceKind::T => "T",
        PieceKind::Z => "Z",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{InputButton, InputEdge};

    #[test]
    fn human_and_model_can_lock_on_the_same_frame() {
        let mut battle = HumanBattle::new(7, 12).unwrap();
        let candidates = battle.bot_candidates().unwrap();
        assert!(candidates.due);
        battle
            .step(&[InputEdge::press(InputButton::HardDrop)], Some(0))
            .unwrap();

        let snapshot = battle.snapshot();
        assert_eq!(snapshot.frame, 1);
        assert_eq!(snapshot.players[0].pieces_placed, 1);
        assert_eq!(snapshot.players[1].pieces_placed, 1);
        assert_eq!(snapshot.next_bot_frame, 12);
    }

    #[test]
    fn model_candidates_follow_the_configured_frame_cadence() {
        let mut battle = HumanBattle::new(11, 12).unwrap();
        battle.bot_candidates().unwrap();
        battle.step(&[], Some(0)).unwrap();
        for _ in 1..12 {
            assert!(!battle.bot_candidates().unwrap().due);
            battle.step(&[], None).unwrap();
        }
        assert_eq!(battle.snapshot().frame, 12);
        assert!(battle.bot_candidates().unwrap().due);
    }

    #[test]
    fn snapshot_exposes_two_visible_playfields() {
        let battle = HumanBattle::new(13, 12).unwrap();
        let snapshot = battle.snapshot();
        assert_eq!(snapshot.players[0].board_rows.len(), 20);
        assert_eq!(snapshot.players[1].board_rows.len(), 20);
        assert!(
            snapshot
                .players
                .iter()
                .all(|player| player.active.is_some())
        );
    }
}
