use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn cargo_check_fixture(name: &str, source: &str) -> std::process::Output {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut dir = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_nanos();
    dir.push(format!(
        "mpi_scope_compile_fail_{}_{}_{}",
        std::process::id(),
        unique,
        name
    ));

    fs::create_dir_all(dir.join("src")).expect("create temporary fixture directory");
    let dep_path = manifest_dir.to_string_lossy().replace('\\', "\\\\");
    let workspace_lock = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("mpi crate should be inside the workspace")
        .join("Cargo.lock");
    fs::copy(workspace_lock, dir.join("Cargo.lock")).expect("copy workspace lockfile");
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "mpi-scope-compile-fail-{name}"
version = "0.0.0"
edition = "2024"

[dependencies]
mpi = {{ path = "{dep_path}" }}
"#
        ),
    )
    .expect("write temporary fixture manifest");
    fs::write(dir.join("src/main.rs"), source).expect("write temporary fixture source");

    let output = Command::new("cargo")
        .arg("check")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .output()
        .expect("run cargo check for compile-fail fixture");

    let _ = fs::remove_dir_all(&dir);
    output
}

fn assert_fails_task_scope(name: &str, source: &str, expected_method: &str) {
    let output = cargo_check_fixture(name, source);
    assert!(
        !output.status.success(),
        "fixture `{name}` unexpectedly compiled successfully"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("TaskScope"),
        "fixture `{name}` did not fail because of TaskScope; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(expected_method),
        "fixture `{name}` did not mention `{expected_method}`; stderr:\n{stderr}"
    );
}

fn assert_fails_contains(name: &str, source: &str, expected: &[&str]) {
    let output = cargo_check_fixture(name, source);
    assert!(
        !output.status.success(),
        "fixture `{name}` unexpectedly compiled successfully"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected_fragment in expected {
        assert!(
            stderr.contains(expected_fragment),
            "fixture `{name}` did not mention `{expected_fragment}`; stderr:\n{stderr}"
        );
    }
}

#[test]
fn req_027_event_api_rejects_non_task_scope_context() {
    assert_fails_task_scope(
        "event",
        r#"
use mpi::task;

#[derive(Default)]
struct Counter;

#[task(queue_size = 4)]
impl Counter {
    #[start]
    async fn start(&mut self, _ctx: &mut CounterContext) {}

    #[event]
    async fn add(&mut self, _ctx: &mut CounterContext, _amount: u32) {}

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

fn main() {
    let (counter, runtime) = Counter::spawn(Counter::default()).unwrap();
    let mut outside_task = ();
    let _ = counter.add(&mut outside_task, 1);
    counter.stop_blocking().unwrap();
    runtime.join().unwrap();
}
"#,
        "add",
    );
}

#[test]
fn req_120_req_121_call_api_rejects_non_task_scope_context() {
    assert_fails_task_scope(
        "call",
        r#"
use mpi::task;

#[derive(Default)]
struct Counter;

#[task(queue_size = 4)]
impl Counter {
    #[start]
    async fn start(&mut self, _ctx: &mut CounterContext) {}

    #[call(reply = u32)]
    async fn get(&mut self, _ctx: &mut CounterContext) -> u32 {
        1
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

fn main() {
    let (counter, runtime) = Counter::spawn(Counter::default()).unwrap();
    let mut outside_task = ();
    let _ = counter.get(&mut outside_task);
    counter.stop_blocking().unwrap();
    runtime.join().unwrap();
}
"#,
        "get",
    );
}

#[test]
fn req_070_nonprotocol_call_rejects_missing_receive_declaration() {
    assert_fails_contains(
        "nonprotocol_call_receive",
        r#"
use mpi::task;

#[derive(Default)]
struct Counter;

#[task(queue_size = 4)]
impl Counter {
    #[start]
    async fn start(&mut self, _ctx: &mut CounterContext) {}

    #[call(reply = u32)]
    async fn get(&mut self, _ctx: &mut CounterContext) -> u32 {
        1
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Client;

#[task(queue_size = 4)]
impl Client {
    #[start]
    async fn start(&mut self, _ctx: &mut ClientContext) {}

    #[event]
    async fn ask(&mut self, ctx: &mut ClientContext, counter: CounterHandle) {
        let _reply = counter.get(ctx).await.unwrap();
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ClientContext) {
        ctx.stop();
    }
}

fn main() {}
"#,
        &["CanReceive", "Response"],
    );
}

#[test]
fn req_101_req_121_stream_api_rejects_non_task_scope_context() {
    assert_fails_task_scope(
        "stream",
        r#"
use mpi::task;

#[derive(Default)]
struct Producer;

#[task(queue_size = 4)]
impl Producer {
    #[start]
    async fn start(&mut self, _ctx: &mut ProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 2)]
    async fn numbers(
        &mut self,
        _ctx: &mut ProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
        count: u32,
    ) -> Result<(), String> {
        for value in 0..count {
            out.push(value).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ProducerContext) {
        ctx.stop();
    }
}

fn main() {
    let (producer, runtime) = Producer::spawn(Producer).unwrap();
    let mut outside_task = ();
    let _ = producer.numbers(&mut outside_task, 3);
    producer.stop_blocking().unwrap();
    runtime.join().unwrap();
}
"#,
        "numbers",
    );
}

#[test]
fn req_071_nonprotocol_stream_rejects_missing_receive_declaration() {
    assert_fails_contains(
        "nonprotocol_stream_receive",
        r#"
use mpi::task;

#[derive(Default)]
struct Producer;

#[task(queue_size = 4)]
impl Producer {
    #[start]
    async fn start(&mut self, _ctx: &mut ProducerContext) {}

    #[stream(item = u32, error = String, batch_size = 2)]
    async fn numbers(
        &mut self,
        _ctx: &mut ProducerContext,
        out: &mut mpi::BoxStreamSink<u32, String>,
    ) -> Result<(), String> {
        out.push(1).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ProducerContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Client;

#[task(queue_size = 4)]
impl Client {
    #[start]
    async fn start(&mut self, _ctx: &mut ClientContext) {}

    #[event]
    async fn ask(&mut self, ctx: &mut ClientContext, producer: ProducerHandle) {
        let mut stream = producer.numbers(ctx).unwrap();
        let _ = stream.next(ctx).await.unwrap();
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ClientContext) {
        ctx.stop();
    }
}

fn main() {}
"#,
        &["CanReceive", "StreamEvent"],
    );
}

#[test]
fn req_070_req_168_protocol_call_rejects_missing_receive_declaration() {
    assert_fails_contains(
        "protocol_call_receive",
        r#"
use mpi::{protocol, task};

#[derive(Clone)]
struct GetRequest;

#[derive(Clone)]
struct GetReply;

protocol! {
    pub protocol CounterProtocolV1 {
        call Get(GetRequest) -> GetReply;
    }
}

#[derive(Default)]
struct Counter;

#[task(queue_size = 4)]
impl Counter {
    #[start]
    async fn start(&mut self, _ctx: &mut CounterContext) {}

    #[call(protocol = CounterProtocolV1::Get, reply = GetReply)]
    async fn get(&mut self, _ctx: &mut CounterContext, _request: GetRequest) -> GetReply {
        GetReply
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Client;

#[task(queue_size = 4)]
impl Client {
    #[start]
    async fn start(&mut self, _ctx: &mut ClientContext) {}

    #[event]
    async fn ask(
        &mut self,
        ctx: &mut ClientContext,
        counter: CounterProtocolV1::Binding<CounterHandle>,
    ) {
        let _reply = counter.get(ctx, GetRequest).await.unwrap();
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ClientContext) {
        ctx.stop();
    }
}

fn main() {}
"#,
        &["CanReceive", "CounterProtocolV1", "Reply"],
    );
}

#[test]
fn req_168_protocol_call_receive_declaration_rejects_wrong_protocol_identity() {
    assert_fails_contains(
        "protocol_call_receive_identity",
        r#"
use mpi::{protocol, task};

#[derive(Clone)]
struct GetRequest;

#[derive(Clone)]
struct GetReply;

protocol! {
    pub protocol CounterProtocolV1 {
        call Get(GetRequest) -> GetReply;
    }
}

protocol! {
    pub protocol OtherCounterProtocolV1 {
        call Get(GetRequest) -> GetReply;
    }
}

#[derive(Default)]
struct Counter;

#[task(queue_size = 4)]
impl Counter {
    #[start]
    async fn start(&mut self, _ctx: &mut CounterContext) {}

    #[call(protocol = OtherCounterProtocolV1::Get, reply = GetReply)]
    async fn get(&mut self, _ctx: &mut CounterContext, _request: GetRequest) -> GetReply {
        GetReply
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut CounterContext) {
        ctx.stop();
    }
}

#[derive(Default)]
struct Client;

#[task(queue_size = 4, receives(CounterProtocolV1::Get::Reply))]
impl Client {
    #[start]
    async fn start(&mut self, _ctx: &mut ClientContext) {}

    #[event]
    async fn ask(
        &mut self,
        ctx: &mut ClientContext,
        counter: OtherCounterProtocolV1::Binding<CounterHandle>,
    ) {
        let _reply = counter.get(ctx, GetRequest).await.unwrap();
    }

    #[event(priority)]
    async fn stop(&mut self, ctx: &mut ClientContext) {
        ctx.stop();
    }
}

fn main() {}
"#,
        &["CanReceive", "OtherCounterProtocolV1", "Reply"],
    );
}
