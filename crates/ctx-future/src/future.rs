use crate::CtxPoll;

/// A future-like computation that borrows context only during resume.
///
/// `CtxFuture` is intentionally simpler than [`std::future::Future`]: the
/// caller owns the context and passes it into each resume step. A pending
/// implementation stores its continuation state in `self`, not in a retained
/// mutable borrow of `Cx`.
pub trait CtxFuture<Cx, Input = ()> {
    /// Value produced when the computation completes.
    type Output;

    /// Resume the computation with temporary mutable access to context.
    fn resume(&mut self, cx: &mut Cx, input: Input) -> CtxPoll<Self::Output>;
}

/// Create a [`CtxFuture`] from a resume closure.
pub fn resume_fn<F>(f: F) -> ResumeFn<F> {
    ResumeFn::new(f)
}

/// [`CtxFuture`] adapter returned by [`resume_fn`].
#[derive(Clone, Debug)]
pub struct ResumeFn<F> {
    f: F,
}

impl<F> ResumeFn<F> {
    /// Wrap a resume closure.
    #[must_use]
    pub const fn new(f: F) -> Self {
        Self { f }
    }
}

impl<Cx, Input, Output, F> CtxFuture<Cx, Input> for ResumeFn<F>
where
    F: FnMut(&mut Cx, Input) -> CtxPoll<Output>,
{
    type Output = Output;

    fn resume(&mut self, cx: &mut Cx, input: Input) -> CtxPoll<Self::Output> {
        (self.f)(cx, input)
    }
}

impl<Cx, Input, F> CtxFuture<Cx, Input> for &mut F
where
    F: CtxFuture<Cx, Input> + ?Sized,
{
    type Output = F::Output;

    fn resume(&mut self, cx: &mut Cx, input: Input) -> CtxPoll<Self::Output> {
        (**self).resume(cx, input)
    }
}

impl<Cx, Input, F> CtxFuture<Cx, Input> for Box<F>
where
    F: CtxFuture<Cx, Input> + ?Sized,
{
    type Output = F::Output;

    fn resume(&mut self, cx: &mut Cx, input: Input) -> CtxPoll<Self::Output> {
        (**self).resume(cx, input)
    }
}
