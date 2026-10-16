//! Declarative plan/apply diffing.
//!
//! A deploy plan compares the desired composition against live backend
//! state and a per-project ledger of applied spec fingerprints
//! (`applied.json`, next to the state file). Each component gets exactly one
//! action; `ctst plan` prints the plan and `ctst run` executes it in
//! dependency order, replacing changed components one at a time.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};
use containust_common::types::{ContainerId, UpdateStrategy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::backend::{ContainerConfig, ContainerInfo};

/// File name of the applied-spec ledger, stored next to `state.json`.
pub const LEDGER_FILE_NAME: &str = "applied.json";

/// Suffix of the staged container a start-first update creates next to
/// the one it replaces (`api` → `api~next`).
pub const STAGED_SUFFIX: &str = "~next";

/// Name of the staged replacement for `name` during a start-first update.
#[must_use]
pub fn staged_name(name: &str) -> String {
    format!("{name}{STAGED_SUFFIX}")
}

/// The declared component a leftover staged container belongs to.
#[must_use]
pub fn staged_owner(name: &str) -> Option<&str> {
    name.strip_suffix(STAGED_SUFFIX)
}

/// What `ctst run` will do to one component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanAction {
    /// No container with this name exists yet.
    Create,
    /// The container exists but its spec (or an upstream) changed.
    Replace,
    /// The container exists with the current spec but is not running.
    Start,
    /// The container is running with the current spec.
    Keep,
    /// The container is no longer declared (removed only with `--prune`).
    Remove,
}

impl PlanAction {
    /// Single-character diff marker used by `ctst plan`.
    #[must_use]
    pub const fn symbol(self) -> char {
        match self {
            Self::Create => '+',
            Self::Replace => '~',
            Self::Start => '>',
            Self::Keep => '=',
            Self::Remove => '-',
        }
    }

    /// Whether this action changes anything on the host.
    #[must_use]
    pub const fn is_change(self) -> bool {
        !matches!(self, Self::Keep)
    }
}

impl fmt::Display for PlanAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Create => "create",
            Self::Replace => "replace",
            Self::Start => "start",
            Self::Keep => "keep",
            Self::Remove => "remove",
        };
        f.write_str(label)
    }
}

/// One planned change for a named component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedChange {
    /// Component (container) name.
    pub name: String,
    /// Action that apply will take.
    pub action: PlanAction,
    /// Human-readable reason for non-trivial actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Existing container ID, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_id: Option<ContainerId>,
    /// Desired spec fingerprint (`None` for removals).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// How a `replace` is carried out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<UpdateStrategy>,
    /// Why the declared strategy is not used (e.g. start-first fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Ordered plan: declared components in dependency order, then removals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployPlan {
    /// Planned changes.
    pub changes: Vec<PlannedChange>,
    /// Problems that will not stop the apply but will likely break the
    /// stack (e.g. a `CONNECT` across networks).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl DeployPlan {
    /// Number of changes with the given action.
    #[must_use]
    pub fn count(&self, action: PlanAction) -> usize {
        self.changes.iter().filter(|c| c.action == action).count()
    }

    /// Whether applying the plan would leave the host untouched.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        !self.changes.iter().any(|c| c.action.is_change())
    }

    /// One-line summary such as `1 to create, 0 to replace, ...`.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} to create, {} to replace, {} to start, {} unchanged, {} to remove",
            self.count(PlanAction::Create),
            self.count(PlanAction::Replace),
            self.count(PlanAction::Start),
            self.count(PlanAction::Keep),
            self.count(PlanAction::Remove),
        )
    }
}

/// Desired component spec handed to the planner.
#[derive(Debug, Clone)]
pub struct DesiredComponent<'a> {
    /// Component name.
    pub name: &'a str,
    /// Spec fingerprint (see [`fingerprint`]).
    pub fingerprint: String,
    /// Components this one `CONNECT`s to (upstreams).
    pub upstreams: Vec<&'a str>,
    /// Effective update strategy for a replace.
    pub strategy: UpdateStrategy,
    /// Why the declared strategy fell back, if it did.
    pub note: Option<String>,
}

/// Computes a stable fingerprint of everything that shapes a container.
///
/// Uses the declared image string (not a resolved preset digest) so planning
/// stays read-only and offline-safe.
#[must_use]
pub fn fingerprint(config: &ContainerConfig) -> String {
    let ns = &config.namespaces;
    let spec = serde_json::json!({
        "v": 1,
        "image": config.image,
        "command": config.command,
        "env": config.env,
        "memory_bytes": config.memory_bytes,
        "cpu_shares": config.cpu_shares,
        "readonly_rootfs": config.readonly_rootfs,
        "volumes": config.volumes,
        "port": config.port,
        "port_mappings": config.port_mappings,
        "network": config.network,
        "restart": config.restart,
        "healthcheck": config.healthcheck,
        "ns": [ns.pid, ns.mount, ns.network, ns.uts, ns.ipc, ns.user],
    });
    let digest = Sha256::digest(spec.to_string().as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Builds the plan from desired components (in dependency order), live
/// containers, and the applied ledger.
#[must_use]
pub fn compute(
    desired: &[DesiredComponent<'_>],
    live: &[ContainerInfo],
    ledger: &Ledger,
) -> DeployPlan {
    let live_by_name: HashMap<&str, &ContainerInfo> =
        live.iter().map(|info| (info.name.as_str(), info)).collect();
    let mut replaced: HashSet<&str> = HashSet::new();
    let mut changes = Vec::with_capacity(desired.len());

    for component in desired {
        let existing = live_by_name.get(component.name).copied();
        let (action, reason) = existing.map_or((PlanAction::Create, None), |info| {
            action_for_existing(component, info, ledger, &replaced)
        });
        let (strategy, note) = match action {
            PlanAction::Replace => {
                let _ = replaced.insert(component.name);
                replace_strategy(component, existing)
            }
            // Warn up front that the declared strategy will not apply.
            PlanAction::Create => (None, component.note.clone()),
            _ => (None, None),
        };
        changes.push(PlannedChange {
            name: component.name.to_string(),
            action,
            reason,
            container_id: existing.map(|info| info.id.clone()),
            fingerprint: Some(component.fingerprint.clone()),
            strategy,
            note,
        });
    }

    let declared: HashSet<&str> = desired.iter().map(|c| c.name).collect();
    changes.extend(removals(&declared, live));
    DeployPlan {
        changes,
        ..DeployPlan::default()
    }
}

/// `remove` changes for live containers that are no longer declared, in
/// name order.
fn removals(declared: &HashSet<&str>, live: &[ContainerInfo]) -> Vec<PlannedChange> {
    let mut orphans: Vec<&ContainerInfo> = live
        .iter()
        .filter(|info| !declared.contains(info.name.as_str()))
        .collect();
    orphans.sort_by(|a, b| a.name.cmp(&b.name));
    orphans
        .into_iter()
        .map(|info| {
            let leftover = staged_owner(&info.name).is_some_and(|owner| declared.contains(owner));
            PlannedChange {
                name: info.name.clone(),
                action: PlanAction::Remove,
                reason: Some(if leftover {
                    "left over from an interrupted start-first update".into()
                } else {
                    "no longer declared".into()
                }),
                container_id: Some(info.id.clone()),
                fingerprint: None,
                strategy: None,
                note: None,
            }
        })
        .collect()
}

/// Strategy for replacing `component`; a stopped container serves no
/// traffic, so it is simply recreated.
fn replace_strategy(
    component: &DesiredComponent<'_>,
    existing: Option<&ContainerInfo>,
) -> (Option<UpdateStrategy>, Option<String>) {
    let running = existing.is_some_and(|info| info.state == "running");
    if component.strategy == UpdateStrategy::StartFirst && !running {
        return (Some(UpdateStrategy::Recreate), None);
    }
    (Some(component.strategy), component.note.clone())
}

/// Chooses the action for a component that already has a container.
fn action_for_existing(
    component: &DesiredComponent<'_>,
    info: &ContainerInfo,
    ledger: &Ledger,
    replaced: &HashSet<&str>,
) -> (PlanAction, Option<String>) {
    let Some(applied) = ledger.components.get(component.name) else {
        return (
            PlanAction::Replace,
            Some("no recorded spec (deployed by an older ctst)".into()),
        );
    };
    if applied.fingerprint != component.fingerprint {
        return (PlanAction::Replace, Some("spec changed".into()));
    }
    if let Some(up) = component.upstreams.iter().find(|up| replaced.contains(*up)) {
        return (
            PlanAction::Replace,
            Some(format!("upstream '{up}' replaced")),
        );
    }
    if info.state == "running" {
        (PlanAction::Keep, None)
    } else {
        (
            PlanAction::Start,
            Some(format!("container is {}", info.state)),
        )
    }
}

/// Spec recorded for one applied component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedComponent {
    /// Fingerprint of the spec that was deployed.
    pub fingerprint: String,
    /// Container that was created for it.
    pub container_id: ContainerId,
}

/// Per-project record of the specs most recently applied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// Applied specs keyed by component name.
    #[serde(default)]
    pub components: BTreeMap<String, AppliedComponent>,
}

impl Ledger {
    /// Ledger path for a state file (sibling `applied.json`).
    #[must_use]
    pub fn path_for(state_file: &Path) -> PathBuf {
        state_file.with_file_name(LEDGER_FILE_NAME)
    }

    /// Loads the ledger; a missing file is an empty ledger.
    ///
    /// # Errors
    ///
    /// Returns an error when the file exists but cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| ContainustError::Config {
                message: format!("invalid plan ledger {}: {e}", path.display()),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ContainustError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// Atomically writes the ledger (temp file + rename).
    ///
    /// # Errors
    ///
    /// Returns an error when the ledger cannot be written.
    pub fn save(&self, path: &Path) -> Result<()> {
        let io_err = |source| ContainustError::Io {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| ContainustError::Config {
            message: format!("cannot serialize plan ledger: {e}"),
        })?;
        let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
        let mut file = std::fs::File::create(&tmp).map_err(io_err)?;
        file.write_all(&bytes).map_err(io_err)?;
        file.sync_all().map_err(io_err)?;
        std::fs::rename(&tmp, path).map_err(io_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, state: &str) -> ContainerInfo {
        ContainerInfo {
            id: ContainerId::new(format!("id-{name}")),
            name: name.into(),
            state: state.into(),
            pid: None,
            image: String::new(),
            created_at: String::new(),
        }
    }

    fn desired<'a>(name: &'a str, fp: &str, upstreams: Vec<&'a str>) -> DesiredComponent<'a> {
        DesiredComponent {
            name,
            fingerprint: fp.into(),
            upstreams,
            strategy: UpdateStrategy::Recreate,
            note: None,
        }
    }

    fn ledger(entries: &[(&str, &str)]) -> Ledger {
        Ledger {
            components: entries
                .iter()
                .map(|(name, fp)| {
                    (
                        (*name).to_string(),
                        AppliedComponent {
                            fingerprint: (*fp).into(),
                            container_id: ContainerId::new(format!("id-{name}")),
                        },
                    )
                })
                .collect(),
        }
    }

    fn actions(plan: &DeployPlan) -> Vec<(&str, PlanAction)> {
        plan.changes
            .iter()
            .map(|c| (c.name.as_str(), c.action))
            .collect()
    }

    #[test]
    fn fresh_project_creates_everything() {
        let plan = compute(
            &[desired("db", "a", vec![]), desired("api", "b", vec!["db"])],
            &[],
            &Ledger::default(),
        );
        assert_eq!(
            actions(&plan),
            vec![("db", PlanAction::Create), ("api", PlanAction::Create)]
        );
        assert!(!plan.is_noop());
    }

    #[test]
    fn unchanged_running_stack_is_noop() {
        let plan = compute(
            &[desired("db", "a", vec![])],
            &[info("db", "running")],
            &ledger(&[("db", "a")]),
        );
        assert_eq!(actions(&plan), vec![("db", PlanAction::Keep)]);
        assert!(plan.is_noop());
        assert!(plan.summary().contains("1 unchanged"));
    }

    #[test]
    fn stopped_container_with_same_spec_is_started() {
        let plan = compute(
            &[desired("db", "a", vec![])],
            &[info("db", "stopped")],
            &ledger(&[("db", "a")]),
        );
        assert_eq!(actions(&plan), vec![("db", PlanAction::Start)]);
    }

    #[test]
    fn changed_spec_replaces_and_cascades_to_dependents() {
        let plan = compute(
            &[
                desired("db", "new", vec![]),
                desired("api", "b", vec!["db"]),
                desired("cache", "c", vec![]),
            ],
            &[
                info("db", "running"),
                info("api", "running"),
                info("cache", "running"),
            ],
            &ledger(&[("db", "old"), ("api", "b"), ("cache", "c")]),
        );
        assert_eq!(
            actions(&plan),
            vec![
                ("db", PlanAction::Replace),
                ("api", PlanAction::Replace),
                ("cache", PlanAction::Keep),
            ]
        );
        assert_eq!(
            plan.changes[1].reason.as_deref(),
            Some("upstream 'db' replaced")
        );
    }

    #[test]
    fn container_without_ledger_entry_is_replaced() {
        let plan = compute(
            &[desired("db", "a", vec![])],
            &[info("db", "running")],
            &Ledger::default(),
        );
        assert_eq!(actions(&plan), vec![("db", PlanAction::Replace)]);
    }

    #[test]
    fn undeclared_containers_are_removed_in_name_order() {
        let plan = compute(
            &[],
            &[info("zeta", "running"), info("alpha", "stopped")],
            &Ledger::default(),
        );
        assert_eq!(
            actions(&plan),
            vec![("alpha", PlanAction::Remove), ("zeta", PlanAction::Remove)]
        );
    }

    #[test]
    fn start_first_applies_only_to_running_replacements() {
        let mut api = desired("api", "new", vec![]);
        api.strategy = UpdateStrategy::StartFirst;
        let mut worker = desired("worker", "new", vec![]);
        worker.strategy = UpdateStrategy::StartFirst;
        let plan = compute(
            &[api, worker],
            &[info("api", "running"), info("worker", "stopped")],
            &ledger(&[("api", "old"), ("worker", "old")]),
        );
        assert_eq!(plan.changes[0].strategy, Some(UpdateStrategy::StartFirst));
        assert_eq!(plan.changes[1].strategy, Some(UpdateStrategy::Recreate));
        let json = serde_json::to_value(&plan).expect("json");
        assert_eq!(json["changes"][0]["strategy"], "start-first");
    }

    #[test]
    fn fallback_note_is_carried_into_the_plan() {
        let mut api = desired("api", "new", vec![]);
        api.note = Some("host network".into());
        let plan = compute(
            &[api],
            &[info("api", "running")],
            &ledger(&[("api", "old")]),
        );
        assert_eq!(plan.changes[0].note.as_deref(), Some("host network"));
    }

    #[test]
    fn leftover_staged_container_is_labelled() {
        let plan = compute(
            &[desired("api", "a", vec![])],
            &[info("api", "running"), info("api~next", "running")],
            &ledger(&[("api", "a")]),
        );
        assert_eq!(plan.changes[1].name, "api~next");
        assert_eq!(plan.changes[1].action, PlanAction::Remove);
        assert!(
            plan.changes[1]
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("interrupted start-first"))
        );
        assert_eq!(staged_owner(&staged_name("api")), Some("api"));
    }

    #[test]
    fn ledger_roundtrips_and_missing_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = Ledger::path_for(&dir.path().join("state").join("state.json"));
        assert_eq!(Ledger::load(&path).expect("missing"), Ledger::default());
        let ledger = ledger(&[("db", "a")]);
        ledger.save(&path).expect("save");
        assert_eq!(Ledger::load(&path).expect("load"), ledger);
    }

    #[test]
    fn fingerprint_tracks_spec_changes() {
        let base = ContainerConfig {
            name: "app".into(),
            image: "file:///x".into(),
            command: vec![],
            env: vec![],
            memory_bytes: None,
            cpu_shares: None,
            readonly_rootfs: true,
            volumes: vec![],
            port: None,
            ports: vec![],
            port_mappings: vec![],
            network: "none".into(),
            restart: containust_common::types::RestartPolicy::Never,
            healthcheck: None,
            namespaces: containust_core::namespace::NamespaceConfig::default(),
            defer_publish: false,
        };
        let mut changed = base.clone();
        changed.memory_bytes = Some(1);
        assert_eq!(fingerprint(&base), fingerprint(&base.clone()));
        assert_ne!(fingerprint(&base), fingerprint(&changed));
        assert_eq!(fingerprint(&base).len(), 64);
    }
}
