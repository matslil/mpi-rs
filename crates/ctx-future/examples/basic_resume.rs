use ctx_future::{CtxFuture, CtxPoll};

struct WaitOnce {
    resumed_before: bool,
}

impl CtxFuture<Vec<&'static str>> for WaitOnce {
    type Output = usize;

    fn resume(&mut self, cx: &mut Vec<&'static str>, (): ()) -> CtxPoll<Self::Output> {
        cx.push("resumed");

        if self.resumed_before {
            CtxPoll::Ready(cx.len())
        } else {
            self.resumed_before = true;
            CtxPoll::Pending
        }
    }
}

fn main() {
    let mut cx = Vec::new();
    let mut future = WaitOnce {
        resumed_before: false,
    };

    assert_eq!(future.resume(&mut cx, ()), CtxPoll::Pending);

    cx.push("used by scheduler");

    assert_eq!(future.resume(&mut cx, ()), CtxPoll::Ready(3));
    println!("context entries: {cx:?}");
}
