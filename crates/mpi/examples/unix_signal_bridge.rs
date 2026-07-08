#[cfg(all(unix, feature = "unix-signals"))]
use mpi::task;

#[cfg(all(unix, feature = "unix-signals"))]
#[derive(Default)]
struct SignalTask {
    last_signal: Option<i32>,
    count: u32,
}

#[cfg(all(unix, feature = "unix-signals"))]
#[task(queue_size = 8)]
impl SignalTask {
    #[start]
    fn start(_ctx: &mut SignalTaskContext) {}

    #[event(priority)]
    fn signal(ctx: &mut SignalTaskContext, signal: i32) {
        ctx.with_state(|state| {
            state.last_signal = Some(signal);
            state.count += 1;
        });
    }

    #[call]
    fn snapshot(ctx: &mut SignalTaskContext) -> (Option<i32>, u32) {
        ctx.with_state(|state| (state.last_signal, state.count))
    }

    #[event(priority)]
    fn stop(ctx: &mut SignalTaskContext) {
        ctx.stop();
    }
}

#[cfg(all(unix, feature = "unix-signals"))]
fn main() {
    use signal_hook::consts::signal::SIGUSR1;

    let (task, runtime) = SignalTask::spawn(SignalTask::default()).unwrap();
    let bridge = mpi::forward_signals(task.clone(), [SIGUSR1], |signal| {
        SignalTaskMessage::Signal { signal }
    })
    .expect("signal bridge starts");

    signal_hook::low_level::raise(SIGUSR1).expect("example signal can be raised");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut snapshot = task.snapshot_blocking().unwrap();
    while snapshot.1 == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
        snapshot = task.snapshot_blocking().unwrap();
    }

    assert_eq!(snapshot, (Some(SIGUSR1), 1));

    bridge.join().unwrap();
    task.stop_blocking().unwrap();
    runtime.join().unwrap();
}

#[cfg(not(all(unix, feature = "unix-signals")))]
fn main() {
    println!("unix_signal_bridge requires a Unix target with the mpi unix-signals feature");
}
