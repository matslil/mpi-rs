use std::time::Duration;

use timeout_service::{Time, TimeoutRequest, start_timeout_service};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = start_timeout_service::<8>();
    service
        .protocol()
        .request_blocking(TimeoutRequest::new(Time::now() + Duration::from_millis(10)))?;

    println!("timeout expired");
    Ok(())
}
