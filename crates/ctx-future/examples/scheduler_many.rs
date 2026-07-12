use ctx_future::{CtxFuture, CtxPoll};

#[derive(Default)]
struct SchedulerContext {
    ticks: u32,
    log: Vec<&'static str>,
}

struct Delay {
    name: &'static str,
    remaining: u32,
}

impl CtxFuture<SchedulerContext> for Delay {
    type Output = &'static str;

    fn resume(&mut self, cx: &mut SchedulerContext, (): ()) -> CtxPoll<Self::Output> {
        cx.log.push(self.name);

        if self.remaining == 0 {
            CtxPoll::Ready(self.name)
        } else {
            self.remaining -= 1;
            CtxPoll::Pending
        }
    }
}

fn main() {
    let mut cx = SchedulerContext::default();
    let mut jobs: Vec<Option<Box<dyn CtxFuture<SchedulerContext, Output = &'static str>>>> = vec![
        Some(Box::new(Delay {
            name: "fast",
            remaining: 0,
        })),
        Some(Box::new(Delay {
            name: "slow",
            remaining: 2,
        })),
    ];
    let mut completed = Vec::new();

    while completed.len() < jobs.len() {
        cx.ticks += 1;

        for job in &mut jobs {
            let Some(future) = job else {
                continue;
            };

            if let CtxPoll::Ready(name) = future.resume(&mut cx, ()) {
                completed.push(name);
                *job = None;
            }
        }
    }

    assert_eq!(completed, ["fast", "slow"]);
    assert_eq!(cx.ticks, 3);
    println!("completed {completed:?} in {} ticks", cx.ticks);
}
