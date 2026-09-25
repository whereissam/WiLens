use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Run a command to completion, killing it if it exceeds `timeout`. `label`
/// names the tool in error messages.
pub fn run_with_timeout(
    command: &mut Command,
    timeout: Duration,
    label: &str,
) -> Result<Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to run {label}: {error}"))?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{label} did not finish within {} seconds.",
                    timeout.as_secs()
                ));
            }
            Err(error) => return Err(format!("Failed to wait for {label}: {error}")),
        }
    }

    child
        .wait_with_output()
        .map_err(|error| format!("Failed to read {label} output: {error}"))
}
