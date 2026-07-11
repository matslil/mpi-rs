use std::time::{SystemTime, UNIX_EPOCH};

use persistent_log_storage_service::start_file_log_storage_service;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct DecisionPayload {
    transaction: u64,
    decision: String,
}

fn unique_path(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "pls-example-{name}-{}-{nanos}.log",
        std::process::id()
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = unique_path("typed");
    let service = start_file_log_storage_service(&path)?;
    let payload = DecisionPayload {
        transaction: 42,
        decision: "commit".to_string(),
    };

    let index = service.store_serialized_blocking(&payload)??;
    service.commit_blocking(index)??;

    let recovered = service.read_serialized_blocking::<DecisionPayload>(None)??;
    assert_eq!(recovered, vec![(0, payload)]);

    let _ = std::fs::remove_file(path);
    println!("typed recovery succeeded");
    Ok(())
}
