use mpi::{CallError, Response, TaskTerminated, TaskTermination, task};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for task state"
        );
        std::thread::yield_now();
    }
}

#[derive(Default)]
struct FailingTask;

#[task(queue_size = 8)]
impl FailingTask {
    #[event]
    fn panic_now(_ctx: &mut FailingTaskContext) {
        panic!("isolated task panic");
    }
}

struct Supervisor {
    monitors: Vec<mpi::TaskMonitor>,
    subscribed: Arc<AtomicBool>,
    observed: Arc<Mutex<Vec<TaskTerminated>>>,
}

#[task(queue_size = 8)]
impl Supervisor {
    #[event]
    fn observe(ctx: &mut SupervisorContext, target: FailingTaskHandle) {
        let monitor = target.supervise(ctx);
        ctx.with_state(|state| {
            state.monitors.push(monitor);
            state.subscribed.store(true, Ordering::Release);
        });
    }

    #[task_terminated]
    fn task_terminated(ctx: &mut SupervisorContext, event: TaskTerminated) {
        ctx.with_state(|state| state.observed.lock().unwrap().push(event));
    }
}

#[derive(Default)]
struct FailingCallTask;

#[task(queue_size = 8)]
impl FailingCallTask {
    #[call]
    fn fail(_ctx: &mut FailingCallTaskContext) -> u32 {
        panic!("call handler panic");
    }
}

#[derive(Default)]
struct FailingStreamTask;

#[task(queue_size = 8)]
impl FailingStreamTask {
    #[stream(item = u32, error = String, batch_size = 1)]
    fn fail(
        _ctx: &mut FailingStreamTaskContext,
        _out: &mut mpi::BoxStreamSink<u32, String>,
    ) -> Result<(), String> {
        panic!("stream handler panic");
    }
}

struct CallObserver {
    observed: Arc<Mutex<Option<CallError>>>,
}

#[task(queue_size = 8, receives(Response<u32>))]
impl CallObserver {
    #[event]
    fn observe(ctx: &mut CallObserverContext, target: FailingCallTaskHandle) {
        let result = target.fail(ctx).await.unwrap_err();
        ctx.with_state(|state| *state.observed.lock().unwrap() = Some(result));
    }
}

struct StreamObserver {
    observed: Arc<Mutex<Option<mpi::StreamError<String>>>>,
}

#[task(queue_size = 8, receives(mpi::StreamEvent<u32, String>))]
impl StreamObserver {
    #[event]
    fn observe(ctx: &mut StreamObserverContext, target: FailingStreamTaskHandle) {
        let mut stream = target.fail(ctx).unwrap();
        let result = stream.next(ctx).await.unwrap_err();
        ctx.with_state(|state| *state.observed.lock().unwrap() = Some(result));
    }
}

#[test]
fn mpi_req_136_137_140_141_143_144_task_panic_is_isolated_and_supervised() {
    let (target, target_runtime) = FailingTask::spawn(FailingTask).unwrap();
    let subscribed = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let (supervisor, supervisor_runtime) = Supervisor::spawn(Supervisor {
        monitors: Vec::new(),
        subscribed: Arc::clone(&subscribed),
        observed: Arc::clone(&observed),
    })
    .unwrap();

    supervisor.observe_blocking(target.clone()).unwrap();
    wait_until(|| subscribed.load(Ordering::Acquire));
    target.panic_now_blocking().unwrap();

    wait_until(|| !observed.lock().unwrap().is_empty());
    let event = observed.lock().unwrap()[0].clone();
    let expected = TaskTermination::Panicked {
        message: Some("isolated task panic".to_owned()),
    };
    assert_eq!(event.target, target.endpoint());
    assert_eq!(event.termination, expected);
    assert!(event.supervised);
    assert_eq!(target.termination(), Some(expected.clone()));
    assert_eq!(target_runtime.join().unwrap_err().termination(), &expected);

    supervisor.stop_blocking().unwrap();
    supervisor_runtime.join().unwrap();
}

#[test]
fn mpi_req_139_active_call_reports_target_termination() {
    let (target, target_runtime) = FailingCallTask::spawn(FailingCallTask).unwrap();
    let observed = Arc::new(Mutex::new(None));
    let (observer, observer_runtime) = CallObserver::spawn(CallObserver {
        observed: Arc::clone(&observed),
    })
    .unwrap();

    observer.observe_blocking(target.clone()).unwrap();
    wait_until(|| observed.lock().unwrap().is_some());
    assert_eq!(
        observed.lock().unwrap().clone().unwrap(),
        CallError::TargetTerminated(TaskTermination::Panicked {
            message: Some("call handler panic".to_owned()),
        })
    );
    assert!(matches!(
        target_runtime.join().unwrap_err().termination(),
        TaskTermination::Panicked { .. }
    ));

    observer.stop_blocking().unwrap();
    observer_runtime.join().unwrap();
}

#[test]
fn mpi_req_139_active_stream_reports_target_termination() {
    let (target, target_runtime) = FailingStreamTask::spawn(FailingStreamTask).unwrap();
    let observed = Arc::new(Mutex::new(None));
    let (observer, observer_runtime) = StreamObserver::spawn(StreamObserver {
        observed: Arc::clone(&observed),
    })
    .unwrap();

    observer.observe_blocking(target.clone()).unwrap();
    wait_until(|| observed.lock().unwrap().is_some());
    assert_eq!(
        observed.lock().unwrap().clone().unwrap(),
        mpi::StreamError::TargetTerminated(TaskTermination::Panicked {
            message: Some("stream handler panic".to_owned()),
        })
    );
    assert!(matches!(
        target_runtime.join().unwrap_err().termination(),
        TaskTermination::Panicked { .. }
    ));

    observer.stop_blocking().unwrap();
    observer_runtime.join().unwrap();
}
