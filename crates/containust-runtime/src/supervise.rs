//! Restart-policy and healthcheck enforcement.
//!
//! Containust is daemonless, so policies are applied during
//! reconciliation (every `ctst ps` / `ctst run` invocation): containers
//! whose process died are restarted according to their policy, and due
//! health probes execute with unhealthy containers restarted.

use containust_common::error::Result;
use containust_common::types::{
    ContainerId, ContainerState, HealthRecord, HealthState, HealthcheckSpec, RestartPolicy,
};

use crate::backend::ContainerBackend;
use crate::state::{StateEntry, StateStore};

/// Work performed by one policy-enforcement pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PolicyOutcome {
    /// Containers restarted (crash or unhealthy).
    pub restarted: usize,
    /// Containers newly marked unhealthy.
    pub unhealthy: usize,
}

/// Applies restart policies and due health probes for one project.
///
/// Failures to restart an individual container are logged and do not
/// abort the pass; state access errors propagate.
///
/// # Errors
///
/// Returns an error when the state index cannot be read or updated.
pub fn enforce_policies(
    store: &StateStore,
    backend: &dyn ContainerBackend,
) -> Result<PolicyOutcome> {
    let mut outcome = PolicyOutcome::default();
    outcome.restarted += restart_failed_containers(store, backend)?;
    let (probed_unhealthy, probe_restarts) = probe_running_containers(store, backend)?;
    outcome.unhealthy += probed_unhealthy;
    outcome.restarted += probe_restarts;
    Ok(outcome)
}

/// Restarts `Failed` containers whose policy demands it.
///
/// A `Failed` entry means reconciliation observed the process dead
/// while the container was expected to run; both `always` and
/// `on-failure` treat that as a restartable failure.
fn restart_failed_containers(store: &StateStore, backend: &dyn ContainerBackend) -> Result<usize> {
    let snapshot = store.read()?;
    let mut restarted = 0;
    for entry in &snapshot.containers {
        if entry.state != ContainerState::Failed || entry.restart == RestartPolicy::Never {
            continue;
        }
        if try_restart(store, backend, &entry.id)? {
            restarted += 1;
        }
    }
    Ok(restarted)
}

/// Runs due health probes on running containers with a healthcheck.
fn probe_running_containers(
    store: &StateStore,
    backend: &dyn ContainerBackend,
) -> Result<(usize, usize)> {
    let snapshot = store.read()?;
    let now = chrono::Utc::now();
    let mut unhealthy = 0;
    let mut restarted = 0;
    for entry in &snapshot.containers {
        let Some(spec) = &entry.healthcheck else {
            continue;
        };
        if entry.state != ContainerState::Running || !probe_is_due(entry, spec, now) {
            continue;
        }
        let healthy = run_probe(backend, &entry.id, spec);
        let became_unhealthy = record_probe_result(store, &entry.id, spec, healthy)?;
        if became_unhealthy {
            unhealthy += 1;
            tracing::warn!(id = %entry.id, name = %entry.name, "container is unhealthy");
            restarted += usize::from(restart_unhealthy(store, backend, entry)?);
        }
    }
    Ok((unhealthy, restarted))
}

/// Stops and restarts an unhealthy container when its policy allows it.
fn restart_unhealthy(
    store: &StateStore,
    backend: &dyn ContainerBackend,
    entry: &StateEntry,
) -> Result<bool> {
    if entry.restart == RestartPolicy::Never {
        return Ok(false);
    }
    backend.stop(&entry.id)?;
    try_restart(store, backend, &entry.id)
}

/// Returns whether the probe interval (and start period) has elapsed.
fn probe_is_due(
    entry: &StateEntry,
    spec: &HealthcheckSpec,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let started = parse_rfc3339(&entry.created_at);
    if let Some(started) = started {
        let grace = chrono::Duration::seconds(i64::try_from(spec.start_period_secs).unwrap_or(0));
        if now < started + grace {
            return false;
        }
    }
    let last = entry
        .health
        .as_ref()
        .and_then(|health| health.last_probe_at.as_deref())
        .and_then(parse_rfc3339);
    last.is_none_or(|last| {
        let interval = chrono::Duration::seconds(i64::try_from(spec.interval_secs).unwrap_or(0));
        now >= last + interval
    })
}

fn parse_rfc3339(text: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|parsed| parsed.with_timezone(&chrono::Utc))
}

/// Executes one probe; any exec error counts as a failed probe.
fn run_probe(backend: &dyn ContainerBackend, id: &ContainerId, spec: &HealthcheckSpec) -> bool {
    match backend.exec(id, &spec.command) {
        Ok(output) => output.exit_code == 0,
        Err(error) => {
            tracing::warn!(id = %id, %error, "health probe execution failed");
            false
        }
    }
}

/// Persists a probe result; returns whether the container crossed the
/// failure threshold on this probe.
fn record_probe_result(
    store: &StateStore,
    id: &ContainerId,
    spec: &HealthcheckSpec,
    healthy: bool,
) -> Result<bool> {
    let retries = spec.retries.max(1);
    store.update(|state| {
        let Some(entry) = state.containers.iter_mut().find(|entry| entry.id == *id) else {
            return Ok(false);
        };
        let mut record = entry.health.clone().unwrap_or_default();
        record.last_probe_at = Some(chrono::Utc::now().to_rfc3339());
        if healthy {
            record.consecutive_failures = 0;
            record.state = HealthState::Healthy;
        } else {
            record.consecutive_failures += 1;
            if record.consecutive_failures >= retries {
                record.state = HealthState::Unhealthy;
            }
        }
        let crossed = record.state == HealthState::Unhealthy
            && entry.health.as_ref().map(|previous| previous.state) != Some(HealthState::Unhealthy);
        entry.health = Some(record);
        Ok(crossed)
    })
}

/// Attempts a restart; failures are logged, never fatal for the pass.
fn try_restart(
    store: &StateStore,
    backend: &dyn ContainerBackend,
    id: &ContainerId,
) -> Result<bool> {
    match backend.start(id) {
        Ok(pid) => {
            tracing::info!(id = %id, pid, "container restarted by policy");
            store.update(|state| {
                let Some(entry) = state.containers.iter_mut().find(|entry| entry.id == *id) else {
                    return Ok(());
                };
                entry.restart_count += 1;
                entry.health = entry.healthcheck.is_some().then(HealthRecord::default);
                Ok(())
            })?;
            Ok(true)
        }
        Err(error) => {
            tracing::warn!(id = %id, %error, "policy restart failed");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use containust_common::error::ContainustError;

    use super::*;
    use crate::backend::{ContainerConfig, ContainerInfo};
    use crate::exec::ExecOutput;
    use crate::state::StateFile;

    #[derive(Default)]
    struct ProbeBackend {
        starts: AtomicUsize,
        stops: AtomicUsize,
        execs: AtomicUsize,
        probe_fails: AtomicBool,
    }

    impl ContainerBackend for ProbeBackend {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn create(&self, _config: &ContainerConfig) -> Result<ContainerId> {
            Err(ContainustError::Config {
                message: "unused".into(),
            })
        }
        fn start(&self, _id: &ContainerId) -> Result<u32> {
            let _ = self.starts.fetch_add(1, Ordering::SeqCst);
            Ok(7)
        }
        fn stop(&self, _id: &ContainerId) -> Result<()> {
            let _ = self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn exec(&self, _id: &ContainerId, _cmd: &[String]) -> Result<ExecOutput> {
            let _ = self.execs.fetch_add(1, Ordering::SeqCst);
            Ok(ExecOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: i32::from(self.probe_fails.load(Ordering::SeqCst)),
            })
        }
        fn remove(&self, _id: &ContainerId) -> Result<()> {
            Ok(())
        }
        fn logs(&self, _id: &ContainerId) -> Result<String> {
            Ok(String::new())
        }
        fn list(&self) -> Result<Vec<ContainerInfo>> {
            Ok(Vec::new())
        }
        fn is_available(&self) -> bool {
            true
        }
    }

    fn entry(
        id: &str,
        state: ContainerState,
        restart: RestartPolicy,
        healthcheck: Option<HealthcheckSpec>,
    ) -> StateEntry {
        StateEntry {
            id: ContainerId::new(id),
            name: id.into(),
            state,
            pid: state.eq(&ContainerState::Running).then_some(1),
            image: "file:///image".into(),
            command: Vec::new(),
            env: Vec::new(),
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: Vec::new(),
            ports: Vec::new(),
            restart,
            healthcheck,
            health: None,
            restart_count: 0,
            rootfs_path: None,
            log_path: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn store_with(entries: Vec<StateEntry>) -> (tempfile::TempDir, StateStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = StateStore::new(dir.path().join("state.json"));
        store
            .write(&StateFile {
                containers: entries,
                ..StateFile::default()
            })
            .expect("seed state");
        (dir, store)
    }

    fn quick_probe() -> HealthcheckSpec {
        HealthcheckSpec {
            command: vec!["true".into()],
            interval_secs: 0,
            timeout_secs: 1,
            retries: 1,
            start_period_secs: 0,
        }
    }

    #[test]
    fn failed_container_with_always_policy_is_restarted() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Failed,
            RestartPolicy::Always,
            None,
        )]);
        let backend = ProbeBackend::default();

        let outcome = enforce_policies(&store, &backend).expect("enforce");

        assert_eq!(outcome.restarted, 1);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        let state = store.read().expect("read");
        assert_eq!(state.containers[0].restart_count, 1);
    }

    #[test]
    fn failed_container_with_never_policy_is_not_restarted() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Failed,
            RestartPolicy::Never,
            None,
        )]);
        let backend = ProbeBackend::default();

        let outcome = enforce_policies(&store, &backend).expect("enforce");

        assert_eq!(outcome, PolicyOutcome::default());
        assert_eq!(backend.starts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn stopped_container_is_never_auto_restarted() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Stopped,
            RestartPolicy::Always,
            None,
        )]);
        let backend = ProbeBackend::default();

        let outcome = enforce_policies(&store, &backend).expect("enforce");
        assert_eq!(outcome.restarted, 0);
    }

    #[test]
    fn healthy_probe_records_healthy_state() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Running,
            RestartPolicy::Never,
            Some(quick_probe()),
        )]);
        let backend = ProbeBackend::default();

        let outcome = enforce_policies(&store, &backend).expect("enforce");

        assert_eq!(outcome.unhealthy, 0);
        let state = store.read().expect("read");
        let health = state.containers[0].health.clone().expect("health record");
        assert_eq!(health.state, HealthState::Healthy);
        assert!(health.last_probe_at.is_some());
    }

    #[test]
    fn failing_probe_marks_unhealthy_and_restarts() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Running,
            RestartPolicy::OnFailure,
            Some(quick_probe()),
        )]);
        let backend = ProbeBackend::default();
        backend.probe_fails.store(true, Ordering::SeqCst);

        let outcome = enforce_policies(&store, &backend).expect("enforce");

        assert_eq!(outcome.unhealthy, 1);
        assert_eq!(outcome.restarted, 1);
        assert_eq!(backend.stops.load(Ordering::SeqCst), 1);
        assert_eq!(backend.starts.load(Ordering::SeqCst), 1);
        let state = store.read().expect("read");
        // The restart reset probe bookkeeping for the new process.
        let health = state.containers[0].health.clone().expect("health record");
        assert_eq!(health.state, HealthState::Starting);
        assert_eq!(state.containers[0].restart_count, 1);
    }

    #[test]
    fn failing_probe_with_never_policy_only_marks_unhealthy() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Running,
            RestartPolicy::Never,
            Some(quick_probe()),
        )]);
        let backend = ProbeBackend::default();
        backend.probe_fails.store(true, Ordering::SeqCst);

        let outcome = enforce_policies(&store, &backend).expect("enforce");

        assert_eq!(outcome.unhealthy, 1);
        assert_eq!(outcome.restarted, 0);
        let state = store.read().expect("read");
        let health = state.containers[0].health.clone().expect("health record");
        assert_eq!(health.state, HealthState::Unhealthy);
    }

    #[test]
    fn probe_within_interval_is_skipped() {
        let mut seeded = entry(
            "a",
            ContainerState::Running,
            RestartPolicy::Never,
            Some(HealthcheckSpec {
                interval_secs: 3600,
                ..quick_probe()
            }),
        );
        seeded.health = Some(HealthRecord {
            state: HealthState::Healthy,
            consecutive_failures: 0,
            last_probe_at: Some(chrono::Utc::now().to_rfc3339()),
        });
        let (_dir, store) = store_with(vec![seeded]);
        let backend = ProbeBackend::default();

        let _ = enforce_policies(&store, &backend).expect("enforce");
        assert_eq!(backend.execs.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn probe_within_start_period_is_skipped() {
        let (_dir, store) = store_with(vec![entry(
            "a",
            ContainerState::Running,
            RestartPolicy::Never,
            Some(HealthcheckSpec {
                start_period_secs: 3600,
                ..quick_probe()
            }),
        )]);
        let backend = ProbeBackend::default();

        let _ = enforce_policies(&store, &backend).expect("enforce");
        assert_eq!(backend.execs.load(Ordering::SeqCst), 0);
    }
}
