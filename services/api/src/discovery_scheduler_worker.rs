use std::{env, time::Duration};

use tokio::time::sleep;
use tracing::{info, warn};

use crate::{cmdb::discovery::run_scheduled_jobs_once, error::AppResult, state::AppState};

pub fn start(state: AppState) {
    if !scheduler_enabled() {
        info!("discovery scheduler worker is disabled by env");
        return;
    }

    tokio::spawn(async move {
        if let Err(err) = run_loop(state).await {
            warn!(error = %err, "discovery scheduler worker terminated unexpectedly");
        }
    });
}

async fn run_loop(state: AppState) -> AppResult<()> {
    let poll_interval = Duration::from_secs(poll_interval_seconds());
    info!(
        poll_seconds = poll_interval.as_secs(),
        "discovery scheduler worker started"
    );

    loop {
        match run_scheduled_jobs_once(&state.db).await {
            Ok(processed) => {
                if processed > 0 {
                    info!(
                        processed_jobs = processed,
                        "discovery scheduler processed jobs"
                    );
                }
            }
            Err(err) => {
                warn!(error = %err, "discovery scheduler tick failed");
            }
        }
        sleep(poll_interval).await;
    }
}

fn scheduler_enabled() -> bool {
    env::var("DISCOVERY_SCHEDULER_ENABLED")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

fn poll_interval_seconds() -> u64 {
    env::var("DISCOVERY_SCHEDULER_POLL_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|value| value.clamp(5, 3600))
        .unwrap_or(30)
}
