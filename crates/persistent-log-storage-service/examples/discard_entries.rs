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
    let path = unique_path("discard");
    let service = start_file_log_storage_service(&path)?;

    let first = service.store_blocking(b"prepared".to_vec())??;
    let second = service.store_blocking(b"committed".to_vec())??;
    service.commit_blocking(second)??;
    service.discard_blocking(first)??;

    let entries = service.read_blocking(None)??;
    assert_eq!(entries, vec![(1, b"committed".to_vec())]);

    let _ = std::fs::remove_file(path);
    println!("entries after discard: {entries:?}");
    Ok(())
}
