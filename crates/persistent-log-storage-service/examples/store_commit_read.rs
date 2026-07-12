use std::time::{SystemTime, UNIX_EPOCH};

use persistent_log_storage_service::start_file_log_storage_service;

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
    let path = unique_path("store-commit-read");

    {
        let service = start_file_log_storage_service(&path)?;
        let index = service.store_blocking(b"commit transaction 42".to_vec())??;
        service.commit_blocking(index)??;
    }

    let service = start_file_log_storage_service(&path)?;
    let entries = service.read_blocking(None)??;
    assert_eq!(entries, vec![(0, b"commit transaction 42".to_vec())]);

    let _ = std::fs::remove_file(path);
    println!("recovered entries: {entries:?}");
    Ok(())
}
