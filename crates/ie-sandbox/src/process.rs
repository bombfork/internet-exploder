use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use crate::channel::IpcChannel;
use crate::message::IpcMessage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessKind {
    Browser,
    Renderer,
    Network,
}

impl ProcessKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessKind::Browser => "browser",
            ProcessKind::Renderer => "renderer",
            ProcessKind::Network => "network",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "browser" => Some(Self::Browser),
            "renderer" => Some(Self::Renderer),
            "network" => Some(Self::Network),
            _ => None,
        }
    }
}

pub struct ChildHandle {
    process: tokio::process::Child,
    channel: IpcChannel,
    kind: ProcessKind,
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

impl ChildHandle {
    pub fn channel(&mut self) -> &mut IpcChannel {
        &mut self.channel
    }

    pub fn kind(&self) -> ProcessKind {
        self.kind
    }

    pub fn process_id(&self) -> u32 {
        self.process.id().unwrap_or(0)
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }

    /// Graceful shutdown: send Shutdown, wait, kill if timeout.
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        let _ = self.channel.send(&IpcMessage::Shutdown).await;
        match timeout(SHUTDOWN_TIMEOUT, self.process.wait()).await {
            Ok(Ok(status)) => {
                tracing::info!("{:?} process exited with status: {}", self.kind, status);
                Ok(())
            }
            Ok(Err(e)) => {
                tracing::error!("{:?} process wait error: {e}", self.kind);
                Err(e.into())
            }
            Err(_) => {
                tracing::warn!(
                    "{:?} process did not exit within {:?}, killing",
                    self.kind,
                    SHUTDOWN_TIMEOUT
                );
                self.process.kill().await?;
                Ok(())
            }
        }
    }

    pub async fn kill(&mut self) -> anyhow::Result<()> {
        self.process.kill().await?;
        Ok(())
    }
}

impl Drop for ChildHandle {
    fn drop(&mut self) {
        // Best-effort kill to avoid zombies
        let _ = self.process.start_kill();
    }
}

/// Spawn a child process that communicates via IPC.
/// Uses the current executable by default.
#[cfg(unix)]
pub async fn spawn_child(kind: ProcessKind) -> anyhow::Result<ChildHandle> {
    spawn_child_with_exe(kind, std::env::current_exe()?).await
}

/// Spawn a child process using a specific executable path.
#[cfg(unix)]
pub async fn spawn_child_with_exe(
    kind: ProcessKind,
    exe_path: std::path::PathBuf,
) -> anyhow::Result<ChildHandle> {
    let (parent_channel, child_fd) = IpcChannel::pair_for_spawn()?;

    let mut cmd = Command::new(exe_path);
    cmd.arg("--subprocess-kind").arg(kind.as_str());
    cmd.env("IE_IPC_FD", child_fd.to_string());
    // Inherit stderr for child tracing output, null stdin/stdout
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::inherit());

    // Ensure child fd survives exec (clear CLOEXEC was done in pair_for_spawn)
    // After spawn, close child fd in parent
    unsafe {
        cmd.pre_exec(move || {
            // The fd is already non-CLOEXEC from pair_for_spawn, nothing more needed
            Ok(())
        });
    }

    let process = cmd.spawn()?;

    // Close the child fd in the parent process
    unsafe {
        libc::close(child_fd);
    }

    Ok(ChildHandle {
        process,
        channel: parent_channel,
        kind,
    })
}

// Spawn tests live in crates/ie-shell/tests/subprocess.rs because
// spawn_child re-executes the binary which needs --subprocess-kind CLI support.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_kind_round_trip() {
        for kind in [
            ProcessKind::Browser,
            ProcessKind::Renderer,
            ProcessKind::Network,
        ] {
            assert_eq!(ProcessKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(ProcessKind::parse("invalid"), None);
    }
}
