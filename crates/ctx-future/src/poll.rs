/// Completion state returned by a [`CtxFuture`](crate::CtxFuture) resume step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CtxPoll<T> {
    /// The computation is suspended and may be resumed later.
    Pending,

    /// The computation has completed with a value.
    Ready(T),
}

impl<T> CtxPoll<T> {
    /// Return whether this value is [`CtxPoll::Pending`].
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Return whether this value is [`CtxPoll::Ready`].
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    /// Map a ready value while preserving pending state.
    pub fn map<U, F>(self, f: F) -> CtxPoll<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Pending => CtxPoll::Pending,
            Self::Ready(value) => CtxPoll::Ready(f(value)),
        }
    }

    /// Convert this value into `Some(T)` when ready.
    pub fn into_ready(self) -> Option<T> {
        match self {
            Self::Pending => None,
            Self::Ready(value) => Some(value),
        }
    }
}
