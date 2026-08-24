use crate::{ClearedLines, PieceKind, SpinOutcome};

/// Score-free description of the mechanics produced by one locked piece.
///
/// Versus rules consume this event to calculate attacks. Solo test sessions
/// can inspect it without carrying a 40 LINES or BLITZ scoring system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearEvent {
    pub piece: PieceKind,
    pub lines: u8,
    pub spin: Option<SpinOutcome>,
    pub perfect_clear: bool,
}

impl ClearEvent {
    pub const fn new(
        piece: PieceKind,
        lines: u8,
        spin: Option<SpinOutcome>,
        perfect_clear: bool,
    ) -> Self {
        Self {
            piece,
            lines,
            spin,
            perfect_clear,
        }
    }

    pub const fn from_lock(
        piece: PieceKind,
        cleared: ClearedLines,
        spin: Option<SpinOutcome>,
        perfect_clear: bool,
    ) -> Self {
        Self::new(piece, cleared.count(), spin, perfect_clear)
    }

    pub const fn cleared_any(self) -> bool {
        self.lines > 0
    }
}

#[cfg(test)]
mod tests {
    use super::ClearEvent;
    use crate::PieceKind;

    #[test]
    fn event_is_score_free_and_reports_only_transition_facts() {
        let event = ClearEvent::new(PieceKind::I, 4, None, true);

        assert_eq!(event.lines, 4);
        assert!(event.cleared_any());
        assert!(event.perfect_clear);
    }
}
