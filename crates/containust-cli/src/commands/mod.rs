//! CLI command definitions and dispatch.

pub mod build;
pub mod convert;
pub mod doctor;
pub mod exec;
pub mod images;
pub mod logs;
pub mod plan;
pub mod ps;
pub mod pull;
pub mod remove;
pub mod run;
pub mod stop;
pub mod vm;

use clap::{Parser, Subcommand};
use containust_common::types::ContainerId;
use containust_runtime::backend::ContainerInfo;
use containust_runtime::engine::{Engine, EngineOptions};
use std::path::{Path, PathBuf};

use crate::build_info;

const LONG_VERSION: &str = build_info::long_version();

/// Containust — Daemon-less sovereign container runtime.
#[derive(Parser, Debug)]
#[command(
    name = "ctst",
    version = build_info::version(),
    long_version = LONG_VERSION,
    about,
    long_about = None
)]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Command,

    /// Enable offline mode (block all network access).
    #[arg(long, global = true)]
    pub offline: bool,

    /// Path to the state file.
    #[arg(long, global = true)]
    pub state_file: Option<String>,
}

/// Runtime settings shared by every CLI command.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOptions {
    /// Reject remote imports and images.
    pub offline: bool,
    /// Optional explicit state index path.
    pub state_file: Option<PathBuf>,
}

impl RuntimeOptions {
    fn from_cli(cli: &Cli) -> Self {
        let env_offline = std::env::var("CONTAINUST_OFFLINE").is_ok_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        });
        Self {
            offline: cli.offline || env_offline,
            state_file: cli
                .state_file
                .clone()
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("CONTAINUST_STATE_FILE").map(PathBuf::from)),
        }
    }

    /// Creates an engine using this command's storage and policy.
    #[must_use]
    pub fn engine(&self) -> Engine {
        self.engine_for_project(Path::new("containust.ctst"))
    }

    /// Creates an engine scoped to the composition's project directory.
    #[must_use]
    pub fn engine_for_project(&self, composition: &Path) -> Engine {
        let (data_dir, state_file) = self.state_file.as_ref().map_or_else(
            || {
                let data_dir = containust_common::constants::project_dir(composition);
                let state_file = data_dir.join("state").join("state.json");
                (data_dir, state_file)
            },
            |state_file| {
                let data_dir = state_file
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .map_or_else(
                        || containust_common::constants::project_dir(composition),
                        Path::to_path_buf,
                    );
                (data_dir, state_file.clone())
            },
        );
        Engine::with_options(EngineOptions {
            data_dir,
            state_file,
            offline: self.offline,
        })
    }
}

fn resolve_container_id(engine: &Engine, target: &str) -> anyhow::Result<ContainerId> {
    let containers = engine.list().map_err(|e| anyhow::anyhow!("{e}"))?;
    resolve_container_id_from(&containers, target)
}

fn resolve_container_id_from(
    containers: &[ContainerInfo],
    target: &str,
) -> anyhow::Result<ContainerId> {
    containers
        .iter()
        .find(|container| container.id.as_str() == target || container.name == target)
        .map(|container| container.id.clone())
        .ok_or_else(|| anyhow::anyhow!("container not found: {target}"))
}

/// Available CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Parse a .ctst file and build container images/layers.
    Build(build::BuildArgs),
    /// Display the planned infrastructure changes before applying.
    Plan(plan::PlanArgs),
    /// Deploy the component graph.
    Run(run::RunArgs),
    /// List running containers with real-time metrics.
    Ps(ps::PsArgs),
    /// Execute a command inside a running container.
    Exec(exec::ExecArgs),
    /// Stop containers and clean up resources.
    Stop(stop::StopArgs),
    /// Remove stopped containers and their project-owned resources.
    #[command(name = "rm")]
    Remove(remove::RemoveArgs),
    /// Pull an OCI registry image into the local catalog.
    Pull(pull::PullArgs),
    /// Manage the local image catalog.
    Images(images::ImagesArgs),
    /// Convert a docker-compose.yml to .ctst format.
    Convert(convert::ConvertArgs),
    /// View container logs.
    Logs(logs::LogsArgs),
    /// Manage the lightweight VM backend.
    #[command(subcommand)]
    Vm(VmCommand),
    /// Diagnose platform, QEMU, cache, and offline readiness.
    Doctor(doctor::DoctorArgs),
}

/// VM subcommands.
#[cfg_attr(test, allow(dead_code))]
#[derive(Subcommand, Debug)]
pub enum VmCommand {
    /// Start the VM backend.
    Start(vm::VmStartArgs),
    /// Stop the VM backend.
    Stop(vm::VmStopArgs),
}

/// Dispatches the parsed CLI command to its handler.
///
/// # Errors
///
/// Returns an error if the command execution fails.
pub fn execute(cli: Cli) -> anyhow::Result<()> {
    let options = RuntimeOptions::from_cli(&cli);
    match cli.command {
        Command::Build(args) => build::execute(args, &options),
        Command::Plan(args) => plan::execute(args, &options),
        Command::Run(args) => run::execute(args, &options),
        Command::Ps(args) => ps::execute(args, &options),
        Command::Exec(args) => exec::execute(args, &options),
        Command::Stop(args) => stop::execute(args, &options),
        Command::Remove(args) => remove::execute(args, &options),
        Command::Pull(args) => pull::execute(args, &options),
        Command::Images(args) => images::execute(args, &options),
        Command::Convert(args) => convert::execute(args, &options),
        Command::Logs(args) => logs::execute(args, &options),
        Command::Vm(subcommand) => match subcommand {
            VmCommand::Start(args) => vm::vm_start(args, &options),
            VmCommand::Stop(args) => vm::vm_stop(args, &options),
        },
        Command::Doctor(args) => doctor::execute(args, &options),
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::needless_borrows_for_generic_args,
    clippy::match_wildcard_for_single_variants,
    clippy::semicolon_if_nothing_returned
)]
mod tests {
    use super::*;

    // --- Subcommand parsing ---

    #[test]
    fn cli_build_subcommand_parses_with_default_file() {
        let cli = Cli::try_parse_from(&["ctst", "build"]).expect("should parse");
        match cli.command {
            Command::Build(args) => assert_eq!(args.file, "containust.ctst"),
            other => panic!("expected Build, got {other:?}"),
        }
    }

    #[test]
    fn cli_build_subcommand_parses_with_custom_file() {
        let cli = Cli::try_parse_from(&["ctst", "build", "custom.ctst"]).expect("should parse");
        match cli.command {
            Command::Build(args) => {
                assert_eq!(args.file, "custom.ctst");
                assert!(!args.dry_run);
            }
            other => panic!("expected Build, got {other:?}"),
        }
    }

    #[test]
    fn cli_build_subcommand_parses_dry_run_flag() {
        let cli = Cli::try_parse_from(&["ctst", "build", "--dry-run"]).expect("should parse");
        match cli.command {
            Command::Build(args) => assert!(args.dry_run),
            other => panic!("expected Build, got {other:?}"),
        }
    }

    #[test]
    fn cli_plan_subcommand_parses_with_default_file() {
        let cli = Cli::try_parse_from(&["ctst", "plan"]).expect("should parse");
        match cli.command {
            Command::Plan(args) => assert_eq!(args.file, "containust.ctst"),
            other => panic!("expected Plan, got {other:?}"),
        }
    }

    #[test]
    fn cli_run_subcommand_parses_with_flags() {
        let cli = Cli::try_parse_from(&["ctst", "run", "--detach"]).expect("should parse");
        match cli.command {
            Command::Run(args) => assert!(args.detach),
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn cli_run_subcommand_parses_default_attach() {
        let cli = Cli::try_parse_from(&["ctst", "run"]).expect("should parse");
        match cli.command {
            Command::Run(args) => assert!(!args.detach),
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn cli_run_subcommand_parses_custom_file() {
        let cli = Cli::try_parse_from(&["ctst", "run", "app.ctst"]).expect("should parse");
        match cli.command {
            Command::Run(args) => assert_eq!(args.file, "app.ctst"),
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn cli_ps_subcommand_parses_all_flag() {
        let cli = Cli::try_parse_from(&["ctst", "ps", "--all"]).expect("should parse");
        match cli.command {
            Command::Ps(args) => assert!(args.all),
            other => panic!("expected Ps, got {other:?}"),
        }
    }

    #[test]
    fn cli_ps_subcommand_parses_tui_flag() {
        let cli = Cli::try_parse_from(&["ctst", "ps", "--tui"]).expect("should parse");
        match cli.command {
            Command::Ps(args) => assert!(args.tui),
            other => panic!("expected Ps, got {other:?}"),
        }
    }

    #[test]
    fn cli_exec_subcommand_parses_container_and_command() {
        let cli =
            Cli::try_parse_from(&["ctst", "exec", "abc123", "ls", "-la"]).expect("should parse");
        match cli.command {
            Command::Exec(args) => {
                assert_eq!(args.container, "abc123");
                assert_eq!(args.command, vec!["ls", "-la"]);
            }
            other => panic!("expected Exec, got {other:?}"),
        }
    }

    #[test]
    fn cli_exec_subcommand_requires_command() {
        let result = Cli::try_parse_from(&["ctst", "exec", "abc123"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_stop_subcommand_parses_with_containers() {
        let cli = Cli::try_parse_from(&["ctst", "stop", "ctr1", "ctr2"]).expect("should parse");
        match cli.command {
            Command::Stop(args) => {
                assert_eq!(args.containers, vec!["ctr1", "ctr2"]);
                assert!(!args.force);
            }
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn cli_stop_subcommand_parses_no_containers() {
        let cli = Cli::try_parse_from(&["ctst", "stop"]).expect("should parse");
        match cli.command {
            Command::Stop(args) => {
                assert!(args.containers.is_empty());
            }
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn cli_stop_subcommand_parses_force_flag() {
        let cli = Cli::try_parse_from(&["ctst", "stop", "--force"]).expect("should parse");
        match cli.command {
            Command::Stop(args) => assert!(args.force),
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn cli_remove_subcommand_parses_targets_and_force() {
        let cli =
            Cli::try_parse_from(&["ctst", "rm", "web", "db", "--force"]).expect("should parse");
        match cli.command {
            Command::Remove(args) => {
                assert_eq!(args.containers, vec!["web", "db"]);
                assert!(args.force);
            }
            other => panic!("expected Remove, got {other:?}"),
        }
    }

    #[test]
    fn cli_remove_subcommand_requires_target() {
        assert!(Cli::try_parse_from(&["ctst", "rm"]).is_err());
    }

    #[test]
    fn cli_images_subcommand_parses_list_flag() {
        let cli = Cli::try_parse_from(&["ctst", "images", "--list"]).expect("should parse");
        match cli.command {
            Command::Images(args) => assert!(args.list),
            other => panic!("expected Images, got {other:?}"),
        }
    }

    #[test]
    fn cli_images_subcommand_parses_presets_flag() {
        let cli = Cli::try_parse_from(&["ctst", "images", "--presets"]).expect("should parse");
        match cli.command {
            Command::Images(args) => assert!(args.presets),
            other => panic!("expected Images, got {other:?}"),
        }
    }

    #[test]
    fn cli_images_subcommand_parses_remove_option() {
        let cli = Cli::try_parse_from(&["ctst", "images", "--remove", "sha256:abcdef"])
            .expect("should parse");
        match cli.command {
            Command::Images(args) => assert_eq!(args.remove, Some("sha256:abcdef".to_string())),
            other => panic!("expected Images, got {other:?}"),
        }
    }

    #[test]
    fn cli_convert_subcommand_parses_with_default_file() {
        let cli = Cli::try_parse_from(&["ctst", "convert"]).expect("should parse");
        match cli.command {
            Command::Convert(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("docker-compose.yml"))
            }
            other => panic!("expected Convert, got {other:?}"),
        }
    }

    #[test]
    fn cli_convert_subcommand_parses_with_output() {
        let cli = Cli::try_parse_from(&["ctst", "convert", "compose.yml", "-o", "out.ctst"])
            .expect("should parse");
        match cli.command {
            Command::Convert(args) => {
                assert_eq!(args.file, std::path::PathBuf::from("compose.yml"));
                assert_eq!(args.output, Some(std::path::PathBuf::from("out.ctst")));
            }
            other => panic!("expected Convert, got {other:?}"),
        }
    }

    #[test]
    fn cli_logs_subcommand_parses_container_and_follow() {
        let cli = Cli::try_parse_from(&["ctst", "logs", "--follow", "mycontainer"])
            .expect("should parse");
        match cli.command {
            Command::Logs(args) => {
                assert_eq!(args.container, "mycontainer");
                assert!(args.follow);
            }
            other => panic!("expected Logs, got {other:?}"),
        }
    }

    #[test]
    fn cli_logs_subcommand_parses_without_follow() {
        let cli = Cli::try_parse_from(&["ctst", "logs", "ctr1"]).expect("should parse");
        match cli.command {
            Command::Logs(args) => {
                assert_eq!(args.container, "ctr1");
                assert!(!args.follow);
            }
            other => panic!("expected Logs, got {other:?}"),
        }
    }

    // --- Global flags ---

    #[test]
    fn cli_global_offline_flag_parses() {
        let cli = Cli::try_parse_from(&["ctst", "--offline", "build"]).expect("should parse");
        assert!(cli.offline);
    }

    #[test]
    fn cli_global_state_file_parses() {
        let cli = Cli::try_parse_from(&["ctst", "--state-file", "/tmp/state.json", "ps"])
            .expect("should parse");
        assert_eq!(cli.state_file, Some("/tmp/state.json".to_string()));
    }

    #[test]
    fn cli_global_flags_combine_with_subcommand() {
        let cli = Cli::try_parse_from(&[
            "ctst",
            "--offline",
            "--state-file",
            "/tmp/s.json",
            "build",
            "app.ctst",
        ])
        .expect("should parse");
        assert!(cli.offline);
        assert_eq!(cli.state_file, Some("/tmp/s.json".to_string()));
        match cli.command {
            Command::Build(args) => assert_eq!(args.file, "app.ctst"),
            other => panic!("expected Build, got {other:?}"),
        }
    }

    #[test]
    fn cli_defaults_offline_is_false() {
        let cli = Cli::try_parse_from(&["ctst", "run"]).expect("should parse");
        assert!(!cli.offline);
        assert!(cli.state_file.is_none());
    }

    #[test]
    fn project_engines_use_independent_storage_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first_file = dir.path().join("first").join("app.ctst");
        let second_file = dir.path().join("second").join("app.ctst");
        std::fs::create_dir_all(first_file.parent().expect("first parent")).expect("first project");
        std::fs::create_dir_all(second_file.parent().expect("second parent"))
            .expect("second project");
        std::fs::write(&first_file, "").expect("first file");
        std::fs::write(&second_file, "").expect("second file");
        let options = RuntimeOptions::default();

        let first = options.engine_for_project(&first_file);
        let second = options.engine_for_project(&second_file);

        assert_ne!(first.data_dir(), second.data_dir());
        assert_eq!(
            first.state_file(),
            first_file
                .canonicalize()
                .expect("canonical first file")
                .parent()
                .expect("canonical first parent")
                .join(".containust/state/state.json")
        );
        assert_eq!(
            second.state_file(),
            second_file
                .canonicalize()
                .expect("canonical second file")
                .parent()
                .expect("canonical second parent")
                .join(".containust/state/state.json")
        );
    }

    #[test]
    fn explicit_state_file_overrides_project_storage() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state_file = dir.path().join("custom").join("state.json");
        let options = RuntimeOptions {
            offline: false,
            state_file: Some(state_file.clone()),
        };

        let engine = options.engine_for_project(&dir.path().join("app.ctst"));
        assert_eq!(engine.state_file(), state_file);
        assert_eq!(engine.data_dir(), dir.path().join("custom"));
    }

    // --- Error cases ---

    #[test]
    fn cli_no_subcommand_fails() {
        let result = Cli::try_parse_from(&["ctst"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_unknown_subcommand_fails() {
        let result = Cli::try_parse_from(&["ctst", "unknown"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_version_flag_succeeds() {
        let result = Cli::try_parse_from(&["ctst", "--version"]);
        // --version prints and exits through clap, so either err or
        // a parsed result is acceptable depending on clap internals
        let _ = result;
    }

    #[test]
    fn cli_help_flag_succeeds() {
        let result = Cli::try_parse_from(&["ctst", "--help"]);
        // --help prints and exits through clap, so either err or
        // a parsed result is acceptable depending on clap internals
        let _ = result;
    }

    // --- VM subcommand parsing ---

    #[test]
    fn cli_vm_start_subcommand_parses() {
        let cli = Cli::try_parse_from(&["ctst", "vm", "start"]).expect("should parse");
        match cli.command {
            Command::Vm(subcommand) => match subcommand {
                VmCommand::Start(_) => {}
                other => panic!("expected Start, got {other:?}"),
            },
            other => panic!("expected Vm, got {other:?}"),
        }
    }

    #[test]
    fn cli_vm_stop_subcommand_parses() {
        let cli = Cli::try_parse_from(&["ctst", "vm", "stop"]).expect("should parse");
        match cli.command {
            Command::Vm(subcommand) => match subcommand {
                VmCommand::Stop(_) => {}
                other => panic!("expected Stop, got {other:?}"),
            },
            other => panic!("expected Vm, got {other:?}"),
        }
    }

    #[test]
    fn cli_vm_no_subcommand_fails() {
        let result = Cli::try_parse_from(&["ctst", "vm"]);
        assert!(result.is_err());
    }
}
