//! `IMPORT` and `FROM` resolution.
//!
//! Imported files contribute their components as *templates*: they are in
//! scope for `FROM` but are not deployed themselves. Without an alias a
//! template is named after its component; `IMPORT "f.ctst" AS lib` names
//! it `lib.<component>`, and `lib` alone when the file declares exactly one
//! component. Local components are templates too (and are deployed).
//!
//! A child inherits every property of its parent and overrides the ones it
//! sets; `env` maps are merged key by key. Chains are allowed; circular
//! inheritance and circular imports are rejected. Remote (`https://`)
//! imports are not supported yet and fail closed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use containust_common::error::{ContainustError, Result};

use crate::parser::ast::{ComponentDecl, CompositionFile, ImportDecl};

/// Maximum depth of nested imports and of a `FROM` chain.
pub const MAX_DEPTH: usize = 16;

/// Reads, parses, and resolves a `.ctst` file, including its imports and
/// template inheritance.
///
/// # Errors
///
/// Returns an error when the file or an import cannot be read or parsed,
/// an import is remote or circular, or a `FROM` template is missing or
/// circular.
pub fn load_file(path: &Path) -> Result<CompositionFile> {
    let content = std::fs::read_to_string(path).map_err(|source| ContainustError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let file = crate::parser::parse_ctst(&content)?;
    let mut stack: Vec<PathBuf> = std::fs::canonicalize(path).into_iter().collect();
    resolve_in(file, &base_dir(path), &mut stack)
}

/// Resolves the imports and `FROM` chains of an already parsed file whose
/// relative imports are relative to `base_dir`.
///
/// # Errors
///
/// See [`load_file`].
pub fn resolve(file: CompositionFile, base_dir: &Path) -> Result<CompositionFile> {
    resolve_in(file, base_dir, &mut Vec::new())
}

fn base_dir(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn resolve_in(
    mut file: CompositionFile,
    base_dir: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<CompositionFile> {
    let imported = load_imports(&file.imports, base_dir, stack)?;
    let locals: HashMap<&str, &ComponentDecl> = file
        .components
        .iter()
        .map(|component| (component.name.as_str(), component))
        .collect();
    let scope = Scope {
        locals: &locals,
        imported: &imported,
    };
    let resolved = file
        .components
        .iter()
        .map(|component| scope.inherit(component, &mut Vec::new()))
        .collect::<Result<Vec<_>>>()?;
    file.components = resolved;
    Ok(file)
}

/// Loads every import (recursively resolved) into a template table.
fn load_imports(
    imports: &[ImportDecl],
    base_dir: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<HashMap<String, ComponentDecl>> {
    let mut templates = HashMap::new();
    for import in imports {
        let file = load_import(import, base_dir, stack)?;
        let single = file.components.len() == 1;
        for component in file.components {
            if let (Some(alias), true) = (&import.alias, single) {
                insert_template(&mut templates, alias.clone(), component.clone())?;
            }
            let key = import.alias.as_ref().map_or_else(
                || component.name.clone(),
                |alias| format!("{alias}.{}", component.name),
            );
            insert_template(&mut templates, key, component)?;
        }
    }
    Ok(templates)
}

fn load_import(
    import: &ImportDecl,
    base_dir: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<CompositionFile> {
    let source = import.source.as_str();
    if source.contains("://") {
        return Err(config(format!(
            "remote IMPORT is not supported yet: {source} \
             (download the file and import it by path)"
        )));
    }
    let path = base_dir.join(source);
    let canonical = std::fs::canonicalize(&path).map_err(|_| ContainustError::NotFound {
        kind: "import file",
        id: path.display().to_string(),
    })?;
    if stack.contains(&canonical) {
        return Err(config(format!(
            "circular IMPORT of {}",
            canonical.display()
        )));
    }
    if stack.len() >= MAX_DEPTH {
        return Err(config(format!(
            "IMPORT nesting deeper than {MAX_DEPTH} at {}",
            canonical.display()
        )));
    }
    let content = std::fs::read_to_string(&canonical).map_err(|source| ContainustError::Io {
        path: canonical.clone(),
        source,
    })?;
    let parsed = crate::parser::parse_ctst(&content)?;
    stack.push(canonical.clone());
    let resolved = resolve_in(parsed, &base_dir_of(&canonical), stack);
    let _ = stack.pop();
    resolved
}

fn base_dir_of(canonical: &Path) -> PathBuf {
    canonical
        .parent()
        .map_or_else(|| PathBuf::from("/"), Path::to_path_buf)
}

fn insert_template(
    templates: &mut HashMap<String, ComponentDecl>,
    key: String,
    component: ComponentDecl,
) -> Result<()> {
    if templates.contains_key(&key) {
        return Err(config(format!(
            "template '{key}' is imported more than once; use IMPORT ... AS <alias>"
        )));
    }
    let _ = templates.insert(key, component);
    Ok(())
}

/// Templates visible to the components of one file.
struct Scope<'a> {
    locals: &'a HashMap<&'a str, &'a ComponentDecl>,
    imported: &'a HashMap<String, ComponentDecl>,
}

impl Scope<'_> {
    /// Returns `component` with its whole `FROM` chain merged in.
    fn inherit(&self, component: &ComponentDecl, chain: &mut Vec<String>) -> Result<ComponentDecl> {
        let Some(parent_name) = component.from_template.as_deref() else {
            return Ok(component.clone());
        };
        chain.push(component.name.clone());
        if chain.iter().any(|name| name == parent_name) || chain.len() > MAX_DEPTH {
            return Err(config(format!(
                "circular FROM inheritance: {} -> {parent_name}",
                chain.join(" -> ")
            )));
        }
        let parent = self
            .locals
            .get(parent_name)
            .copied()
            .or_else(|| self.imported.get(parent_name))
            .ok_or_else(|| {
                config(format!(
                    "component '{}' inherits FROM unknown template '{parent_name}' \
                     (define it in this file or IMPORT it)",
                    component.name
                ))
            })?;
        // The root of every chain has an image: the parser rejects a
        // component with neither `image` nor `FROM`.
        let parent = self.inherit(parent, chain)?;
        Ok(merge(&parent, component))
    }
}

/// Child properties override the parent's; `env` maps are merged.
fn merge(parent: &ComponentDecl, child: &ComponentDecl) -> ComponentDecl {
    let pick_vec = |own: &Vec<String>, base: &Vec<String>| {
        if own.is_empty() {
            base.clone()
        } else {
            own.clone()
        }
    };
    let mut env = parent.env.clone();
    env.extend(child.env.clone());
    ComponentDecl {
        name: child.name.clone(),
        from_template: None,
        image: child.image.clone().or_else(|| parent.image.clone()),
        port: child.port.or(parent.port),
        ports: if child.ports.is_empty() {
            parent.ports.clone()
        } else {
            child.ports.clone()
        },
        memory: child.memory.clone().or_else(|| parent.memory.clone()),
        cpu: child.cpu.clone().or_else(|| parent.cpu.clone()),
        env,
        volume: child.volume.clone().or_else(|| parent.volume.clone()),
        volumes: pick_vec(&child.volumes, &parent.volumes),
        command: pick_vec(&child.command, &parent.command),
        entrypoint: child
            .entrypoint
            .clone()
            .or_else(|| parent.entrypoint.clone()),
        readonly: child.readonly.or(parent.readonly),
        workdir: child.workdir.clone().or_else(|| parent.workdir.clone()),
        user: child.user.clone().or_else(|| parent.user.clone()),
        hostname: child.hostname.clone().or_else(|| parent.hostname.clone()),
        restart: child.restart.clone().or_else(|| parent.restart.clone()),
        network: child.network.clone().or_else(|| parent.network.clone()),
        healthcheck: child
            .healthcheck
            .clone()
            .or_else(|| parent.healthcheck.clone()),
        update: child.update.clone().or_else(|| parent.update.clone()),
        stop_grace: child
            .stop_grace
            .clone()
            .or_else(|| parent.stop_grace.clone()),
    }
}

const fn config(message: String) -> ContainustError {
    ContainustError::Config { message }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&path, body).expect("write");
        path
    }

    fn component<'a>(file: &'a CompositionFile, name: &str) -> &'a ComponentDecl {
        file.components
            .iter()
            .find(|c| c.name == name)
            .expect("component")
    }

    #[test]
    fn local_template_is_merged_with_env_overrides() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(
            dir.path(),
            "stack.ctst",
            r#"COMPONENT base_worker {
    image   = "file:///opt/worker"
    memory  = "128MiB"
    restart = "on-failure"
    env     = { QUEUE = "default"  LOG = "info" }
}
COMPONENT payments FROM base_worker {
    memory = "256MiB"
    env    = { QUEUE = "payments" }
}
"#,
        );
        let file = load_file(&path).expect("load");
        let payments = component(&file, "payments");
        assert_eq!(payments.image.as_deref(), Some("file:///opt/worker"));
        assert_eq!(payments.memory.as_deref(), Some("256MiB"));
        assert_eq!(payments.restart.as_deref(), Some("on-failure"));
        assert_eq!(
            payments.env.get("QUEUE").map(String::as_str),
            Some("payments")
        );
        assert_eq!(payments.env.get("LOG").map(String::as_str), Some("info"));
        assert!(payments.from_template.is_none());
        assert_eq!(file.components.len(), 2, "local templates still deploy");
    }

    #[test]
    fn imported_templates_resolve_by_name_alias_and_dotted_alias() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _ = write(
            dir.path(),
            "lib/redis.ctst",
            "COMPONENT redis {\n image = \"file:///redis\"\n port = 6379\n}\n",
        );
        let _ = write(
            dir.path(),
            "lib/dbs.ctst",
            "COMPONENT postgres {\n image = \"file:///pg\"\n}\nCOMPONENT mysql {\n image = \"file:///my\"\n}\n",
        );
        let path = write(
            dir.path(),
            "stack.ctst",
            r#"IMPORT "lib/redis.ctst"
IMPORT "lib/redis.ctst" AS r
IMPORT "lib/dbs.ctst" AS dbs
COMPONENT cache FROM redis { memory = "64MiB" }
COMPONENT cache2 FROM r { }
COMPONENT db FROM dbs.postgres { port = 5432 }
"#,
        );
        let file = load_file(&path).expect("load");
        assert_eq!(file.components.len(), 3, "imports are templates only");
        assert_eq!(component(&file, "cache").port, Some(6379));
        assert_eq!(
            component(&file, "cache2").image.as_deref(),
            Some("file:///redis")
        );
        assert_eq!(component(&file, "db").image.as_deref(), Some("file:///pg"));
    }

    #[test]
    fn chained_templates_resolve_through_imports() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _ = write(
            dir.path(),
            "a.ctst",
            "COMPONENT root {\n image = \"file:///root\"\n cpu = \"512\"\n}\n",
        );
        let _ = write(
            dir.path(),
            "b.ctst",
            "IMPORT \"a.ctst\"\nCOMPONENT mid FROM root {\n memory = \"1MiB\"\n}\n",
        );
        let path = write(
            dir.path(),
            "stack.ctst",
            "IMPORT \"b.ctst\"\nCOMPONENT leaf FROM mid {\n port = 1\n}\n",
        );
        let file = load_file(&path).expect("load");
        let leaf = component(&file, "leaf");
        assert_eq!(leaf.image.as_deref(), Some("file:///root"));
        assert_eq!(leaf.cpu.as_deref(), Some("512"));
        assert_eq!(leaf.memory.as_deref(), Some("1MiB"));
    }

    #[test]
    fn errors_are_explicit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cases = [
            (
                "COMPONENT a FROM b {\n image = \"file:///a\"\n}\nCOMPONENT b FROM a {\n image = \"file:///b\"\n}\n",
                "circular FROM",
            ),
            (
                "COMPONENT a FROM missing {\n}\n",
                "unknown template 'missing'",
            ),
            (
                "IMPORT \"https://example.com/t.ctst\" AS t\nCOMPONENT a FROM t {\n}\n",
                "remote IMPORT is not supported",
            ),
            ("IMPORT \"nope.ctst\"\n", "import file"),
            ("IMPORT \"stack.ctst\"\n", "circular IMPORT"),
        ];
        for (body, expected) in cases {
            let path = write(dir.path(), "stack.ctst", body);
            let error = load_file(&path).expect_err(expected).to_string();
            assert!(error.contains(expected), "{expected:?} not in {error:?}");
        }
    }

    #[test]
    fn duplicate_unaliased_template_names_are_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _ = write(
            dir.path(),
            "x.ctst",
            "COMPONENT t {\n image = \"file:///x\"\n}\n",
        );
        let _ = write(
            dir.path(),
            "y.ctst",
            "COMPONENT t {\n image = \"file:///y\"\n}\n",
        );
        let path = write(
            dir.path(),
            "stack.ctst",
            "IMPORT \"x.ctst\"\nIMPORT \"y.ctst\"\nCOMPONENT a FROM t {\n}\n",
        );
        let error = load_file(&path).expect_err("duplicate").to_string();
        assert!(error.contains("imported more than once"), "{error}");
    }
}
