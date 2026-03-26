use std::ffi::{CStr, CString, c_char, c_int};

use anyhow::Result;

use crate::sandbox::{SandboxProfile, SandboxResult};

extern "C" {
    fn sandbox_init(profile: *const c_char, flags: u64, errorbuf: *mut *mut c_char) -> c_int;
    fn sandbox_free_error(errorbuf: *mut c_char);
}

pub fn apply(profile: SandboxProfile) -> Result<SandboxResult> {
    let sbpl = match profile {
        SandboxProfile::Network => NETWORK_SBPL,
        SandboxProfile::Renderer => RENDERER_SBPL,
    };

    apply_profile(sbpl)?;
    Ok(SandboxResult::Applied {
        layers: vec!["sandbox_init".to_string()],
    })
}

fn apply_profile(sbpl: &str) -> Result<()> {
    let c_profile = CString::new(sbpl)?;
    let mut err: *mut c_char = std::ptr::null_mut();
    let ret = unsafe { sandbox_init(c_profile.as_ptr(), 0, &mut err) };
    if ret != 0 {
        let msg = if !err.is_null() {
            let s = unsafe { CStr::from_ptr(err) }.to_string_lossy().to_string();
            unsafe { sandbox_free_error(err) };
            s
        } else {
            "unknown error".to_string()
        };
        anyhow::bail!("sandbox_init failed: {msg}");
    }
    Ok(())
}

const NETWORK_SBPL: &str = r#"(version 1)
(deny default)
(allow network*)
(allow system-socket)
(allow sysctl-read)
(allow mach-lookup
  (global-name "com.apple.system.logger")
  (global-name "com.apple.SystemConfiguration.configd")
  (global-name "com.apple.dnssd.service"))
(allow process-info-pidinfo (target self))
"#;

const RENDERER_SBPL: &str = r#"(version 1)
(deny default)
(allow sysctl-read)
(allow mach-lookup
  (global-name "com.apple.system.logger"))
(allow process-info-pidinfo (target self))
"#;
