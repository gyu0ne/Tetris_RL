use crate::{Board, PieceState, RotationDirection, try_rotate};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Geometry-level inputs. Timing-aware reachability is layered above this core.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Movement {
    Down,
    Left,
    Right,
    RotateClockwise,
    RotateCounterclockwise,
    RotateHalf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeometricPlacement {
    pub state: PieceState,
    pub path: Vec<Movement>,
}

/// Enumerates every grounded state reachable by successful unit translations
/// and ordered SRS+ rotations. This does not yet enforce gravity, DAS, lock
/// delay, or frame budgets, so it must not be used as final TL reachability.
pub fn reachable_locks(board: &Board, spawn: PieceState) -> Vec<GeometricPlacement> {
    if board.collides(spawn) {
        return Vec::new();
    }

    let mut frontier = VecDeque::from([(spawn, Vec::new())]);
    let mut visited = BTreeSet::from([spawn]);
    let mut placements = BTreeMap::<PieceState, Vec<Movement>>::new();

    while let Some((state, path)) = frontier.pop_front() {
        if try_movement(board, state, Movement::Down).is_none() {
            placements.entry(state).or_insert_with(|| path.clone());
        }

        for movement in Movement::ALL {
            let Some(next) = try_movement(board, state, movement) else {
                continue;
            };
            if visited.insert(next) {
                let mut next_path = path.clone();
                next_path.push(movement);
                frontier.push_back((next, next_path));
            }
        }
    }

    placements
        .into_iter()
        .map(|(state, path)| GeometricPlacement { state, path })
        .collect()
}

impl Movement {
    pub const ALL: [Self; 6] = [
        Self::Down,
        Self::Left,
        Self::Right,
        Self::RotateClockwise,
        Self::RotateCounterclockwise,
        Self::RotateHalf,
    ];
}

pub fn try_movement(board: &Board, state: PieceState, movement: Movement) -> Option<PieceState> {
    match movement {
        Movement::Down => try_translation(board, state, 0, -1),
        Movement::Left => try_translation(board, state, -1, 0),
        Movement::Right => try_translation(board, state, 1, 0),
        Movement::RotateClockwise => {
            try_rotate(board, state, RotationDirection::Clockwise).map(|r| r.state)
        }
        Movement::RotateCounterclockwise => {
            try_rotate(board, state, RotationDirection::Counterclockwise).map(|r| r.state)
        }
        Movement::RotateHalf => try_rotate(board, state, RotationDirection::Half).map(|r| r.state),
    }
}

pub fn hard_drop(board: &Board, state: PieceState) -> Option<PieceState> {
    if board.collides(state) {
        return None;
    }
    let mut dropped = state;
    while let Some(next) = try_translation(board, dropped, 0, -1) {
        dropped = next;
    }
    Some(dropped)
}

fn try_translation(board: &Board, state: PieceState, dx: i16, dy: i16) -> Option<PieceState> {
    let candidate = state.translated(dx, dy);
    (!board.collides(candidate)).then_some(candidate)
}

#[cfg(test)]
mod tests {
    use super::{hard_drop, reachable_locks, try_movement};
    use crate::{Board, Movement, Orientation, PieceKind, PieceState, SpawnRules};
    use std::collections::BTreeSet;

    #[test]
    fn hard_drop_reaches_floor() {
        let board = Board::empty();
        let spawn = SpawnRules::modern_observed().piece(PieceKind::T);
        let dropped = hard_drop(&board, spawn).expect("spawn is valid");
        assert_eq!(dropped.y, -1);
        assert!(try_movement(&board, dropped, Movement::Down).is_none());
    }

    #[test]
    fn empty_board_has_all_t_placements() {
        let board = Board::empty();
        let spawn = PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18);
        let placements = reachable_locks(&board, spawn);
        let states = placements.iter().map(|p| p.state).collect::<BTreeSet<_>>();

        assert_eq!(placements.len(), states.len());
        assert_eq!(placements.len(), 34);
        assert!(
            placements
                .iter()
                .all(|p| try_movement(&board, p.state, Movement::Down).is_none())
        );
    }

    #[test]
    fn every_recorded_path_replays() {
        let board = Board::empty();
        let spawn = PieceState::new(PieceKind::L, Orientation::Spawn, 3, 18);
        for placement in reachable_locks(&board, spawn) {
            let replayed = placement.path.iter().try_fold(spawn, |piece, movement| {
                try_movement(&board, piece, *movement)
            });
            assert_eq!(replayed, Some(placement.state));
        }
    }
}
