use std::time::{SystemTime, UNIX_EPOCH};

use mpi::{EndpointId, TransactionDecision, TransactionId, TransactionPath};
use mpi_transaction::TransactionDecisionLog;
use persistent_log_storage_service::start_file_log_storage_service;

fn unique_path(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mpi-tx-example-{name}-{}-{nanos}.log",
        std::process::id()
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = unique_path("record-and-recover");
    let root = TransactionPath::root(TransactionId::new(EndpointId(7), 1));
    let child = root.child(10);

    {
        let storage = start_file_log_storage_service(&path)?;
        let log = TransactionDecisionLog::new(storage);
        log.record_decision(&root, TransactionDecision::Commit)?;
        log.record_decision(&child, TransactionDecision::Abort)?;
    }

    let storage = start_file_log_storage_service(&path)?;
    let log = TransactionDecisionLog::new(storage);
    let decisions = log.recover_decisions()?;

    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0].path, root);
    assert_eq!(decisions[0].decision, TransactionDecision::Commit);
    assert_eq!(decisions[1].path, child);
    assert_eq!(decisions[1].decision, TransactionDecision::Abort);

    let _ = std::fs::remove_file(path);
    println!("recovered decisions: {decisions:?}");
    Ok(())
}
