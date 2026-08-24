use crate::LockVisibility;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopOutReason {
    BlockOut,
    LockOut,
    PartialLockOut,
}

/// Profile-controlled lock-out variants. Block-out is always authoritative
/// because a colliding active piece cannot enter the timing kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopOutRules {
    pub lock_out: bool,
    pub partial_lock_out: bool,
}

impl TopOutRules {
    pub const fn block_out_only() -> Self {
        Self {
            lock_out: false,
            partial_lock_out: false,
        }
    }

    pub const fn with_lock_out(lock_out: bool, partial_lock_out: bool) -> Self {
        Self {
            lock_out,
            partial_lock_out,
        }
    }

    pub const fn lock_reason(self, visibility: LockVisibility) -> Option<TopOutReason> {
        match visibility {
            LockVisibility::FullyHidden if self.lock_out => Some(TopOutReason::LockOut),
            LockVisibility::PartiallyHidden if self.partial_lock_out => {
                Some(TopOutReason::PartialLockOut)
            }
            LockVisibility::Visible
            | LockVisibility::PartiallyHidden
            | LockVisibility::FullyHidden => None,
        }
    }
}

impl Default for TopOutRules {
    fn default() -> Self {
        Self::block_out_only()
    }
}

#[cfg(test)]
mod tests {
    use super::{TopOutReason, TopOutRules};
    use crate::LockVisibility;

    #[test]
    fn block_out_only_does_not_guess_lock_out_variants() {
        let rules = TopOutRules::block_out_only();
        assert_eq!(rules.lock_reason(LockVisibility::FullyHidden), None);
        assert_eq!(rules.lock_reason(LockVisibility::PartiallyHidden), None);
    }

    #[test]
    fn lock_out_variants_are_independently_configurable() {
        let rules = TopOutRules::with_lock_out(true, true);
        assert_eq!(
            rules.lock_reason(LockVisibility::FullyHidden),
            Some(TopOutReason::LockOut)
        );
        assert_eq!(
            rules.lock_reason(LockVisibility::PartiallyHidden),
            Some(TopOutReason::PartialLockOut)
        );
        assert_eq!(rules.lock_reason(LockVisibility::Visible), None);
    }
}
