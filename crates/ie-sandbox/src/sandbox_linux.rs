use std::collections::BTreeMap;

use anyhow::Result;
use landlock::{
    ABI, Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
};
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule};

use crate::sandbox::{SandboxProfile, SandboxResult};

pub fn apply(profile: SandboxProfile) -> Result<SandboxResult> {
    let mut applied = Vec::new();
    let mut unavailable = Vec::new();

    // 1. Apply landlock (filesystem restriction)
    match apply_landlock(profile) {
        Ok(true) => applied.push("landlock".to_string()),
        Ok(false) => {
            tracing::warn!("landlock not supported on this kernel");
            unavailable.push("landlock".to_string());
        }
        Err(e) => {
            tracing::warn!("landlock failed: {e}");
            unavailable.push(format!("landlock: {e}"));
        }
    }

    // 2. Apply seccomp (syscall restriction)
    match apply_seccomp(profile) {
        Ok(()) => applied.push("seccomp".to_string()),
        Err(e) => {
            tracing::warn!("seccomp failed: {e}");
            unavailable.push(format!("seccomp: {e}"));
        }
    }

    if unavailable.is_empty() {
        Ok(SandboxResult::Applied { layers: applied })
    } else if applied.is_empty() {
        tracing::error!("no sandbox layers applied");
        Ok(SandboxResult::Unavailable)
    } else {
        Ok(SandboxResult::Partial {
            applied,
            unavailable,
        })
    }
}

fn apply_landlock(profile: SandboxProfile) -> Result<bool> {
    let abi = ABI::V1;

    let ruleset = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))?
        .create()?;

    // Network profile gets read access to TLS cert paths for HTTPS
    let ruleset = match profile {
        SandboxProfile::Network => {
            let mut rs = ruleset;
            // Allow read access to system TLS certificates
            let cert_paths = [
                "/etc/ssl/certs",
                "/etc/pki/tls/certs",
                "/usr/share/ca-certificates",
                "/etc/ca-certificates",
            ];
            for path in &cert_paths {
                if std::path::Path::new(path).exists()
                    && let Ok(fd) = PathFd::new(path)
                {
                    rs = rs.add_rule(PathBeneath::new(fd, AccessFs::from_read(abi)))?;
                }
            }
            // Allow read on DNS/network config files
            for path in [
                "/etc/resolv.conf",
                "/etc/nsswitch.conf",
                "/etc/hosts",
                "/etc/gai.conf",
                "/etc/host.conf",
                "/etc/ld.so.cache",
            ] {
                if let Ok(fd) = PathFd::new(path) {
                    rs = rs.add_rule(PathBeneath::new(fd, AccessFs::from_read(abi)))?;
                }
            }
            // Allow read on system libraries (glibc NSS modules for DNS resolution)
            for path in ["/usr/lib", "/lib", "/lib64"] {
                if std::path::Path::new(path).exists()
                    && let Ok(fd) = PathFd::new(path)
                {
                    rs = rs.add_rule(PathBeneath::new(fd, AccessFs::from_read(abi)))?;
                }
            }
            rs
        }
        SandboxProfile::Renderer => {
            // No filesystem access at all
            ruleset
        }
    };

    ruleset.restrict_self()?;
    Ok(true)
}

fn apply_seccomp(profile: SandboxProfile) -> Result<()> {
    // PR_SET_NO_NEW_PRIVS is required before seccomp
    let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if ret != 0 {
        anyhow::bail!(
            "prctl(PR_SET_NO_NEW_PRIVS) failed: {}",
            std::io::Error::last_os_error()
        );
    }

    let rules = match profile {
        SandboxProfile::Network => network_seccomp_rules(),
        SandboxProfile::Renderer => renderer_seccomp_rules(),
    };

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Errno(libc::EPERM as u32),
        SeccompAction::Allow,
        std::env::consts::ARCH.try_into()?,
    )?;

    let bpf: BpfProgram = filter.try_into()?;
    seccompiler::apply_filter(&bpf)?;
    Ok(())
}

fn network_seccomp_rules() -> BTreeMap<i64, Vec<SeccompRule>> {
    let mut rules = BTreeMap::new();
    let allowed_syscalls: Vec<i64> = vec![
        // Network I/O
        libc::SYS_socket,
        libc::SYS_connect,
        libc::SYS_bind,
        libc::SYS_listen,
        libc::SYS_accept,
        libc::SYS_accept4,
        libc::SYS_sendto,
        libc::SYS_recvfrom,
        libc::SYS_sendmsg,
        libc::SYS_recvmsg,
        libc::SYS_setsockopt,
        libc::SYS_getsockopt,
        libc::SYS_getpeername,
        libc::SYS_getsockname,
        libc::SYS_shutdown,
        // I/O (IPC socket + files)
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_readv,
        libc::SYS_writev,
        libc::SYS_pread64,
        libc::SYS_pwrite64,
        // Filesystem (limited — for TLS certs via landlock)
        libc::SYS_openat,
        libc::SYS_close,
        libc::SYS_fstat,
        libc::SYS_newfstatat,
        libc::SYS_lseek,
        libc::SYS_getdents64,
        libc::SYS_statx,
        libc::SYS_access,
        libc::SYS_faccessat2,
        // Polling/events
        libc::SYS_poll,
        libc::SYS_ppoll,
        libc::SYS_pselect6,
        libc::SYS_epoll_create1,
        libc::SYS_epoll_ctl,
        libc::SYS_epoll_wait,
        libc::SYS_epoll_pwait,
        libc::SYS_eventfd2,
        libc::SYS_timerfd_create,
        libc::SYS_timerfd_settime,
        libc::SYS_timerfd_gettime,
        // Memory management
        libc::SYS_mmap,
        libc::SYS_munmap,
        libc::SYS_mprotect,
        libc::SYS_mremap,
        libc::SYS_brk,
        libc::SYS_madvise,
        // Thread management
        libc::SYS_clone3,
        libc::SYS_clone,
        libc::SYS_futex,
        libc::SYS_set_robust_list,
        libc::SYS_get_robust_list,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_sched_getaffinity,
        libc::SYS_sched_yield,
        // Time
        libc::SYS_clock_gettime,
        libc::SYS_clock_nanosleep,
        libc::SYS_nanosleep,
        libc::SYS_gettimeofday,
        // Misc required
        libc::SYS_dup,
        libc::SYS_dup2,
        libc::SYS_fcntl,
        libc::SYS_ioctl,
        libc::SYS_getrandom,
        libc::SYS_getpid,
        libc::SYS_gettid,
        libc::SYS_getuid,
        libc::SYS_getgid,
        libc::SYS_geteuid,
        libc::SYS_getegid,
        libc::SYS_exit,
        libc::SYS_exit_group,
        libc::SYS_prctl,
        libc::SYS_rseq,
        libc::SYS_pipe2,
        libc::SYS_dup3,
    ];

    for syscall in allowed_syscalls {
        rules.insert(syscall, vec![]);
    }
    rules
}

fn renderer_seccomp_rules() -> BTreeMap<i64, Vec<SeccompRule>> {
    let mut rules = BTreeMap::new();
    let allowed_syscalls: Vec<i64> = vec![
        // Minimal I/O (IPC socket only, no new sockets)
        libc::SYS_read,
        libc::SYS_write,
        libc::SYS_readv,
        libc::SYS_writev,
        libc::SYS_pread64,
        libc::SYS_pwrite64,
        libc::SYS_recvfrom,
        libc::SYS_sendto,
        libc::SYS_recvmsg,
        libc::SYS_sendmsg,
        // Polling
        libc::SYS_poll,
        libc::SYS_ppoll,
        libc::SYS_epoll_create1,
        libc::SYS_epoll_ctl,
        libc::SYS_epoll_wait,
        libc::SYS_epoll_pwait,
        libc::SYS_eventfd2,
        // Memory
        libc::SYS_mmap,
        libc::SYS_munmap,
        libc::SYS_mprotect,
        libc::SYS_mremap,
        libc::SYS_brk,
        libc::SYS_madvise,
        // Threads
        libc::SYS_clone3,
        libc::SYS_clone,
        libc::SYS_futex,
        libc::SYS_set_robust_list,
        libc::SYS_get_robust_list,
        libc::SYS_rt_sigaction,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigreturn,
        libc::SYS_sigaltstack,
        libc::SYS_sched_getaffinity,
        libc::SYS_sched_yield,
        // Time
        libc::SYS_clock_gettime,
        libc::SYS_clock_nanosleep,
        libc::SYS_nanosleep,
        // Misc
        libc::SYS_close,
        libc::SYS_dup,
        libc::SYS_dup2,
        libc::SYS_dup3,
        libc::SYS_fcntl,
        libc::SYS_ioctl,
        libc::SYS_getrandom,
        libc::SYS_getpid,
        libc::SYS_gettid,
        libc::SYS_getuid,
        libc::SYS_getgid,
        libc::SYS_geteuid,
        libc::SYS_getegid,
        libc::SYS_exit,
        libc::SYS_exit_group,
        libc::SYS_prctl,
        libc::SYS_rseq,
        libc::SYS_pipe2,
    ];

    for syscall in allowed_syscalls {
        rules.insert(syscall, vec![]);
    }
    rules
}
