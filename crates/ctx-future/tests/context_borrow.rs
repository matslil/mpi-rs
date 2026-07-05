use ctx_future::{CtxFuture, CtxPoll, resume_fn};

#[derive(Default)]
struct TestContext {
    log: Vec<&'static str>,
    total: i32,
}

#[derive(Default)]
struct TwoStep {
    state: u8,
    first_input: i32,
}

impl CtxFuture<TestContext, i32> for TwoStep {
    type Output = i32;

    fn resume(&mut self, cx: &mut TestContext, input: i32) -> CtxPoll<Self::Output> {
        match self.state {
            0 => {
                cx.log.push("first-resume");
                self.first_input = input;
                self.state = 1;
                CtxPoll::Pending
            }
            1 => {
                cx.log.push("second-resume");
                self.state = 2;
                CtxPoll::Ready(cx.total + self.first_input + input)
            }
            _ => panic!("future resumed after completion"),
        }
    }
}

#[test]
fn ctx_req_001_ctx_req_002_context_is_returned_after_pending() {
    let mut cx = TestContext::default();
    let mut future = TwoStep::default();

    assert_eq!(future.resume(&mut cx, 4), CtxPoll::Pending);

    cx.total = 10;
    cx.log.push("caller-used-context");

    assert_eq!(future.resume(&mut cx, 5), CtxPoll::Ready(19));
    assert_eq!(
        cx.log,
        ["first-resume", "caller-used-context", "second-resume"]
    );
}

#[test]
fn ctx_req_002_multiple_pending_futures_do_not_hold_context_borrows() {
    let mut cx = TestContext::default();
    let mut futures: Vec<Box<dyn CtxFuture<TestContext, i32, Output = i32>>> = vec![
        Box::<TwoStep>::default(),
        Box::<TwoStep>::default(),
        Box::<TwoStep>::default(),
    ];

    for (index, future) in futures.iter_mut().enumerate() {
        assert_eq!(future.resume(&mut cx, index as i32), CtxPoll::Pending);
        cx.total += 1;
    }

    cx.log.push("context-used-while-all-pending");

    assert_eq!(futures[2].resume(&mut cx, 10), CtxPoll::Ready(15));
    assert_eq!(futures[0].resume(&mut cx, 20), CtxPoll::Ready(23));
}

#[test]
fn ctx_req_003_pending_future_can_be_resumed_later_with_context() {
    let mut seen_first_resume = false;
    let mut future = resume_fn(
        |cx: &mut Vec<&'static str>, input: &'static str| -> CtxPoll<usize> {
            cx.push(input);

            if seen_first_resume {
                CtxPoll::Ready(cx.len())
            } else {
                seen_first_resume = true;
                CtxPoll::Pending
            }
        },
    );
    let mut cx = Vec::new();

    assert!(future.resume(&mut cx, "first").is_pending());
    cx.push("between");

    assert_eq!(future.resume(&mut cx, "second"), CtxPoll::Ready(3));
    assert_eq!(cx, ["first", "between", "second"]);
}

#[test]
fn req_152_ctx_future_is_standalone_public_api() {
    fn drive_once<F>(future: &mut F, cx: &mut usize) -> CtxPoll<usize>
    where
        F: CtxFuture<usize, Output = usize>,
    {
        future.resume(cx, ())
    }

    let mut future = resume_fn(|cx: &mut usize, (): ()| {
        *cx += 1;
        CtxPoll::Ready(*cx)
    });
    let mut cx = 0;

    assert_eq!(drive_once(&mut future, &mut cx), CtxPoll::Ready(1));
}
