#[derive(Debug, Clone, Copy)]
pub enum SandboxProfile {
    /// Network process: allow network + DNS. Deny filesystem, deny new processes.
    Network,
    /// Renderer process: deny network, deny filesystem, minimal syscalls.
    Renderer,
}

#[derive(Debug)]
pub enum SandboxResult {
    /// All sandbox layers applied successfully.
    Applied { layers: Vec<String> },
    /// Some layers applied, others unavailable.
    Partial {
        applied: Vec<String>,
        unavailable: Vec<String>,
    },
    /// Sandboxing not available on this platform.
    Unavailable,
}

pub fn apply_sandbox(profile: SandboxProfile) -> anyhow::Result<SandboxResult> {
    #[cfg(target_os = "linux")]
    {
        super::sandbox_linux::apply(profile)
    }

    #[cfg(target_os = "macos")]
    {
        super::sandbox_macos::apply(profile)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        tracing::warn!("sandboxing not available on this platform");
        Ok(SandboxResult::Unavailable)
    }
}
