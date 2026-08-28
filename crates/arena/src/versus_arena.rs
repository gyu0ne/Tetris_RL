use crate::features::{analyze_afterstate, board_summary, column_heights_and_holes};
use crate::{FEATURE_COUNT, GenerationError};
use engine_core::{GameState, PieceKind, SoftDropMode, SpinClassification, WIDTH};
use rayon::prelude::*;
use rules_tetrio::{PlayerHandlingProfile, TetrioRulesDraft};
use std::fmt;
use versus::{
    AttackContext, BattleError, BattleResult, BattleSession, PlacementAction, PlayerId,
    cancel_attack_packets, resolve_attack,
};

const PIECE_CONTEXT_FEATURE_COUNT: usize = 35;
pub const VERSUS_CANDIDATE_FEATURE_COUNT: usize = FEATURE_COUNT + 10 + WIDTH * 2 + 1 + 35;
pub const VERSUS_CANDIDATE_DIAGNOSTIC_COUNT: usize = 5;
pub const VERSUS_STATE_FEATURE_COUNT: usize = 12 + WIDTH * 4 + 35 * 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersusCandidateBatch {
    pub features: Vec<[i32; VERSUS_CANDIDATE_FEATURE_COUNT]>,
    pub diagnostics: Vec<[i32; VERSUS_CANDIDATE_DIAGNOSTIC_COUNT]>,
    pub state_features: Vec<[i32; VERSUS_STATE_FEATURE_COUNT]>,
    pub offsets: Vec<usize>,
    pub done: Vec<bool>,
    pub results: Vec<i8>,
}

#[derive(Clone)]
struct VersusChoice {
    features: [i32; VERSUS_CANDIDATE_FEATURE_COUNT],
    diagnostics: [i32; VERSUS_CANDIDATE_DIAGNOSTIC_COUNT],
    action: PlacementAction,
}

#[derive(Clone)]
struct MatchState {
    battle: BattleSession,
    choices: [Option<Vec<VersusChoice>>; 2],
}

struct MatchCandidateData {
    features: [Vec<[i32; VERSUS_CANDIDATE_FEATURE_COUNT]>; 2],
    diagnostics: [Vec<[i32; VERSUS_CANDIDATE_DIAGNOSTIC_COUNT]>; 2],
    state_features: [[i32; VERSUS_STATE_FEATURE_COUNT]; 2],
    done: [bool; 2],
    results: [i8; 2],
}

#[derive(Clone)]
pub struct VersusBatch {
    matches: Vec<MatchState>,
    frames_per_placement: u32,
}

impl VersusBatch {
    pub fn new(seeds: &[u64], frames_per_placement: u32) -> Result<Self, VersusClosedLoopError> {
        if seeds.is_empty() {
            return Err(VersusClosedLoopError::EmptySeeds);
        }
        if frames_per_placement == 0 {
            return Err(VersusClosedLoopError::ZeroCadence);
        }
        let matches = seeds
            .iter()
            .copied()
            .map(new_match)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            matches,
            frames_per_placement,
        })
    }

    pub fn restore(
        seeds: &[u64],
        histories: &[Vec<(usize, usize)>],
        frames_per_placement: u32,
    ) -> Result<Self, VersusClosedLoopError> {
        if histories.len() != seeds.len() {
            return Err(VersusClosedLoopError::HistoryCount {
                expected: seeds.len(),
                actual: histories.len(),
            });
        }
        let mut batch = Self::new(seeds, frames_per_placement)?;
        batch
            .matches
            .par_iter_mut()
            .zip(histories.par_iter())
            .enumerate()
            .try_for_each(|(match_index, (match_state, history))| {
                for (step, &(one, two)) in history.iter().enumerate() {
                    if match_state.battle.result() != BattleResult::Ongoing {
                        return Err(VersusClosedLoopError::HistoryAfterTerminal {
                            match_index,
                            step,
                        });
                    }
                    step_match(
                        match_state,
                        [Some(one), Some(two)],
                        frames_per_placement,
                        match_index * 2,
                    )?;
                }
                Ok(())
            })?;
        Ok(batch)
    }

    pub fn candidates(&mut self) -> Result<VersusCandidateBatch, VersusClosedLoopError> {
        let matches = self
            .matches
            .par_iter_mut()
            .map(match_candidate_data)
            .collect::<Result<Vec<_>, _>>()?;
        let mut features = Vec::new();
        let mut diagnostics = Vec::new();
        let mut state_features = Vec::with_capacity(self.matches.len() * 2);
        let mut offsets = Vec::with_capacity(self.matches.len() * 2 + 1);
        let mut done = Vec::with_capacity(self.matches.len() * 2);
        let mut results = Vec::with_capacity(self.matches.len() * 2);
        offsets.push(0);

        for match_data in matches {
            for player in 0..2 {
                state_features.push(match_data.state_features[player]);
                features.extend(match_data.features[player].iter().copied());
                diagnostics.extend(match_data.diagnostics[player].iter().copied());
                offsets.push(features.len());
                done.push(match_data.done[player]);
                results.push(match_data.results[player]);
            }
        }

        Ok(VersusCandidateBatch {
            features,
            diagnostics,
            state_features,
            offsets,
            done,
            results,
        })
    }

    pub fn step(&mut self, selections: &[Option<usize>]) -> Result<(), VersusClosedLoopError> {
        let expected = self.matches.len() * 2;
        if selections.len() != expected {
            return Err(VersusClosedLoopError::SelectionCount {
                expected,
                actual: selections.len(),
            });
        }

        for (match_index, match_state) in self.matches.iter_mut().enumerate() {
            let base = match_index * 2;
            if match_state.battle.result() != BattleResult::Ongoing {
                if selections[base..base + 2].iter().any(Option::is_some) {
                    return Err(VersusClosedLoopError::SelectionForDoneMatch(match_index));
                }
                continue;
            }
            step_match(
                match_state,
                [selections[base], selections[base + 1]],
                self.frames_per_placement,
                base,
            )?;
        }
        Ok(())
    }

    pub fn reset_done(&mut self, seeds: &[u64]) -> Result<(), VersusClosedLoopError> {
        let completed = self
            .matches
            .iter()
            .filter(|state| state.battle.result() != BattleResult::Ongoing)
            .count();
        if seeds.len() != completed {
            return Err(VersusClosedLoopError::ResetSeedCount {
                expected: completed,
                actual: seeds.len(),
            });
        }
        let mut seeds = seeds.iter().copied();
        for match_state in &mut self.matches {
            if match_state.battle.result() != BattleResult::Ongoing {
                *match_state = new_match(seeds.next().expect("count checked"))?;
            }
        }
        Ok(())
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }
}

fn step_match(
    match_state: &mut MatchState,
    selections: [Option<usize>; 2],
    frames_per_placement: u32,
    decision_base: usize,
) -> Result<(), VersusClosedLoopError> {
    for player in [PlayerId::One, PlayerId::Two] {
        let index = player_index(player);
        if match_state.choices[index].is_none() {
            match_state.choices[index] =
                Some(enumerate_player_candidates(&match_state.battle, player)?);
        }
    }
    let one = selected_action(&match_state.choices[0], selections[0], decision_base)?;
    let two = selected_action(&match_state.choices[1], selections[1], decision_base + 1)?;
    match_state
        .battle
        .step_placements([one, two], frames_per_placement)?;
    match_state.choices = [None, None];
    Ok(())
}

fn match_candidate_data(
    match_state: &mut MatchState,
) -> Result<MatchCandidateData, VersusClosedLoopError> {
    let result = match_state.battle.result();
    let mut features: [Vec<[i32; VERSUS_CANDIDATE_FEATURE_COUNT]>; 2] = [Vec::new(), Vec::new()];
    let mut diagnostics: [Vec<[i32; VERSUS_CANDIDATE_DIAGNOSTIC_COUNT]>; 2] =
        [Vec::new(), Vec::new()];
    let mut state_features = [[0; VERSUS_STATE_FEATURE_COUNT]; 2];
    let mut done = [false; 2];
    let mut results = [0; 2];
    for player in [PlayerId::One, PlayerId::Two] {
        let index = player_index(player);
        state_features[index] = extract_state_features(&match_state.battle, player);
        if result == BattleResult::Ongoing && match_state.choices[index].is_none() {
            match_state.choices[index] =
                Some(enumerate_player_candidates(&match_state.battle, player)?);
        } else if result != BattleResult::Ongoing {
            match_state.choices[index] = Some(Vec::new());
        }
        let choices = match_state.choices[index]
            .as_ref()
            .expect("candidate cache initialized");
        features[index].extend(choices.iter().map(|choice| choice.features));
        diagnostics[index].extend(choices.iter().map(|choice| choice.diagnostics));
        done[index] = result != BattleResult::Ongoing || choices.is_empty();
        results[index] = result_for_player(result, player);
    }
    Ok(MatchCandidateData {
        features,
        diagnostics,
        state_features,
        done,
        results,
    })
}

fn new_match(seed: u64) -> Result<MatchState, VersusClosedLoopError> {
    let handling = PlayerHandlingProfile::normalized(8, 0, 1, SoftDropMode::Sonic);
    let rules = TetrioRulesDraft::tetra_league_beta_1_7_8_season_2()
        .try_battle_rules([handling; 2])
        .map_err(|error| VersusClosedLoopError::Profile(format!("{error:?}")))?;
    Ok(MatchState {
        battle: BattleSession::new(seed, rules)?,
        choices: [None, None],
    })
}

fn enumerate_player_candidates(
    battle: &BattleSession,
    player: PlayerId,
) -> Result<Vec<VersusChoice>, VersusClosedLoopError> {
    let own = battle.player(player);
    let opponent = battle.player(other_player(player));
    let mut sources = vec![(false, own.session().game().clone())];
    if own.session().game().hold_available() {
        let mut held = own.session().game().clone();
        let outcome = held.hold_active()?;
        if !outcome.top_out {
            sources.push((true, held));
        }
    }

    let own_pending = i32_from_u64(own.incoming().pending_lines());
    let own_ready = i32_from_u64(own.incoming().ready_lines(battle.frame()));
    let opponent_pending = i32_from_u64(opponent.incoming().pending_lines());
    let opponent_ready = i32_from_u64(opponent.incoming().ready_lines(battle.frame()));
    let opponent_summary = board_summary(opponent.session().game().board());
    let mut choices = Vec::new();

    for (used_hold, source) in sources {
        let source_piece_context = piece_context(&source);
        for placement in source.reachable_placements() {
            let preview = source.preview_reachable_placement(&placement)?;
            let locked = preview.locked;
            let analysis = analyze_afterstate(&preview.board, placement.state, locked.cleared);
            let solo = analysis.features;
            let attack = resolve_attack(
                own.attack_state(),
                locked.clear,
                AttackContext {
                    cleared_garbage: locked.cleared_garbage,
                    multiplier: battle.garbage_multiplier(),
                },
                battle.rules().attack,
            )?;
            let mut queue = own.incoming().clone();
            let cancellation = cancel_attack_packets(
                &mut queue,
                attack.packets,
                locked.pieces_placed,
                own.sent_lines(),
                battle.rules().garbage,
            )?;
            let mut features = [0_i32; VERSUS_CANDIDATE_FEATURE_COUNT];
            features[..FEATURE_COUNT].copy_from_slice(&solo);
            features[10] = i32_from_u64(attack.total_attack());
            features[11] = i32_from_u64(cancellation.outgoing.total());
            features[12] = own_pending;
            features[13] = own_ready;
            features[14] = i32_from_u64(u64::from(own.attack_state().combo));
            features[15] = i32_from_u64(u64::from(own.attack_state().back_to_back));
            features[16] = opponent_pending;
            features[17] = opponent_ready;
            features[18] = opponent_summary[1];
            features[19] = opponent_summary[2];
            features[20..30].copy_from_slice(&analysis.heights);
            features[30..40].copy_from_slice(&analysis.holes);
            features[40] = i32::from(used_hold);
            features[41..76].copy_from_slice(&source_piece_context);
            let t_spin = match locked.clear.spin {
                Some(spin) if spin.piece == PieceKind::T => match spin.classification {
                    SpinClassification::Mini => 1,
                    SpinClassification::Full => 2,
                },
                _ => 0,
            };
            let diagnostics = [
                i32::from(locked.clear.lines),
                t_spin,
                i32::from(locked.clear.perfect_clear),
                i32_from_u64(attack.total_attack()),
                i32_from_u64(cancellation.outgoing.total()),
            ];
            choices.push(VersusChoice {
                features,
                diagnostics,
                action: PlacementAction {
                    used_hold,
                    placement: placement.state,
                    last_action: placement.last_action,
                },
            });
        }
    }
    Ok(choices)
}

fn extract_state_features(
    battle: &BattleSession,
    perspective: PlayerId,
) -> [i32; VERSUS_STATE_FEATURE_COUNT] {
    let own = battle.player(perspective);
    let opponent = battle.player(other_player(perspective));
    let own_summary = board_summary(own.session().game().board());
    let opponent_summary = board_summary(opponent.session().game().board());
    let mut features = [0; VERSUS_STATE_FEATURE_COUNT];
    features[..12].copy_from_slice(&[
        own_summary[1],
        opponent_summary[1],
        own_summary[2],
        opponent_summary[2],
        i32_from_u64(own.incoming().pending_lines()),
        i32_from_u64(opponent.incoming().pending_lines()),
        i32_from_u64(own.incoming().ready_lines(battle.frame())),
        i32_from_u64(opponent.incoming().ready_lines(battle.frame())),
        i32_from_u64(u64::from(own.attack_state().combo)),
        i32_from_u64(u64::from(opponent.attack_state().combo)),
        i32_from_u64(u64::from(own.attack_state().back_to_back)),
        i32_from_u64(u64::from(opponent.attack_state().back_to_back)),
    ]);
    let own_game = own.session().game();
    let opponent_game = opponent.session().game();
    let (own_heights, own_holes) = column_heights_and_holes(own_game.board());
    let (opponent_heights, opponent_holes) = column_heights_and_holes(opponent_game.board());
    features[12..22].copy_from_slice(&own_heights);
    features[22..32].copy_from_slice(&opponent_heights);
    features[32..42].copy_from_slice(&own_holes);
    features[42..52].copy_from_slice(&opponent_holes);
    features[52..87].copy_from_slice(&piece_context(own_game));
    features[87..122].copy_from_slice(&piece_context(opponent_game));
    features
}

fn piece_context(game: &GameState) -> [i32; PIECE_CONTEXT_FEATURE_COUNT] {
    let mut features = [0; PIECE_CONTEXT_FEATURE_COUNT];
    write_piece_one_hot(&mut features[0..7], Some(game.active().kind));
    write_piece_one_hot(&mut features[7..14], game.hold());
    for (index, piece) in game.preview().into_iter().take(3).enumerate() {
        let start = 14 + index * 7;
        write_piece_one_hot(&mut features[start..start + 7], Some(piece));
    }
    features
}

fn write_piece_one_hot(target: &mut [i32], piece: Option<PieceKind>) {
    if let Some(piece) = piece {
        target[piece.index()] = 1;
    }
}

fn selected_action(
    cached: &Option<Vec<VersusChoice>>,
    selected: Option<usize>,
    decision: usize,
) -> Result<PlacementAction, VersusClosedLoopError> {
    let choices = cached
        .as_ref()
        .ok_or(VersusClosedLoopError::CandidatesNotRequested(decision))?;
    let selected = selected.ok_or(VersusClosedLoopError::MissingSelection(decision))?;
    choices.get(selected).map(|choice| choice.action).ok_or(
        VersusClosedLoopError::SelectionOutOfRange {
            decision,
            selected,
            candidates: choices.len(),
        },
    )
}

const fn player_index(player: PlayerId) -> usize {
    match player {
        PlayerId::One => 0,
        PlayerId::Two => 1,
    }
}

const fn other_player(player: PlayerId) -> PlayerId {
    match player {
        PlayerId::One => PlayerId::Two,
        PlayerId::Two => PlayerId::One,
    }
}

const fn result_for_player(result: BattleResult, player: PlayerId) -> i8 {
    match (result, player) {
        (BattleResult::PlayerOneWin, PlayerId::One)
        | (BattleResult::PlayerTwoWin, PlayerId::Two) => 1,
        (BattleResult::PlayerOneWin, PlayerId::Two)
        | (BattleResult::PlayerTwoWin, PlayerId::One) => -1,
        (BattleResult::Ongoing | BattleResult::Draw, _) => 0,
    }
}

fn i32_from_u64(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[derive(Debug)]
pub enum VersusClosedLoopError {
    EmptySeeds,
    ZeroCadence,
    SelectionCount {
        expected: usize,
        actual: usize,
    },
    CandidatesNotRequested(usize),
    MissingSelection(usize),
    SelectionForDoneMatch(usize),
    SelectionOutOfRange {
        decision: usize,
        selected: usize,
        candidates: usize,
    },
    ResetSeedCount {
        expected: usize,
        actual: usize,
    },
    HistoryCount {
        expected: usize,
        actual: usize,
    },
    HistoryAfterTerminal {
        match_index: usize,
        step: usize,
    },
    Profile(String),
    Generation(GenerationError),
    Battle(BattleError),
}

impl fmt::Display for VersusClosedLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySeeds => formatter.write_str("at least one seed is required"),
            Self::ZeroCadence => formatter.write_str("cadence must be at least one frame"),
            Self::SelectionCount { expected, actual } => {
                write!(formatter, "expected {expected} selections, got {actual}")
            }
            Self::CandidatesNotRequested(decision) => {
                write!(
                    formatter,
                    "candidates were not requested for decision {decision}"
                )
            }
            Self::MissingSelection(decision) => {
                write!(formatter, "decision {decision} needs a selection")
            }
            Self::SelectionForDoneMatch(index) => {
                write!(
                    formatter,
                    "completed match {index} must not receive a selection"
                )
            }
            Self::SelectionOutOfRange {
                decision,
                selected,
                candidates,
            } => write!(
                formatter,
                "selection {selected} is outside {candidates} candidates for decision {decision}"
            ),
            Self::ResetSeedCount { expected, actual } => {
                write!(formatter, "expected {expected} reset seeds, got {actual}")
            }
            Self::HistoryCount { expected, actual } => {
                write!(formatter, "expected {expected} histories, got {actual}")
            }
            Self::HistoryAfterTerminal { match_index, step } => write!(
                formatter,
                "history for match {match_index} continues after terminal at step {step}"
            ),
            Self::Profile(error) => write!(formatter, "rules profile activation failed: {error}"),
            Self::Generation(error) => write!(formatter, "candidate generation failed: {error}"),
            Self::Battle(error) => write!(formatter, "battle transition failed: {error}"),
        }
    }
}

impl std::error::Error for VersusClosedLoopError {}

impl From<engine_core::GameError> for VersusClosedLoopError {
    fn from(error: engine_core::GameError) -> Self {
        Self::Generation(GenerationError::from(error))
    }
}

impl From<versus::AttackError> for VersusClosedLoopError {
    fn from(error: versus::AttackError) -> Self {
        Self::Battle(BattleError::Attack {
            player: PlayerId::One,
            source: error,
        })
    }
}

impl From<versus::GarbageError> for VersusClosedLoopError {
    fn from(error: versus::GarbageError) -> Self {
        Self::Battle(BattleError::Garbage {
            player: PlayerId::One,
            source: error,
        })
    }
}

impl From<BattleError> for VersusClosedLoopError {
    fn from(error: BattleError) -> Self {
        Self::Battle(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_exposes_two_perspectives_and_steps_both_players() {
        let mut batch = VersusBatch::new(&[1, 2], 12).unwrap();
        let candidates = batch.candidates().unwrap();

        assert_eq!(candidates.offsets.len(), 5);
        assert_eq!(candidates.state_features.len(), 4);
        assert_eq!(candidates.diagnostics.len(), candidates.features.len());
        assert!(candidates.diagnostics.iter().all(|row| {
            (0..=4).contains(&row[0])
                && (0..=2).contains(&row[1])
                && (0..=1).contains(&row[2])
                && row[3] >= 0
                && row[4] >= 0
        }));
        assert!(candidates.done.iter().all(|done| !done));
        assert_eq!(candidates.state_features[0], candidates.state_features[1]);

        batch.step(&[Some(0), Some(0), Some(0), Some(0)]).unwrap();
        assert_eq!(batch.candidates().unwrap().state_features.len(), 4);
    }

    #[test]
    fn mirrored_perspectives_swap_every_state_pair() {
        let mut batch = VersusBatch::new(&[3], 12).unwrap();
        let candidates = batch.candidates().unwrap();
        let one = candidates.state_features[0];
        let two = candidates.state_features[1];

        for pair in 0..6 {
            assert_eq!(one[pair * 2], two[pair * 2 + 1]);
            assert_eq!(one[pair * 2 + 1], two[pair * 2]);
        }
        assert_eq!(&one[12..22], &two[22..32]);
        assert_eq!(&one[22..32], &two[12..22]);
        assert_eq!(&one[32..42], &two[42..52]);
        assert_eq!(&one[42..52], &two[32..42]);
        assert_eq!(&one[52..87], &two[87..122]);
        assert_eq!(&one[87..122], &two[52..87]);
    }

    #[test]
    fn action_history_restores_the_exact_candidate_batch() {
        let mut original = VersusBatch::new(&[5, 7], 12).unwrap();
        for _ in 0..3 {
            original.candidates().unwrap();
            original
                .step(&[Some(0), Some(1), Some(2), Some(3)])
                .unwrap();
        }
        let expected = original.candidates().unwrap();
        let mut restored = VersusBatch::restore(
            &[5, 7],
            &[vec![(0, 1), (0, 1), (0, 1)], vec![(2, 3), (2, 3), (2, 3)]],
            12,
        )
        .unwrap();

        assert_eq!(restored.candidates().unwrap(), expected);
    }
}
