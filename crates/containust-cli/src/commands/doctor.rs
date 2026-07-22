//! `ctst doctor` — platform and environment diagnostics.

use containust_runtime::backend::{PlatformInfo, platform_info};

/// Arguments for the `doctor` command (none today).
#[derive(clap::Args, Debug, Default)]
pub struct DoctorArgs {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Fail,
    Info,
}

/// Runs environment checks and prints a diagnostic table.
///
/// # Errors
///
/// Returns an error when one or more blocking checks fail.
pub fn execute(_args: DoctorArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    let info = platform_info();
    let mut failed = 0_u32;
    println!("Containust doctor\n");
    report_host(&info);
    failed += report_backend(&info);
    failed += report_cache(&info);
    report_offline(options.offline);
    report_linux_extras();
    let ebpf = containust_runtime::observe::ebpf_status();
    report(
        "observe.ebpf",
        Status::Info,
        ebpf,
        "Syscall/file/net probes",
    );
    println!();
    if failed == 0 {
        println!("Doctor summary: ready");
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "doctor found {failed} blocking issue(s); fix Fail rows above"
        ))
    }
}

fn report_host(info: &PlatformInfo) {
    report(
        "host.os",
        Status::Info,
        &info.os,
        "Detected operating system",
    );
    report(
        "host.arch",
        Status::Info,
        &info.arch,
        "Detected CPU architecture",
    );
}

fn report_backend(info: &PlatformInfo) -> u32 {
    if info.native_available {
        report(
            "backend.native",
            Status::Ok,
            "available",
            "Linux native namespaces/cgroups backend",
        );
    } else {
        report(
            "backend.native",
            Status::Info,
            "unavailable",
            "Use the QEMU VM backend on this platform (`ctst vm start`)",
        );
    }
    if info.qemu_available {
        report(
            "backend.qemu",
            Status::Ok,
            "available",
            "QEMU binary found on PATH",
        );
        0
    } else if info.native_available {
        report(
            "backend.qemu",
            Status::Info,
            "not required",
            "Native backend is available; QEMU optional",
        );
        0
    } else {
        report(
            "backend.qemu",
            Status::Fail,
            "missing",
            "Install QEMU (macOS: brew install qemu) for the VM backend",
        );
        1
    }
}

fn report_cache(info: &PlatformInfo) -> u32 {
    let cache = containust_common::constants::global_cache_dir();
    let mut failed = 0_u32;
    match std::fs::create_dir_all(&cache) {
        Ok(()) => report(
            "cache.dir",
            Status::Ok,
            &cache.display().to_string(),
            "Global cache is writable",
        ),
        Err(error) => {
            failed = 1;
            report(
                "cache.dir",
                Status::Fail,
                &cache.display().to_string(),
                &format!("Cannot create cache directory: {error}"),
            );
        }
    }
    let kernel = cache.join("vm").join("vmlinuz");
    if kernel.exists() {
        report(
            "cache.vm_kernel",
            Status::Ok,
            "present",
            "Pinned VM kernel is cached",
        );
    } else if info.native_available {
        report(
            "cache.vm_kernel",
            Status::Info,
            "absent",
            "Not needed for native Linux backend",
        );
    } else {
        report(
            "cache.vm_kernel",
            Status::Warn,
            "absent",
            "Run `ctst vm start` once online to populate ~/.containust/cache/vm/",
        );
    }
    failed
}

fn report_offline(offline: bool) {
    let value = if offline { "enabled" } else { "disabled" };
    report(
        "network.offline",
        Status::Info,
        value,
        "Offline mode blocks remote image/asset downloads",
    );
}

#[cfg_attr(not(target_os = "linux"), allow(clippy::missing_const_for_fn))]
fn report_linux_extras() {
    report_linux_cgroup_v2();
}

#[cfg(target_os = "linux")]
fn report_linux_cgroup_v2() {
    let cgroup = std::path::Path::new("/sys/fs/cgroup");
    if cgroup.join("cgroup.controllers").exists() {
        report(
            "linux.cgroup_v2",
            Status::Ok,
            "available",
            "cgroup v2 controllers are mounted",
        );
    } else {
        report(
            "linux.cgroup_v2",
            Status::Warn,
            "missing",
            "cgroup v2 not detected; resource limits may be unavailable",
        );
    }
}

#[cfg(not(target_os = "linux"))]
const fn report_linux_cgroup_v2() {}

fn report(check: &str, status: Status, value: &str, hint: &str) {
    let label = match status {
        Status::Ok => "OK  ",
        Status::Warn => "WARN",
        Status::Fail => "FAIL",
        Status::Info => "INFO",
    };
    println!("{label}  {check:<22} {value:<24} {hint}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_args_default() {
        let _ = DoctorArgs::default();
    }
}
