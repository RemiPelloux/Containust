# Error Reference

> Comprehensive catalog of all error codes produced by **Containust** — a daemon-less, sovereign container runtime written in Rust.

Every error includes a unique code, a human-readable message, a common trigger, and a resolution. Errors are designed to be **actionable**: each message tells you *what* went wrong and *where*, so you can fix it without guessing.

---

## Error Format

Containust formats errors using Rust's `thiserror` derive macros. The base `ContainustError` enum produces the following display strings:

| Variant | Format String |
|---|---|
| `Io` | `I/O error at {path}: {source}` |
| `Config` | `invalid configuration: {message}` |
| `NotFound` | `{kind} not found: {id}` |
| `HashMismatch` | `hash mismatch for {resource}: expected {expected}, got {actual}` |
| `PermissionDenied` | `permission denied: {message}` |
| `Serialization` | `serialization error: {source}` |

Domain-specific crates wrap these variants in their own error enums and attach contextual codes (see sections below).

---

## Error Categories

| Prefix | Category | Phase | Severity |
|---|---|---|---|
| `E0xx` | Parse errors | `ctst build` / `ctst plan` | Error or Warning |
| `R0xx` | Runtime errors | Container lifecycle | Error |
| `I0xx` | Image errors | Image operations | Error |
| `S0xx` | State errors | State file I/O | Error |

---

## Parse Errors (E0xx)

Reported by the `.ctst` parser in `containust-compose` during `ctst build` or `ctst plan`. The entire file is validated before any container is created.

### E001 — Unexpected Token

| Field | Value |
|---|---|
| **Code** | `E001` |
| **Message** | `unexpected token '{token}' at line {line}, column {col}` |
| **Example** | `COMPNENT app { image = "file:///img" }` (typo in keyword) |
| **Resolution** | Check the spelling of keywords. Valid keywords: `IMPORT`, `AS`, `COMPONENT`, `FROM`, `CONNECT`, `EXPOSE`, `HEALTHCHECK`, `RESTART`, `NETWORK`, `SECRET`. |

### E002 — Undefined Component Reference

| Field | Value |
|---|---|
| **Code** | `E002` |
| **Message** | `undefined component reference: '{name}'` |
| **Example** | `CONNECT api -> database` when only `db` is defined |
| **Resolution** | Verify the component name matches an existing `COMPONENT` block or an imported template. Check for typos. |

### E003 — Duplicate Component Name

| Field | Value |
|---|---|
| **Code** | `E003` |
| **Message** | `duplicate component name: '{name}' already defined at line {line}` |
| **Example** | Two `COMPONENT api { ... }` blocks in the same file |
| **Resolution** | Rename one of the components. Each name must be unique within a file. |

### E004 — Cyclic Dependency Detected

| Field | Value |
|---|---|
| **Code** | `E004` |
| **Message** | `cyclic dependency detected: {cycle_path}` |
| **Example** | `CONNECT a -> b` and `CONNECT b -> a` |
| **Resolution** | Redesign the dependency graph to eliminate cycles. Use `ctst plan` to visualize the graph and identify the cycle. |

### E005 — Missing Required Property

| Field | Value |
|---|---|
| **Code** | `E005` |
| **Message** | `missing required property '{property}' on component '{name}'` |
| **Example** | `COMPONENT app { port = 8080 }` (no `image` property) |
| **Resolution** | Add the missing property. `image` is always required unless inherited via `FROM`. |

### E006 — Type Mismatch

| Field | Value |
|---|---|
| **Code** | `E006` |
| **Message** | `type mismatch on '{property}': expected {expected}, got {actual}` |
| **Example** | `port = "eight-thousand"` (string where integer is expected) |
| **Resolution** | Provide a value of the correct type. See the type system reference in [CTST_LANG.md](CTST_LANG.md#4-type-system). |

### E007 — Invalid Image URI

| Field | Value |
|---|---|
| **Code** | `E007` |
| **Message** | `invalid image URI: '{uri}'` |
| **Example** | `image = "ftp:///opt/images/app"` (unsupported protocol) |
| **Resolution** | Use a supported protocol: `file://`, `tar://`, or `https://`. Plain `http://` is rejected for security. |

### E008 — Unresolved Import

| Field | Value |
|---|---|
| **Code** | `E008` |
| **Message** | `unresolved import: '{path}' not found` |
| **Example** | `IMPORT "templates/missing.ctst" AS tmpl` when the file does not exist |
| **Resolution** | Verify the import path is correct and the file exists. Paths are resolved relative to the importing file, then relative to the project root. |

### E009 — Unused Import (Warning)

| Field | Value |
|---|---|
| **Code** | `E009` |
| **Severity** | Warning |
| **Message** | `unused import: '{alias}' is imported but never referenced` |
| **Example** | `IMPORT "templates/redis.ctst" AS cache` with no `FROM cache` usage |
| **Resolution** | Remove the import or use it in a `COMPONENT ... FROM` clause. Unused imports increase parse time and clutter the file. |

### E010 — Unreachable Component (Warning)

| Field | Value |
|---|---|
| **Code** | `E010` |
| **Severity** | Warning |
| **Message** | `unreachable component: '{name}' is not referenced by any CONNECT or EXPOSE` |
| **Example** | A `COMPONENT logger { ... }` that no other component connects to or exposes |
| **Resolution** | Either connect it via `CONNECT` / `EXPOSE`, or remove it if it is truly unused. |

---

## Runtime Errors (R0xx)

Reported by `containust-runtime` during container lifecycle operations (`ctst run`, `ctst exec`, `ctst stop`).

### R001 — Namespace Creation Failed

| Field | Value |
|---|---|
| **Code** | `R001` |
| **Message** | `failed to create {ns_type} namespace: {reason}` |
| **Cause** | Missing kernel support, insufficient privileges, or `user.max_user_namespaces` sysctl set too low. |
| **Resolution** | Ensure the kernel supports the requested namespace type (PID, NET, MNT, UTS, IPC, USER). Check `cat /proc/sys/user/max_user_namespaces` and increase if needed. |

### R002 — Cgroup Setup Failed

| Field | Value |
|---|---|
| **Code** | `R002` |
| **Message** | `failed to configure cgroup at {path}: {reason}` |
| **Cause** | cgroups v2 not enabled, cgroup filesystem not mounted, or permission denied on `/sys/fs/cgroup`. |
| **Resolution** | Verify cgroups v2 is active: `mount | grep cgroup2`. Ensure the user has write access to the target cgroup hierarchy. |

### R003 — Mount Operation Failed

| Field | Value |
|---|---|
| **Code** | `R003` |
| **Message** | `mount failed at {mountpoint}: {reason}` |
| **Cause** | Invalid volume path, permission denied, or OverlayFS not available. |
| **Resolution** | Verify all volume host paths exist and are accessible. Ensure the kernel supports OverlayFS (`modprobe overlay`). |

### R004 — Process Spawn Failed

| Field | Value |
|---|---|
| **Code** | `R004` |
| **Message** | `failed to spawn process '{command}': {reason}` |
| **Cause** | Binary not found in the container rootfs, missing shared libraries, or permission denied. |
| **Resolution** | Verify the `command` binary exists in the image and has execute permission. For dynamically linked binaries, ensure all required `.so` files are present. |

### R005 — Container Not Found

| Field | Value |
|---|---|
| **Code** | `R005` |
| **Message** | `container not found: '{id}'` |
| **Cause** | The container ID does not exist in the state file. It may have been stopped and cleaned up. |
| **Resolution** | Run `ctst ps` to list active containers. Check the state file path if using `--state-file`. |

### R006 — Container Already Running

| Field | Value |
|---|---|
| **Code** | `R006` |
| **Message** | `container '{id}' is already running` |
| **Cause** | Attempting to start a container that is already in the `Running` state. |
| **Resolution** | Stop the existing container with `ctst stop {id}` before starting a new instance, or use a different component name. |

### R007 — Container Not Running

| Field | Value |
|---|---|
| **Code** | `R007` |
| **Message** | `container '{id}' is not running` |
| **Cause** | Attempting to exec into or stop a container that is in the `Created` or `Stopped` state. |
| **Resolution** | Start the container first with `ctst run`, or check its status with `ctst ps`. |

### R008 — Exec Failed

| Field | Value |
|---|---|
| **Code** | `R008` |
| **Message** | `exec failed in container '{id}': {reason}` |
| **Cause** | The command binary does not exist inside the container, or the container's PID namespace is inaccessible. |
| **Resolution** | Verify the command exists in the container image. If the rootfs is read-only, the binary must be part of the original image. |

---

## Image Errors (I0xx)

Reported by `containust-image` during image fetch, extraction, and validation operations.

### I001 — Image Not Found

| Field | Value |
|---|---|
| **Code** | `I001` |
| **Message** | `image not found: '{uri}'` |
| **Cause** | The `file://` path does not exist, or the `tar://` archive is missing. |
| **Resolution** | Verify the image path exists on disk. For `file://`, check that the directory contains a valid rootfs. For `tar://`, check that the archive exists and is readable. |

### I002 — Hash Mismatch

| Field | Value |
|---|---|
| **Code** | `I002` |
| **Message** | `hash mismatch for {resource}: expected {expected}, got {actual}` |
| **Cause** | The image or layer content has been modified since it was last verified. Possible corruption, incomplete download, or tampering. |
| **Resolution** | Re-download or re-extract the image from a trusted source. If using `tar://`, verify the archive integrity with `sha256sum`. |

### I003 — Extraction Failed

| Field | Value |
|---|---|
| **Code** | `I003` |
| **Message** | `failed to extract image from '{path}': {reason}` |
| **Cause** | The tar archive is malformed, the disk is full, or the target directory is not writable. |
| **Resolution** | Verify the tar archive is valid (`tar -tf archive.tar`). Check available disk space with `df -h`. Ensure the extraction target directory has write permissions. |

### I004 — Invalid Tar Archive

| Field | Value |
|---|---|
| **Code** | `I004` |
| **Message** | `invalid tar archive: '{path}'` |
| **Cause** | The file is not a valid tar archive, is truncated, or uses an unsupported compression format. |
| **Resolution** | Verify the file is a plain tar archive (not gzipped or otherwise compressed unless explicitly supported). Recreate the archive if corrupted. |

### I005 — Remote Fetch Forbidden

| Field | Value |
|---|---|
| **Code** | `I005` |
| **Message** | `remote fetch forbidden: '{uri}' blocked by --offline mode` |
| **Cause** | An `https://` image source was used while the `--offline` flag is active. |
| **Resolution** | Either remove the `--offline` flag, or replace the image source with a local `file://` or `tar://` protocol. Pre-download images for air-gapped deployments. |

### I006 — Download Failed

| Field | Value |
|---|---|
| **Code** | `I006` |
| **Message** | `download failed for '{uri}': {reason}` |
| **Cause** | Network error, DNS resolution failure, TLS certificate error, or the remote registry returned an error. |
| **Resolution** | Check network connectivity. Verify the URL is correct and the registry is reachable. For TLS errors, ensure system certificates are up to date. |

---

## State Errors (S0xx)

Reported when reading or writing the state index file (`state.json`).

### S001 — State File Corrupt

| Field | Value |
|---|---|
| **Code** | `S001` |
| **Message** | `state file corrupt: failed to parse '{path}'` |
| **Cause** | The state file contains invalid JSON, was partially written, or was modified by an external tool. |
| **Resolution** | Delete the state file and re-run `ctst run`. The runtime will recreate it. Back up the file first if you need to inspect it: `cp state.json state.json.bak`. |

### S002 — State File Locked

| Field | Value |
|---|---|
| **Code** | `S002` |
| **Message** | `state file locked: '{path}' is held by another process (PID {pid})` |
| **Cause** | Another `ctst` process is currently reading or writing the state file. |
| **Resolution** | Wait for the other process to finish, or verify it is not stale. If the lock is stale (process no longer running), remove the lock file manually: `rm state.json.lock`. |

### S003 — Permission Denied on State File

| Field | Value |
|---|---|
| **Code** | `S003` |
| **Message** | `permission denied: cannot access state file '{path}'` |
| **Cause** | The current user does not have read/write access to the state file or its parent directory. |
| **Resolution** | Check file permissions with `ls -la state.json`. Adjust ownership or permissions, or use `--state-file` to specify an accessible path. |

### S004 — State File Not Found

| Field | Value |
|---|---|
| **Code** | `S004` |
| **Message** | `state file not found: '{path}'` |
| **Cause** | No state file exists at the expected location. This is normal on first run. |
| **Resolution** | If this is a fresh deployment, no action is needed — `ctst run` creates the state file automatically. If you expected existing state, verify the `--state-file` path. |

---

## Mapping to `ContainustError` Variants

Each error code maps to one or more variants of the Rust `ContainustError` enum defined in `containust-common`:

| Error Code | `ContainustError` Variant | Key Fields |
|---|---|---|
| E001–E008 | `Config` | `message` contains the parse error details |
| E009–E010 | `Config` | `message` contains the warning text |
| R001 | `Io` | `path` = namespace pseudo-file |
| R002 | `Io` | `path` = cgroup directory |
| R003 | `Io` | `path` = mountpoint |
| R004 | `Io` | `path` = binary path |
| R005 | `NotFound` | `kind` = `"container"`, `id` = container ID |
| R006 | `Config` | `message` = already running notice |
| R007 | `Config` | `message` = not running notice |
| R008 | `Io` | `path` = exec binary path |
| I001 | `NotFound` | `kind` = `"image"`, `id` = URI |
| I002 | `HashMismatch` | `resource`, `expected`, `actual` |
| I003 | `Io` | `path` = archive path |
| I004 | `Config` | `message` = archive validation failure |
| I005 | `PermissionDenied` | `message` = offline restriction |
| I006 | `Io` | `path` = download target |
| S001 | `Serialization` | `source` = `serde_json::Error` |
| S002 | `Io` | `path` = lock file path |
| S003 | `PermissionDenied` | `message` = file access denial |
| S004 | `NotFound` | `kind` = `"state file"`, `id` = path |

---

## Programmatic Error Handling

When using Containust as a Rust library via `containust-sdk`, match on `ContainustError` variants to implement recovery strategies:

```rust
use containust_common::error::ContainustError;

fn handle_error(err: ContainustError) {
    match err {
        ContainustError::Io { path, source } => {
            eprintln!("I/O failure at {}: {source}", path.display());
            // Retry transient errors, abort on permanent ones.
        }
        ContainustError::NotFound { kind, id } => {
            eprintln!("{kind} '{id}' does not exist.");
            // Prompt the user to create the resource or check the ID.
        }
        ContainustError::HashMismatch { resource, expected, actual } => {
            eprintln!("Integrity check failed for {resource}.");
            eprintln!("  expected: {expected}");
            eprintln!("  actual:   {actual}");
            // Re-download or re-extract from a trusted source.
        }
        ContainustError::Config { message } => {
            eprintln!("Configuration error: {message}");
            // Validate and fix the .ctst file or runtime config.
        }
        ContainustError::PermissionDenied { message } => {
            eprintln!("Access denied: {message}");
            // Check user privileges, file permissions, or capabilities.
        }
        ContainustError::Serialization { source } => {
            eprintln!("State file error: {source}");
            // Delete and recreate the state file.
        }
    }
}
```

---

*Built with Rust. Designed for sovereignty.*
