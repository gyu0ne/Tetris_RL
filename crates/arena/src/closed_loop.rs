use crate::solo::{CandidateChoice, enumerate_candidates};
use crate::{FEATURE_COUNT, GenerationError, LinearTeacher};
use engine_core::{GameConfig, GameState};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateBatch {
    pub features: Vec<[i32; FEATURE_COUNT]>,
    pub offsets: Vec<usize>,
    pub done: Vec<bool>,
}

#[derive(Clone)]
pub struct SoloBatch {
    games: Vec<GameState>,
    choices: Vec<Option<Vec<CandidateChoice>>>,
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
                *cached = Some(enumerate_candidates(
                    game,
                    self.game_config,
                    LinearTeacher::dellacherie_v1(),
                )?);
            }
            let choices = cached.as_ref().expect("candidate cache was initialized");
            features.extend(choices.iter().map(|choice| choice.record.features));
            offsets.push(features.len());
            done.push(choices.is_empty());
        }

        Ok(CandidateBatch {
            features,
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
            if choice.record.action.hold {
                game.hold_active().map_err(GenerationError::from)?;
            }
            game.lock_placement(choice.placement)
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
            Self::Generation(error) => write!(formatter, "arena transition failed: {error}"),
        }
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
        assert!(candidates.done.iter().all(|done| !done));

        batch.step(&[Some(0), Some(0)]).unwrap();

        assert_eq!(batch.pieces_placed(), vec![1, 1]);
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
