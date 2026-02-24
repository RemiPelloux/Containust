//! End-to-end integration tests for the Containust runtime.
//!
//! These tests verify the full pipeline across cross-platform components:
//! 1. Parse `.ctst` files
//! 2. Validate composition (duplicate names, undefined references)
//! 3. Resolve dependency graph (topological order, cycle detection)
//! 4. Resolve connections (environment variable injection)
//! 5. Build/resolve images (source protocol, hashing, layers, catalog)
//! 6. State management (save/load roundtrip)
//! 7. Log management (append/read)
//! 8. Platform detection

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::cast_precision_loss)]

// ── Parsing ──────────────────────────────────────────────────────────

#[test]
fn pipeline_parse_hello_world_ctst() {
    let input = r#"
COMPONENT hello {
    image = "file:///tmp/test-rootfs"
    port = 8080
    command = ["/bin/echo", "Hello from Containust!"]
    memory = "128MiB"
    env = {
        RUST_LOG = "info"
    }
}
"#;

    let composition =
        containust_compose::parser::parse_ctst(input).expect("should parse hello world");
    assert_eq!(composition.components.len(), 1);

    let comp = &composition.components[0];
    assert_eq!(comp.name, "hello");
    assert_eq!(comp.image.as_deref(), Some("file:///tmp/test-rootfs"));
    assert_eq!(comp.port, Some(8080));
    assert_eq!(comp.command, vec!["/bin/echo", "Hello from Containust!"]);
    assert_eq!(comp.memory.as_deref(), Some("128MiB"));
    assert_eq!(comp.env.get("RUST_LOG").map(String::as_str), Some("info"));
}

#[test]
fn pipeline_parse_multi_component_with_connections() {
    let input = r#"
COMPONENT api {
    image = "file:///opt/images/api"
    port = 8080
    memory = "256MiB"
    command = ["./server"]
    env = {
        RUST_LOG = "debug"
    }
}

COMPONENT db {
    image = "file:///opt/images/postgres"
    port = 5432
    memory = "512MiB"
    env = {
        POSTGRES_USER = "admin"
        POSTGRES_PASSWORD = "secret"
    }
}

COMPONENT cache {
    image = "file:///opt/images/redis"
    port = 6379
    memory = "64MiB"
}

CONNECT api -> db
CONNECT api -> cache
"#;

    let composition =
        containust_compose::parser::parse_ctst(input).expect("should parse multi-component");
    assert_eq!(composition.components.len(), 3);
    assert_eq!(composition.connections.len(), 2);
}

// ── Dependency Graph ─────────────────────────────────────────────────

#[test]
fn pipeline_dependency_order_respects_connections() {
    let input = r#"
COMPONENT api {
    image = "file:///opt/images/api"
    port = 8080
}

COMPONENT db {
    image = "file:///opt/images/postgres"
    port = 5432
}

COMPONENT cache {
    image = "file:///opt/images/redis"
    port = 6379
}

CONNECT api -> db
CONNECT api -> cache
"#;

    let composition = containust_compose::parser::parse_ctst(input).expect("should parse");

    let mut graph = containust_compose::graph::DependencyGraph::new();
    let mut node_map = std::collections::HashMap::new();
    for comp in &composition.components {
        let idx = graph.add_component(&comp.name);
        let _ = node_map.insert(comp.name.clone(), idx);
    }
    for conn in &composition.connections {
        if let (Some(&from), Some(&to)) = (node_map.get(&conn.from), node_map.get(&conn.to)) {
            graph.add_dependency(from, to);
        }
    }

    let order = graph
        .resolve_order()
        .expect("should resolve without cycles");
    assert_eq!(order.len(), 3);

    let api_pos = order.iter().position(|n| n == "api").expect("api present");
    let db_pos = order.iter().position(|n| n == "db").expect("db present");
    let cache_pos = order
        .iter()
        .position(|n| n == "cache")
        .expect("cache present");
    assert!(db_pos < api_pos, "db must deploy before api");
    assert!(cache_pos < api_pos, "cache must deploy before api");
}

#[test]
fn pipeline_cycle_detection() {
    let mut graph = containust_compose::graph::DependencyGraph::new();
    let a = graph.add_component("a");
    let b = graph.add_component("b");
    let c = graph.add_component("c");
    graph.add_dependency(a, b);
    graph.add_dependency(b, c);
    graph.add_dependency(c, a);

    let result = graph.resolve_order();
    assert!(result.is_err(), "cycle should produce an error");
}

// ── Connection Resolution ────────────────────────────────────────────

#[test]
fn pipeline_connection_env_injection() {
    let input = r#"
COMPONENT web {
    image = "file:///opt/web"
    port = 3000
}

COMPONENT database {
    image = "file:///opt/postgres"
    port = 5432
}

CONNECT web -> database
"#;

    let composition = containust_compose::parser::parse_ctst(input).expect("should parse");
    let resolved = containust_compose::resolver::resolve_connections(&composition)
        .expect("should resolve connections");

    let web = resolved
        .iter()
        .find(|r| r.name == "web")
        .expect("web component");
    let has_host = web
        .env
        .iter()
        .any(|(k, v)| k == "DATABASE_HOST" && v == "database");
    let has_port = web
        .env
        .iter()
        .any(|(k, v)| k == "DATABASE_PORT" && v == "5432");
    assert!(has_host, "DATABASE_HOST should be injected");
    assert!(has_port, "DATABASE_PORT should be injected");
}

// ── Image Source Resolution ──────────────────────────────────────────

#[test]
fn pipeline_image_source_file_resolution() {
    let dir = tempfile::tempdir().expect("tempdir");
    let uri = format!("file://{}", dir.path().display());
    let source =
        containust_image::source::resolve_source(&uri).expect("should resolve file source");
    assert!(
        matches!(source, containust_image::source::ImageSource::File(_)),
        "expected File variant"
    );
}

#[test]
fn pipeline_image_source_tar_resolution() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tar_path = dir.path().join("image.tar");
    std::fs::write(&tar_path, b"").expect("create empty tar");

    let uri = format!("tar://{}", tar_path.display());
    let source = containust_image::source::resolve_source(&uri).expect("should resolve tar source");
    assert!(
        matches!(source, containust_image::source::ImageSource::Tar(_)),
        "expected Tar variant"
    );
}

#[test]
fn pipeline_image_source_remote_resolution() {
    let source = containust_image::source::resolve_source("https://example.com/image.tar")
        .expect("should resolve remote source");
    assert!(
        matches!(source, containust_image::source::ImageSource::Remote { .. }),
        "expected Remote variant"
    );
}

#[test]
fn pipeline_image_source_unknown_scheme_fails() {
    let result = containust_image::source::resolve_source("ftp://example.com/image.tar");
    assert!(result.is_err(), "unknown scheme should fail");
}

// ── SHA-256 Hashing ──────────────────────────────────────────────────

#[test]
fn pipeline_sha256_hashing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, b"hello world").expect("write test file");

    let hash = containust_image::hash::hash_file(&file_path).expect("should hash");
    assert_eq!(
        hash.as_hex(),
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}

#[test]
fn pipeline_sha256_hash_validation_success() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, b"hello world").expect("write test file");

    let expected = containust_common::types::Sha256Hash::from_hex(
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
    )
    .expect("valid hex");
    containust_image::hash::validate_hash(&file_path, &expected).expect("hash should match");
}

#[test]
fn pipeline_sha256_hash_validation_mismatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, b"hello world").expect("write test file");

    let wrong = containust_common::types::Sha256Hash::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .expect("valid hex");
    let result = containust_image::hash::validate_hash(&file_path, &wrong);
    assert!(result.is_err(), "mismatched hash should fail");
}

// ── Layer Extraction ─────────────────────────────────────────────────

#[test]
fn pipeline_layer_extraction() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tar_path = dir.path().join("test.tar");
    let extract_dir = dir.path().join("extracted");

    let file = std::fs::File::create(&tar_path).expect("create tar");
    let mut builder = tar::Builder::new(file);
    let data = b"test content";
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "test.txt", &data[..])
        .expect("append to tar");
    builder.finish().expect("finish tar");

    let layer = containust_image::layer::extract_layer(&tar_path, &extract_dir)
        .expect("should extract layer");
    assert!(extract_dir.join("test.txt").exists());
    assert!(layer.size_bytes > 0);
    assert!(!layer.hash.as_hex().is_empty());
}

// ── Image Catalog ────────────────────────────────────────────────────

#[test]
fn pipeline_image_catalog_crud() {
    let dir = tempfile::tempdir().expect("tempdir");
    let catalog = containust_image::registry::ImageCatalog::open(dir.path()).expect("open catalog");

    assert!(
        catalog.list().expect("list").is_empty(),
        "new catalog should be empty"
    );

    let entry = containust_image::registry::ImageEntry {
        id: containust_common::types::ImageId::new("test-1"),
        name: "test-image".into(),
        source: "file:///test".into(),
        layers: vec!["layer1".into()],
        size_bytes: 1024,
        created_at: "2026-01-01T00:00:00Z".into(),
    };
    catalog.register(entry).expect("register image");
    assert_eq!(catalog.list().expect("list").len(), 1);

    catalog
        .remove(&containust_common::types::ImageId::new("test-1"))
        .expect("remove image");
    assert!(catalog.list().expect("list").is_empty());
}

#[test]
fn pipeline_image_catalog_multiple_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    let catalog = containust_image::registry::ImageCatalog::open(dir.path()).expect("open catalog");

    for i in 0..3 {
        let entry = containust_image::registry::ImageEntry {
            id: containust_common::types::ImageId::new(format!("img-{i}")),
            name: format!("image-{i}"),
            source: format!("file:///opt/{i}"),
            layers: vec![],
            size_bytes: (i + 1) * 512,
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        catalog.register(entry).expect("register");
    }
    assert_eq!(catalog.list().expect("list").len(), 3);
}

// ── State Persistence ────────────────────────────────────────────────

#[test]
fn pipeline_state_persistence_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let state_path = dir.path().join("state.json");

    let state = containust_runtime::state::StateFile {
        containers: vec![containust_runtime::state::StateEntry {
            id: containust_common::types::ContainerId::new("test-container"),
            name: "web".into(),
            state: containust_common::types::ContainerState::Running,
            pid: Some(1234),
            image: "file:///test".into(),
            rootfs_path: None,
            log_path: None,
            created_at: "2026-01-01T00:00:00Z".into(),
        }],
    };

    containust_runtime::state::save_state(&state_path, &state).expect("save should succeed");

    let loaded = containust_runtime::state::load_state(&state_path).expect("load should succeed");
    assert_eq!(loaded.containers.len(), 1);
    assert_eq!(loaded.containers[0].name, "web");
    assert_eq!(loaded.containers[0].pid, Some(1234));
    assert_eq!(
        loaded.containers[0].state,
        containust_common::types::ContainerState::Running
    );
}

#[test]
fn pipeline_state_all_lifecycle_states() {
    use containust_common::types::{ContainerId, ContainerState};
    use containust_runtime::state::{StateEntry, StateFile};

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("state.json");

    let states = [
        ContainerState::Created,
        ContainerState::Running,
        ContainerState::Stopped,
        ContainerState::Failed,
    ];

    let state = StateFile {
        containers: states
            .iter()
            .enumerate()
            .map(|(i, s)| StateEntry {
                id: ContainerId::new(format!("c-{i}")),
                name: format!("container-{i}"),
                state: *s,
                pid: None,
                image: "img".into(),
                rootfs_path: None,
                log_path: None,
                created_at: "2026-01-01T00:00:00Z".into(),
            })
            .collect(),
    };

    containust_runtime::state::save_state(&path, &state).expect("save");
    let loaded = containust_runtime::state::load_state(&path).expect("load");
    assert_eq!(loaded.containers.len(), 4);
    assert_eq!(loaded.containers[0].state, ContainerState::Created);
    assert_eq!(loaded.containers[1].state, ContainerState::Running);
    assert_eq!(loaded.containers[2].state, ContainerState::Stopped);
    assert_eq!(loaded.containers[3].state, ContainerState::Failed);
}

// ── Log Management ───────────────────────────────────────────────────

#[test]
fn pipeline_log_append_and_read() {
    let dir = tempfile::tempdir().expect("tempdir");

    containust_runtime::logs::append_log(dir.path(), "test-container", "line 1").expect("append 1");
    containust_runtime::logs::append_log(dir.path(), "test-container", "line 2").expect("append 2");

    let logs =
        containust_runtime::logs::read_logs(dir.path(), "test-container").expect("read logs");
    assert!(logs.contains("line 1"));
    assert!(logs.contains("line 2"));
}

#[test]
fn pipeline_log_isolation_between_containers() {
    let dir = tempfile::tempdir().expect("tempdir");

    containust_runtime::logs::append_log(dir.path(), "alpha", "alpha msg").expect("append alpha");
    containust_runtime::logs::append_log(dir.path(), "beta", "beta msg").expect("append beta");

    let alpha_logs = containust_runtime::logs::read_logs(dir.path(), "alpha").expect("read alpha");
    let beta_logs = containust_runtime::logs::read_logs(dir.path(), "beta").expect("read beta");

    assert!(alpha_logs.contains("alpha msg"));
    assert!(!alpha_logs.contains("beta msg"));
    assert!(beta_logs.contains("beta msg"));
    assert!(!beta_logs.contains("alpha msg"));
}

// ── Validation Errors ────────────────────────────────────────────────

#[test]
fn pipeline_validator_rejects_duplicate_names() {
    let input = r#"
COMPONENT app {
    image = "file:///test"
}
COMPONENT app {
    image = "file:///test2"
}
"#;

    let result = containust_compose::parser::parse_ctst(input);
    assert!(
        result.is_err(),
        "duplicate component names should be rejected"
    );
}

#[test]
fn pipeline_validator_rejects_undefined_connection() {
    let input = r#"
COMPONENT web {
    image = "file:///test"
}
CONNECT web -> nonexistent
"#;

    let result = containust_compose::parser::parse_ctst(input);
    assert!(
        result.is_err(),
        "undefined connection target should be rejected"
    );
}

#[test]
fn pipeline_validator_rejects_missing_image() {
    let input = r"
COMPONENT bare {
    port = 8080
}
";

    let result = containust_compose::parser::parse_ctst(input);
    assert!(
        result.is_err(),
        "component without image or FROM should be rejected"
    );
}

// ── Template Inheritance ─────────────────────────────────────────────

#[test]
fn pipeline_from_template_parsing() {
    let input = r#"
COMPONENT base {
    image = "file:///opt/base"
    port = 8080
}
COMPONENT web FROM base {
    memory = "256MiB"
}
"#;

    let composition =
        containust_compose::parser::parse_ctst(input).expect("should parse FROM template");
    assert_eq!(composition.components.len(), 2);

    let web = composition
        .components
        .iter()
        .find(|c| c.name == "web")
        .expect("web component");
    assert_eq!(web.from_template.as_deref(), Some("base"));
    assert_eq!(web.memory.as_deref(), Some("256MiB"));
}

// ── IMPORT Parsing ───────────────────────────────────────────────────

#[test]
fn pipeline_import_parsing() {
    let input = r#"
IMPORT "templates/postgres.ctst" AS pg
COMPONENT web {
    image = "file:///opt/web"
}
"#;

    let composition = containust_compose::parser::parse_ctst(input).expect("should parse IMPORT");
    assert_eq!(composition.imports.len(), 1);
    assert_eq!(composition.imports[0].source, "templates/postgres.ctst");
    assert_eq!(composition.imports[0].alias.as_deref(), Some("pg"));
}

// ── Healthcheck Parsing ──────────────────────────────────────────────

#[test]
fn pipeline_healthcheck_parsing() {
    let input = r#"
COMPONENT web {
    image = "file:///opt/web"
    port = 8080
    healthcheck = {
        command = ["curl", "-f", "http://localhost:8080/health"]
        interval = "30s"
        timeout = "5s"
        retries = 3
    }
}
"#;

    let composition =
        containust_compose::parser::parse_ctst(input).expect("should parse healthcheck");
    let comp = &composition.components[0];
    assert!(comp.healthcheck.is_some());
    let hc = comp.healthcheck.as_ref().expect("healthcheck present");
    assert_eq!(
        hc.command,
        vec!["curl", "-f", "http://localhost:8080/health"]
    );
    assert_eq!(hc.interval.as_deref(), Some("30s"));
    assert_eq!(hc.timeout.as_deref(), Some("5s"));
    assert_eq!(hc.retries, Some(3));
}

// ── Platform Detection ───────────────────────────────────────────────

#[test]
fn pipeline_platform_detection() {
    let info = containust_runtime::backend::platform_info();
    assert!(!info.os.is_empty());
    assert!(!info.arch.is_empty());
    if cfg!(target_os = "macos") {
        assert!(
            !info.native_available,
            "macOS does not support native Linux backend"
        );
    }
}

// ── Byte Formatting (utility logic) ──────────────────────────────────

#[test]
fn pipeline_format_bytes() {
    fn format_bytes(bytes: u64) -> String {
        const KIB: u64 = 1024;
        const MIB: u64 = KIB * 1024;
        const GIB: u64 = MIB * 1024;
        if bytes >= GIB {
            format!("{:.1} GiB", bytes as f64 / GIB as f64)
        } else if bytes >= MIB {
            format!("{:.1} MiB", bytes as f64 / MIB as f64)
        } else if bytes >= KIB {
            format!("{:.1} KiB", bytes as f64 / KIB as f64)
        } else {
            format!("{bytes} B")
        }
    }
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.0 KiB");
    assert_eq!(format_bytes(1_048_576), "1.0 MiB");
    assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
}

// ── Advanced Parsing Scenarios ───────────────────────────────────────

#[test]
fn pipeline_component_with_all_properties() {
    let input = r#"
COMPONENT fullstack {
    image = "file:///opt/app"
    port = 8080
    ports = [8080, 8443]
    memory = "512MiB"
    cpu = "2"
    volume = "/data"
    volumes = ["/data", "/logs"]
    command = ["./app", "--config", "/etc/app.toml"]
    readonly = true
    workdir = "/app"
    user = "nobody"
    hostname = "fullstack-host"
    restart = "always"
    network = "bridge"
    env = {
        APP_ENV = "production"
        LOG_LEVEL = "warn"
    }
}
"#;

    let composition =
        containust_compose::parser::parse_ctst(input).expect("should parse all properties");
    let comp = &composition.components[0];
    assert_eq!(comp.name, "fullstack");
    assert_eq!(comp.port, Some(8080));
    assert_eq!(comp.ports, vec![8080, 8443]);
    assert_eq!(comp.cpu.as_deref(), Some("2"));
    assert_eq!(comp.volume.as_deref(), Some("/data"));
    assert_eq!(comp.volumes, vec!["/data", "/logs"]);
    assert_eq!(comp.readonly, Some(true));
    assert_eq!(comp.workdir.as_deref(), Some("/app"));
    assert_eq!(comp.user.as_deref(), Some("nobody"));
    assert_eq!(comp.hostname.as_deref(), Some("fullstack-host"));
    assert_eq!(comp.restart.as_deref(), Some("always"));
    assert_eq!(comp.network.as_deref(), Some("bridge"));
    assert_eq!(comp.env.len(), 2);
}

#[test]
fn pipeline_empty_input_produces_empty_ast() {
    let input = "";
    let composition = containust_compose::parser::parse_ctst(input).expect("empty input is valid");
    assert!(composition.components.is_empty());
    assert!(composition.connections.is_empty());
    assert!(composition.imports.is_empty());
}

#[test]
fn pipeline_comments_are_ignored() {
    let input = r#"
// This is a comment
COMPONENT app {
    // Another comment
    image = "file:///opt/app"
}
"#;

    let composition =
        containust_compose::parser::parse_ctst(input).expect("comments should be ignored");
    assert_eq!(composition.components.len(), 1);
    assert_eq!(composition.components[0].name, "app");
}

// ── Storage Backend ──────────────────────────────────────────────────

#[test]
fn pipeline_storage_backend_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = containust_image::storage::StorageBackend::open(dir.path().to_path_buf())
        .expect("open storage");

    let layer_path = storage.layer_path("abcdef1234567890");
    assert!(
        layer_path
            .to_str()
            .expect("utf-8")
            .contains("abcdef1234567890"),
    );
    assert!(!storage.has_layer("nonexistent"));
    assert_eq!(storage.root(), dir.path());
}

// ── Resolved Component ──────────────────────────────────────────────

#[test]
fn pipeline_resolver_preserves_component_env() {
    let input = r#"
COMPONENT svc {
    image = "file:///opt/svc"
    env = {
        KEY = "value"
        OTHER = "data"
    }
}
"#;

    let composition = containust_compose::parser::parse_ctst(input).expect("should parse");
    let resolved =
        containust_compose::resolver::resolve_connections(&composition).expect("should resolve");

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "svc");
    assert!(
        resolved[0]
            .env
            .iter()
            .any(|(k, v)| k == "KEY" && v == "value"),
        "original env should be preserved"
    );
    assert!(
        resolved[0]
            .env
            .iter()
            .any(|(k, v)| k == "OTHER" && v == "data"),
    );
}

// ── Common Types ─────────────────────────────────────────────────────

#[test]
fn pipeline_container_id_generate_unique() {
    let id1 = containust_common::types::ContainerId::generate();
    let id2 = containust_common::types::ContainerId::generate();
    assert_ne!(id1, id2, "generated IDs should be unique");
    assert!(!id1.as_str().is_empty());
}

#[test]
fn pipeline_sha256_hash_display() {
    let hash = containust_common::types::Sha256Hash::from_hex(
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9",
    )
    .expect("valid hex");
    let display = format!("{hash}");
    assert!(
        display.starts_with("sha256:"),
        "Display should prefix with sha256:"
    );
}

#[test]
fn pipeline_sha256_hash_invalid_hex_rejected() {
    let result = containust_common::types::Sha256Hash::from_hex("not-a-valid-hex");
    assert!(result.is_err(), "invalid hex should be rejected");

    let result = containust_common::types::Sha256Hash::from_hex("abcdef");
    assert!(result.is_err(), "too short hex should be rejected");
}

#[test]
fn pipeline_container_state_display() {
    assert_eq!(
        format!("{}", containust_common::types::ContainerState::Created),
        "created"
    );
    assert_eq!(
        format!("{}", containust_common::types::ContainerState::Running),
        "running"
    );
    assert_eq!(
        format!("{}", containust_common::types::ContainerState::Stopped),
        "stopped"
    );
    assert_eq!(
        format!("{}", containust_common::types::ContainerState::Failed),
        "failed"
    );
}
