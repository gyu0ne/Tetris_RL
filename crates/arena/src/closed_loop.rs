use crate::solo::{InferenceChoice, enumerate_candidates, enumerate_inference_candidates};
use crate::{FEATURE_COUNT, GenerationError, LinearTeacher};
use engine_core::{GameConfig, GameState, PieceKind, VISIBLE_HEIGHT};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateBatch {
    pub features: Vec<[i32; FEATURE_COUNT]>,
    pub teacher_scores: Vec<i64>,
    pub offsets: Vec<usize>,
    pub done: Vec<bool>,
}

#[derive(Clone)]
pub struct SoloBatch {
    games: Vec<GameState>,
    choices: Vec<Option<Vec<InferenceChoice>>>,
    game_config: GameConfig,
}

impl SoloBatch {
    pub fn new(seeds: &[u64]) -> Result<Self, ClosedLoopError> {
        if seeds.is_empty() {
            return Err(ClosedLoopError::EmptySeeds);
        }
        let game_config = GameConfig::default();
        let games = seeds
            .iter()
            .copied()
            .map(|seed| GameState::new(seed, game_config).map_err(GenerationError::from))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            choices: vec![None; games.len()],
            games,
            game_config,
        })
    }

    pub fn candidates(&mut self) -> Result<CandidateBatch, ClosedLoopError> {
        let mut features = Vec::new();
        let mut offsets = Vec::with_capacity(self.games.len() + 1);
        let mut done = Vec::with_capacity(self.games.len());
        offsets.push(0);

        for (game, cached) in self.games.iter().zip(&mut self.choices) {
            if game.is_top_out() {
                *cached = Some(Vec::new());
            } else if cached.is_none() {
                *cached = Some(enumerate_inference_candidates(game)?);
            }
            let choices = cached.as_ref().expect("candidate cache was initialized");
            features.extend(choices.iter().map(|choice| choice.features));
            offsets.push(features.len());
            done.push(choices.is_empty());
        }

        Ok(CandidateBatch {
            features,
            teacher_scores: Vec::new(),
            offsets,
            done,
        })
    }

    pub fn labeled_candidates(&mut self) -> Result<CandidateBatch, ClosedLoopError> {
        let mut features = Vec::new();
        let mut teacher_scores = Vec::new();
        let mut offsets = Vec::with_capacity(self.games.len() + 1);
        let mut done = Vec::with_capacity(self.games.len());
        offsets.push(0);

        for (game, cached) in self.games.iter().zip(&mut self.choices) {
            if game.is_top_out() {
                *cached = Some(Vec::new());
            } else {
                let labeled =
                    enumerate_candidates(game, self.game_config, LinearTeacher::dellacherie_v1())?;
                features.extend(labeled.iter().map(|choice| choice.record.features));
                teacher_scores.extend(labeled.iter().map(|choice| choice.record.teacher_score));
                *cached = Some(
                    labeled
                        .into_iter()
                        .map(|choice| InferenceChoice {
                            features: choice.record.features,
                            placement: choice.placement,
                            used_hold: choice.record.action.hold,
                            last_action: choice.last_action,
                        })
                        .collect(),
                );
            }
            let choices = cached.as_ref().expect("candidate cache was initialized");
            offsets.push(features.len());
            done.push(choices.is_empty());
        }

        Ok(CandidateBatch {
            features,
            teacher_scores,
            offsets,
            done,
        })
    }

    pub fn step(&mut self, selections: &[Option<usize>]) -> Result<(), ClosedLoopError> {
        if selections.len() != self.games.len() {
            return Err(ClosedLoopError::SelectionCount {
                expected: self.games.len(),
                actual: selections.len(),
            });
        }

        for (index, ((game, cached), selection)) in self
            .games
            .iter_mut()
            .zip(&mut self.choices)
            .zip(selections)
            .enumerate()
        {
            let choices = cached
                .as_ref()
                .ok_or(ClosedLoopError::CandidatesNotRequested(index))?;
            if choices.is_empty() {
                if selection.is_some() {
                    return Err(ClosedLoopError::SelectionForDoneGame(index));
                }
                continue;
            }
            let selected = selection.ok_or(ClosedLoopError::MissingSelection(index))?;
            let choice =
                choices
                    .get(selected)
                    .cloned()
                    .ok_or(ClosedLoopError::SelectionOutOfRange {
                        game: index,
                        selected,
                        candidates: choices.len(),
                    })?;
            if choice.used_hold {
                game.hold_active().map_err(GenerationError::from)?;
            }
            game.lock_placement_with_action(choice.placement, choice.last_action)
                .map_err(GenerationError::from)?;
            *cached = None;
        }
        Ok(())
    }

    pub fn pieces_placed(&self) -> Vec<u64> {
        self.games.iter().map(GameState::pieces_placed).collect()
    }

    pub fn game_count(&self) -> usize {
        self.games.len()
    }

    pub fn snapshot(&self, index: usize) -> Result<SoloSnapshot, ClosedLoopError> {
        let game = self
            .games
            .get(index)
            .ok_or(ClosedLoopError::GameOutOfRange {
                selected: index,
                games: self.games.len(),
            })?;
        Ok(SoloSnapshot {
            board_rows: game.board().rows()[..VISIBLE_HEIGHT].to_vec(),
            garbage_rows: game.board().garbage_rows()[..VISIBLE_HEIGHT].to_vec(),
            active: piece_name(game.active().kind).to_owned(),
            hold: game.hold().map(|kind| piece_name(kind).to_owned()),
            preview: game
                .preview()
                .into_iter()
                .map(|kind| piece_name(kind).to_owned())
                .collect(),
            pieces_placed: game.pieces_placed(),
            top_out: game.is_top_out(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoloSnapshot {
    pub board_rows: Vec<u16>,
    pub garbage_rows: Vec<u16>,
    pub active: String,
    pub hold: Option<String>,
    pub preview: Vec<String>,
    pub pieces_placed: u64,
    pub top_out: bool,
}

#[derive(Debug)]
pub enum ClosedLoopError {
    EmptySeeds,
    SelectionCount {
        expected: usize,
        actual: usize,
    },
    CandidatesNotRequested(usize),
    MissingSelection(usize),
    SelectionForDoneGame(usize),
    SelectionOutOfRange {
        game: usize,
        selected: usize,
        candidates: usize,
    },
    GameOutOfRange {
        selected: usize,
        games: usize,
    },
    Generation(GenerationError),
}

impl fmt::Display for ClosedLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySeeds => formatter.write_str("at least one seed is required"),
            Self::SelectionCount { expected, actual } => {
                write!(formatter, "expected {expected} selections, got {actual}")
            }
            Self::CandidatesNotRequested(game) => {
                write!(formatter, "candidates were not requested for game {game}")
            }
            Self::MissingSelection(game) => write!(formatter, "game {game} needs a selection"),
            Self::SelectionForDoneGame(game) => {
                write!(formatter, "done game {game} must not receive a selection")
            }
            Self::SelectionOutOfRange {
                game,
                selected,
                candidates,
            } => write!(
                formatter,
                "selection {selected} is outside {candidates} candidates for game {game}"
            ),
            Self::GameOutOfRange { selected, games } => {
                write!(formatter, "game {selected} is outside {games} games")
            }
            Self::Generation(error) => write!(formatter, "arena transition failed: {error}"),
        }
    }
}

const fn piece_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::I => "I",
        PieceKind::O => "O",
        PieceKind::T => "T",
        PieceKind::S => "S",
        PieceKind::Z => "Z",
        PieceKind::J => "J",
        PieceKind::L => "L",
    }
}

impl std::error::Error for ClosedLoopError {}

impl From<GenerationError> for ClosedLoopError {
    fn from(error: GenerationError) -> Self {
        Self::Generation(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_exposes_flat_features_and_applies_one_choice_per_game() {
        let mut batch = SoloBatch::new(&[1, 2]).unwrap();
        let candidates = batch.candidates().unwrap();

        assert_eq!(candidates.offsets.len(), 3);
        assert_eq!(candidates.offsets[0], 0);
        assert_eq!(candidates.offsets[2], candidates.features.len());
        assert!(candidates.teacher_scores.is_empty());
        assert!(candidates.done.iter().all(|done| !done));

        batch.step(&[Some(0), Some(0)]).unwrap();

        assert_eq!(batch.pieces_placed(), vec![1, 1]);
    }

    #[test]
    fn labeled_batch_retains_teacher_scores_for_aggregation() {
        let mut batch = SoloBatch::new(&[3]).unwrap();
        let candidates = batch.labeled_candidates().unwrap();

        assert_eq!(candidates.teacher_scores.len(), candidates.features.len());
        batch.step(&[Some(0)]).unwrap();
        assert_eq!(batch.pieces_placed(), vec![1]);
    }

    #[test]
    fn inference_batch_preserves_labeled_candidate_order_and_features() {
        let mut inference = SoloBatch::new(&[11, 12]).unwrap();
        let mut labeled = SoloBatch::new(&[11, 12]).unwrap();

        let inference_candidates = inference.candidates().unwrap();
        let labeled_candidates = labeled.labeled_candidates().unwrap();

        assert_eq!(inference_candidates.features, labeled_candidates.features);
        assert_eq!(inference_candidates.offsets, labeled_candidates.offsets);
        assert_eq!(inference_candidates.done, labeled_candidates.done);
    }

    #[test]
    fn snapshot_exposes_visible_board_and_queue() {
        let batch = SoloBatch::new(&[5]).unwrap();
        let snapshot = batch.snapshot(0).unwrap();

        assert_eq!(snapshot.board_rows.len(), VISIBLE_HEIGHT);
        assert_eq!(snapshot.garbage_rows.len(), VISIBLE_HEIGHT);
        assert_eq!(snapshot.preview.len(), GameConfig::default().preview);
        assert_eq!(snapshot.pieces_placed, 0);
        assert!(!snapshot.top_out);
    }

    #[test]
    fn step_requires_candidates_and_one_selection_per_active_game() {
        let mut batch = SoloBatch::new(&[7]).unwrap();
        assert!(matches!(
            batch.step(&[Some(0)]),
            Err(ClosedLoopError::CandidatesNotRequested(0))
        ));
        batch.candidates().unwrap();
        assert!(matches!(
            batch.step(&[None]),
            Err(ClosedLoopError::MissingSelection(0))
        ));
    }
}
