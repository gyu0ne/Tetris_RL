use crate::{Board, LastAction, PieceState, RotationDirection, try_rotate};
use std::collections::{BTreeMap, VecDeque};

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
    pub last_action: LastAction,
}

/// Enumerates every grounded state reachable by successful unit translations
/// and ordered SRS+ rotations. This does not yet enforce gravity, DAS, lock
/// delay, or frame budgets, so it must not be used as final TL reachability.
pub fn reachable_locks(board: &Board, spawn: PieceState) -> Vec<GeometricPlacement> {
    if board.collides(spawn) {
        return Vec::new();
    }

    #[derive(Clone, Copy)]
    struct SearchNode {
        state: PieceState,
        parent: Option<usize>,
        movement: Option<Movement>,
    }

    #[derive(Clone, Copy)]
    struct PathEnd {
        node: usize,
        final_movement: Option<Movement>,
        last_action: LastAction,
    }

    let mut nodes = vec![SearchNode {
        state: spawn,
        parent: None,
        movement: None,
    }];
    let mut frontier = VecDeque::from([0_usize]);
    let mut visited = BTreeMap::from([(spawn, 0_usize)]);
    let mut placements = BTreeMap::<PieceState, PathEnd>::new();
    let mut rotation_finishes = BTreeMap::<PieceState, PathEnd>::new();

    while let Some(node_index) = frontier.pop_front() {
        let node = nodes[node_index];
        let state = node.state;
        if try_movement(board, state, Movement::Down).is_none() {
            placements.entry(state).or_insert(PathEnd {
                node: node_index,
                final_movement: None,
                last_action: last_action_for_movement(node.movement),
            });
        }

        for movement in Movement::ALL {
            let Some((next, last_action)) = try_movement_with_action(board, state, movement) else {
                continue;
            };
            if matches!(last_action, LastAction::Rotation { .. })
                && try_movement(board, next, Movement::Down).is_none()
            {
                rotation_finishes.entry(next).or_insert(PathEnd {
                    node: node_index,
                    final_movement: Some(movement),
                    last_action,
                });
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = visited.entry(next) {
                let next_index = nodes.len();
                entry.insert(next_index);
                nodes.push(SearchNode {
                    state: next,
                    parent: Some(node_index),
                    movement: Some(movement),
                });
                frontier.push_back(next_index);
            }
        }
    }

    for (state, rotation_path_end) in rotation_finishes {
        if placements.contains_key(&state) {
            placements.insert(state, rotation_path_end);
        }
    }

    placements
        .into_iter()
        .map(|(state, path_end)| {
            let mut path = Vec::new();
            let mut cursor = Some(path_end.node);
            while let Some(index) = cursor {
                let node = nodes[index];
                if let Some(movement) = node.movement {
                    path.push(movement);
                }
                cursor = node.parent;
            }
            path.reverse();
            if let Some(movement) = path_end.final_movement {
                path.push(movement);
            }
            GeometricPlacement {
                state,
                path,
                last_action: path_end.last_action,
            }
        })
        .collect()
}

fn last_action_for_movement(movement: Option<Movement>) -> LastAction {
    match movement {
        None => LastAction::None,
        Some(Movement::Down | Movement::Left | Movement::Right) => LastAction::Translation,
        Some(
            Movement::RotateClockwise | Movement::RotateCounterclockwise | Movement::RotateHalf,
        ) => LastAction::None,
    }
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
    try_movement_with_action(board, state, movement).map(|(state, _)| state)
}

fn try_movement_with_action(
    board: &Board,
    state: PieceState,
    movement: Movement,
) -> Option<(PieceState, LastAction)> {
    match movement {
        Movement::Down => {
            try_translation(board, state, 0, -1).map(|state| (state, LastAction::Translation))
        }
        Movement::Left => {
            try_translation(board, state, -1, 0).map(|state| (state, LastAction::Translation))
        }
        Movement::Right => {
            try_translation(board, state, 1, 0).map(|state| (state, LastAction::Translation))
        }
        Movement::RotateClockwise => {
            rotation_with_action(board, state, RotationDirection::Clockwise)
        }
        Movement::RotateCounterclockwise => {
            rotation_with_action(board, state, RotationDirection::Counterclockwise)
        }
        Movement::RotateHalf => rotation_with_action(board, state, RotationDirection::Half),
    }
}

fn rotation_with_action(
    board: &Board,
    state: PieceState,
    direction: RotationDirection,
) -> Option<(PieceState, LastAction)> {
    try_rotate(board, state, direction).map(|rotation| {
        (
            rotation.state,
            LastAction::Rotation {
                direction,
                kick_index: rotation.kick_index,
            },
        )
    })
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
    use super::{hard_drop, reachable_locks, try_movement, try_movement_with_action};
    use crate::{
        Board, GeometricPlacement, HEIGHT, LastAction, Movement, Orientation, PieceKind,
        PieceState, SpawnRules,
    };
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

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

    #[test]
    fn grounded_rotation_destinations_retain_rotation_provenance() {
        let board = Board::empty();
        let spawn = PieceState::new(PieceKind::T, Orientation::Spawn, 3, 18);
        let placements = reachable_locks(&board, spawn);

        assert!(
            placements.iter().any(|placement| matches!(
                placement.last_action,
                crate::LastAction::Rotation { .. }
            ))
        );
    }

    #[test]
    fn compact_search_matches_the_previous_path_enumerator() {
        let mut state = 0x9e37_79b9_u32;
        for case in 0..16 {
            let mut rows = [0_u16; HEIGHT];
            for row in rows.iter_mut().take(8) {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *row = ((state >> 16) as u16) & 0x03ff;
                if *row == 0x03ff {
                    *row ^= 1_u16 << (case % 10);
                }
            }
            let board = Board::from_rows(rows).unwrap();
            for kind in PieceKind::ALL {
                let spawn = SpawnRules::modern_observed().piece(kind);
                assert_eq!(
                    reachable_locks(&board, spawn),
                    legacy_reachable_locks(&board, spawn)
                );
            }
        }
    }

    fn legacy_reachable_locks(board: &Board, spawn: PieceState) -> Vec<GeometricPlacement> {
        if board.collides(spawn) {
            return Vec::new();
        }
        let mut frontier = VecDeque::from([(spawn, Vec::new())]);
        let mut visited = BTreeSet::from([spawn]);
        let mut placements = BTreeMap::<PieceState, (Vec<Movement>, LastAction)>::new();
        let mut rotation_finishes = BTreeMap::<PieceState, (Vec<Movement>, LastAction)>::new();
        while let Some((piece, path)) = frontier.pop_front() {
            if try_movement(board, piece, Movement::Down).is_none() {
                placements
                    .entry(piece)
                    .or_insert_with(|| (path.clone(), legacy_last_action(&path)));
            }
            for movement in Movement::ALL {
                let Some((next, last_action)) = try_movement_with_action(board, piece, movement)
                else {
                    continue;
                };
                let mut next_path = path.clone();
                next_path.push(movement);
                if matches!(last_action, LastAction::Rotation { .. })
                    && try_movement(board, next, Movement::Down).is_none()
                {
                    rotation_finishes
                        .entry(next)
                        .or_insert_with(|| (next_path.clone(), last_action));
                }
                if visited.insert(next) {
                    frontier.push_back((next, next_path));
                }
            }
        }
        for (piece, rotation_path) in rotation_finishes {
            if placements.contains_key(&piece) {
                placements.insert(piece, rotation_path);
            }
        }
        placements
            .into_iter()
            .map(|(piece, (path, last_action))| GeometricPlacement {
                state: piece,
                path,
                last_action,
            })
            .collect()
    }

    fn legacy_last_action(path: &[Movement]) -> LastAction {
        match path.last() {
            None => LastAction::None,
            Some(Movement::Down | Movement::Left | Movement::Right) => LastAction::Translation,
            Some(
                Movement::RotateClockwise | Movement::RotateCounterclockwise | Movement::RotateHalf,
            ) => LastAction::None,
        }
    }
}
