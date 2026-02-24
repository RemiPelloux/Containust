# Containust Specification

**Version**: 1.0 (Definitive)
**Language**: Rust (Systems)
**Target**: Sovereign, secure, and modular infrastructure.

## 1. Vision and Objectives

Containust is not a Docker clone but an evolution. It combines the power of Linux isolation with the flexibility of Infrastructure-as-Code (Terraform-style) in a single, secure, daemon-less binary.

- **Zero Daemon**: No permanent root process.
- **Native Composition**: Dependency graph management between containers.
- **Sovereignty**: Priority to local sources and disconnected (air-gapped) environments.

## 2. Core Engine Technical Specifications

### 2.1 Isolation (Namespaces & Cgroups)

- **Namespaces**: Complete isolation (PID, Mount, Network, User, IPC, UTS).
- **Cgroups v2**: Granular resource limiting (CPU, RAM, I/O) via `/sys/fs/cgroup`.
- **RootFS**: Root change via `pivot_root` (more secure than `chroot`).

### 2.2 Filesystem & Storage

- **OverlayFS**: Layer management for cache reuse.
- **Lazy-Loading (FUSE)**: Ability to start a container before its image is fully extracted.
- **Local-First**: Native support for `file://` (directories) and `tar://` (archives) with SHA-256 validation.

## 3. Composition Language: .ctst

The format is designed to be declarative and LLM-friendly.

### Modularity

- **IMPORT**: Import components from other files or network.
- **COMPONENT**: Define reusable bricks with parameters.

### Auto-Wiring

- **CONNECT**: Establish logical links between components (e.g., App -> DB).
- Automatic injection of connection environment variables.

### Static Analysis

Automatic "distroless" builds by analyzing binary dependencies (internal ldd) to keep only what is necessary.

## 4. State Management & CLI (The Toolbox)

Containust maintains a local state index (e.g., `/var/lib/containust/state.json`) to manage lifecycles without a daemon.

### 4.1 CLI Commands

| Command | Description |
|---|---|
| `ctst build` | Parse the .ctst and generate images/layers |
| `ctst plan` | Display infrastructure changes before applying |
| `ctst run` | Deploy the component graph |
| `ctst ps` | List containers with real-time metrics |
| `ctst exec` | Enter a running container (namespace joining) |
| `ctst stop` | Graceful stop and resource cleanup |
| `ctst images` | Manage the local RootFS catalog |
| `ctst convert` | Convert docker-compose.yml to .ctst format |

### 4.2 Observability

- **TUI Dashboard**: Interactive terminal interface (ratatui).
- **eBPF Tracer**: Real-time monitoring of syscalls, file opens, and network sockets inside containers.

## 5. Containust SDK

The SDK enables using Containust as a Rust library rather than a CLI tool.

- **Crate `containust-core`**: Low-level functions for namespace manipulation.
- **Crate `containust-sdk`**:
  - `ContainerBuilder`: Fluent API for configuring and launching processes.
  - `GraphResolver`: Tool for manipulating and validating component dependencies.
  - `EventListener`: Event stream for programmatic monitoring.

## 6. Security & Critical Environments

- **Offline Mode**: Strict blocking of all outbound network access during build and run via `--offline`.
- **Read-Only Rootfs**: Immutable filesystem by default. Only declared volumes are writable.
- **Capability Dropping**: Systematic removal of unnecessary Linux privileges.

## 7. Technical Stack Summary

| Component | Technology |
|---|---|
| Language | Rust (Edition 2024) |
| Parsing | nom |
| System | nix, libc |
| UI | ratatui, clap |
| Graph | petgraph |
| Observability | aya (eBPF) |
