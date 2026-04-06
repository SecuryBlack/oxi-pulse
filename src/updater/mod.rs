use std::time::Duration;
use tracing::{error, info, warn};

const GITHUB_OWNER: &str = "securyblack";
const GITHUB_REPO: &str = "oxi-pulse";
const CHECK_INTERVAL: Duration = Duration::from_secs(86_400); // 24 hours
const STARTUP_DELAY: Duration = Duration::from_secs(300);    // 5 minutes

/// Spawn a background task that checks for a new release 5 minutes after
/// startup and then once every 24 hours.
/// If a newer version is found and downloaded, the process exits cleanly
/// so the OS service manager (systemd / Windows Service) restarts it
/// with the new binary.
pub fn start_daily_check() {
    tokio::spawn(async move {
        // Short delay on startup so the agent is fully initialised before
        // hitting the network, but short enough that a restart picks up
        // a pending update within minutes.
        tokio::time::sleep(STARTUP_DELAY).await;

        loop {
            info!("checking for updates…");
            match tokio::task::spawn_blocking(check_and_update).await {
                Ok(Ok(updated)) => {
                    if updated {
                        info!("update applied — exiting for service restart");
                        std::process::exit(0);
                    } else {
                        info!("already on latest version");
                    }
                }
                Ok(Err(e)) => warn!("update check failed: {}", e),
                Err(e) => error!("update task panicked: {}", e),
            }

            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Blocking: query GitHub Releases, compare versions, download and replace
/// the binary if a newer version is available.
/// Returns `true` if the binary was replaced (caller should exit).
fn check_and_update() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let current = env!("CARGO_PKG_VERSION");
    let target = self_update::get_target();

    let status = self_update::backends::github::Update::configure()
        .repo_owner(GITHUB_OWNER)
        .repo_name(GITHUB_REPO)
        .bin_name("oxipulse")
        .target(&target)
        .current_version(current)
        .build()?
        .update()?;

    Ok(status.updated())
}
