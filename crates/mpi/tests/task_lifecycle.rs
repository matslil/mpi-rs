use mpi::{CallError, Response, TaskTermination, task};
use std::sync::mpsc;

#[derive(Default)]
struct FailingTask;

#[task(queue_size = 8)]
impl FailingTask {
    #[event]
    fn panic_now(_ctx: &mut FailingTaskContext) {
        panic!("isolated task panic");
    }
}

#[derive(Default)]
struct Supervisor;

#[task(queue_size = 8)]
impl Supervisor {
    #[event]
    fn observe(
        ctx: &mut SupervisorContext,
        target: FailingTaskHandle,
        subscribed: mpsc::Sender<()>,
        observed: mpsc::Sender<TaskTermination>,
    ) {
        let monitor = target.supervise(ctx);
        subscribed.send(()).unwrap();
        observed.send(monitor.await).unwrap();
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

#[derive(Default)]
struct CallObserver;

#[task(queue_size = 8, receives(Response<u32>))]
impl CallObserver {
    #[event]
    fn observe(
        ctx: &mut CallObserverContext,
        target: FailingCallTaskHandle,
        observed: mpsc::Sender<CallError>,
    ) {
        observed.send(target.fail(ctx).await.unwrap_err()).unwrap();
    }
}

#[derive(Default)]
struct StreamObserver;

#[task(queue_size = 8, receives(mpi::StreamEvent<u32, String>))]
impl StreamObserver {
    #[event]
    fn observe(
        ctx: &mut StreamObserverContext,
        target: FailingStreamTaskHandle,
        observed: mpsc::Sender<mpi::StreamError<String>>,
    ) {
        let mut stream = target.fail(ctx).unwrap();
        observed.send(stream.next(ctx).await.unwrap_err()).unwrap();
    }
}

#[test]
fn mpi_req_136_137_140_141_143_144_task_panic_is_isolated_and_supervised() {
    let (target, target_runtime) = FailingTask::spawn(FailingTask).unwrap();
    let (supervisor, supervisor_runtime) = Supervisor::spawn(Supervisor).unwrap();
    let (subscribed_tx, subscribed_rx) = mpsc::channel();
    let (observed_tx, observed_rx) = mpsc::channel();

    supervisor
        .observe_blocking(target.clone(), subscribed_tx, observed_tx)
        .unwrap();
    subscribed_rx.recv().unwrap();
    target.panic_now_blocking().unwrap();

    let expected = TaskTermination::Panicked {
        message: Some("isolated task panic".to_owned()),
    };
    assert_eq!(observed_rx.recv().unwrap(), expected);
    assert_eq!(target.termination(), Some(expected.clone()));
    assert_eq!(target_runtime.join().unwrap_err().termination(), &expected);

    supervisor.stop_blocking().unwrap();
    supervisor_runtime.join().unwrap();
}

#[test]
fn mpi_req_139_active_call_reports_target_termination() {
    let (target, target_runtime) = FailingCallTask::spawn(FailingCallTask).unwrap();
    let (observer, observer_runtime) = CallObserver::spawn(CallObserver).unwrap();
    let (observed_tx, observed_rx) = mpsc::channel();

    observer
        .observe_blocking(target.clone(), observed_tx)
        .unwrap();

    let error = observed_rx.recv().unwrap();
    assert_eq!(
        error,
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
    let (observer, observer_runtime) = StreamObserver::spawn(StreamObserver).unwrap();
    let (observed_tx, observed_rx) = mpsc::channel();

    observer
        .observe_blocking(target.clone(), observed_tx)
        .unwrap();

    assert_eq!(
        observed_rx.recv().unwrap(),
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
