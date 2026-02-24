# Containust Specification

**Version**: 1.0 (Definitive)
**Language**: Rust (Systems)
**Target**: Sovereign, secure, and modular infrastructure.
**Platforms**: Linux (native), macOS (VM backend), Windows (VM backend)

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

Containust maintains a project-local state directory (`.containust/` next to the `.ctst` file) to manage lifecycles without a daemon. Global immutable assets (VM kernel, initramfs) are cached in `~/.containust/cache/`.

### 4.0 Storage Model

| Tier | Location | Contents |
|------|----------|----------|
| **Global cache** | `~/.containust/cache/` | Immutable VM assets (kernel, initramfs) |
| **Project state** | `.containust/` (sibling of `.ctst` file) | Container state, logs, images |

### 4.1 CLI Commands

| Command | Description |
|---|---|
| `ctst build` | Parse the .ctst and generate images/layers |
| `ctst plan` | Display infrastructure changes before applying |
| `ctst run` | Deploy the component graph |
| `ctst ps` | List containers with real-time metrics |
| `ctst exec` | Enter a running container (namespace joining) |
| `ctst stop` | Graceful stop and resource cleanup |
| `ctst logs` | View container stdout/stderr logs |
| `ctst images` | Manage the local RootFS catalog |
| `ctst convert` | Convert docker-compose.yml to .ctst format |
| `ctst vm start/stop` | Manage the platform VM (macOS/Windows) |

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

## 7. Cross-Platform Support

### 7.1 Native-First, VM-Fallback Architecture

Containust uses a `ContainerBackend` trait to abstract platform-specific container operations. On Linux, the `LinuxNativeBackend` invokes kernel syscalls directly with zero overhead. On macOS and Windows, the `VMBackend` boots a lightweight Alpine Linux VM (~50MB) via QEMU and forwards container operations via JSON-RPC over TCP.

| Platform | Backend | Acceleration | Boot Time |
|---|---|---|---|
| Linux | `LinuxNativeBackend` | N/A (native) | Instant |
| macOS | `VMBackend` (QEMU) | HVF (Apple Hypervisor) | < 2s |
| Windows | `VMBackend` (QEMU) | Hyper-V / WHPX | < 2s |

### 7.2 VM Backend Specification

- **Guest OS**: Alpine Linux (minimal, ~15MB kernel+initramfs)
- **VM Agent**: A BusyBox shell-based JSON-RPC server running inside the VM using `nc` and `chroot`
- **Communication**: JSON-RPC 2.0 over TCP socket
- **Lifecycle**: Auto-start on first container operation, explicit `ctst vm start/stop` for manual control
- **Resource Isolation**: VM runs with constrained resources; individual containers inside the VM have their own cgroup limits
- **State Persistence**: Container state is ephemeral within the VM; project-level state is stored in `.containust/` next to the `.ctst` file on the host
- **Asset Caching**: VM kernel and initramfs cached in `~/.containust/cache/vm/`, downloaded once and reused across projects

### 7.3 `ContainerBackend` Trait

```rust
pub trait ContainerBackend: Send + Sync {
    fn create(&self, config: &ContainerConfig) -> Result<ContainerId>;
    fn start(&self, id: &ContainerId) -> Result<u32>;
    fn stop(&self, id: &ContainerId) -> Result<()>;
    fn exec(&self, id: &ContainerId, cmd: &[String]) -> Result<ExecOutput>;
    fn remove(&self, id: &ContainerId) -> Result<()>;
    fn logs(&self, id: &ContainerId) -> Result<String>;
    fn list(&self) -> Result<Vec<ContainerInfo>>;
    fn is_available(&self) -> bool;
}
```

## 8. Technical Stack Summary

| Component | Technology |
|---|---|
| Language | Rust (Edition 2024) |
| Parsing | nom 8 |
| System | nix 0.29, libc |
| CLI | clap 4.5 |
| TUI | ratatui 0.30 |
| Graph | petgraph 0.7 |
| Observability | aya 0.13 (eBPF) |
| Hashing | sha2 (SHA-256) |
| Serialization | serde, serde_json |
| Logging | tracing |
| VM Backend | QEMU (via `which` crate for detection) |
