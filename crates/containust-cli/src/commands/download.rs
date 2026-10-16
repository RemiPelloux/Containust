//! `ctst download` — Prefetch every resource needed to run air-gapped.
//!
//! Run once on a connected machine: VM boot assets, composition images,
//! curated presets, and extra registry images are downloaded, verified
//! by SHA-256, and stored in the local caches. Afterwards
//! `ctst --offline run` works without network access, on this host or
//! on an on-prem host the caches are copied to.

use std::path::Path;

use clap::Args;
use containust_image::import::{ImportRequest, import_image};
use containust_image::preset::{ImagePreset, list_presets};
use containust_image::reference::{ImageReference, ImageScheme};
use containust_runtime::backend::platform_info;

use super::build::{BuildContext, build_component};
use super::pull::{default_catalog_name, normalize_oci_uri};

const DEFAULT_FILE: &str = "containust.ctst";

/// Policy for prefetching the QEMU VM kernel and initramfs.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmAssets {
    /// Download only when the native Linux backend is unavailable.
    Auto,
    /// Always download (e.g. preparing assets for a macOS/Windows host).
    Always,
    /// Never download.
    Never,
}

/// Arguments for the `download` command.
#[derive(Args, Debug)]
pub struct DownloadArgs {
    /// Path to the .ctst composition whose images are prefetched.
    ///
    /// The default file is optional: when it does not exist, only VM
    /// assets, presets, and `--image` references are downloaded.
    #[arg(default_value = DEFAULT_FILE)]
    pub file: String,

    /// Extra registry image to pull (`name[:tag]` or `oci://…`); repeatable.
    #[arg(long = "image", value_name = "IMAGE")]
    pub images: Vec<String>,

    /// Download every curated `preset://` image for this architecture.
    #[arg(long)]
    pub all_presets: bool,

    /// When to download VM boot assets (`auto`: only if the native backend is missing).
    #[arg(long, value_enum, default_value_t = VmAssets::Auto)]
    pub vm: VmAssets,

    /// List what would be downloaded without touching the network.
    #[arg(long)]
    pub dry_run: bool,
}

/// Executes the `download` command.
///
/// # Errors
///
/// Returns an error when offline mode is active, the composition cannot
/// be parsed, or any download or import fails.
pub fn execute(args: DownloadArgs, options: &super::RuntimeOptions) -> anyhow::Result<()> {
    if options.offline {
        anyhow::bail!(
            "ctst download needs network access; offline mode is enabled \
             (drop --offline / unset CONTAINUST_OFFLINE)"
        );
    }
    let composition = load_composition(&args.file)?;
    let engine = options.engine_for_project(Path::new(&args.file));
    let data_dir = engine.data_dir();
    let mut summary = Summary::default();

    if wants_vm_assets(&args) {
        download_vm_assets(args.dry_run)?;
        summary.vm = true;
    }

    if let Some(composition) = &composition {
        println!("Composition images ({}):", args.file);
        let context = BuildContext {
            data_dir,
            offline: false,
            dry_run: args.dry_run,
        };
        for component in &composition.components {
            let Some(image) = component.image.as_deref() else {
                continue;
            };
            let reference = ImageReference::parse(image).map_err(|e| anyhow::anyhow!("{e}"))?;
            let _imported = build_component(&context, &component.name, &reference)?;
            summary.images += 1;
        }
    }

    if args.all_presets {
        println!("Curated presets:");
        for preset in list_presets() {
            download_preset(data_dir, &preset, args.dry_run)?;
            summary.images += 1;
        }
    }

    if !args.images.is_empty() {
        println!("Registry images:");
        for image in &args.images {
            pull_image(data_dir, image, args.dry_run)?;
            summary.images += 1;
        }
    }

    summary.print(data_dir, args.dry_run);
    if let Some(composition) = &composition {
        report_offline_readiness(composition, args.dry_run);
    }
    Ok(())
}

/// Parses the composition, treating a missing default file as "none".
fn load_composition(
    file: &str,
) -> anyhow::Result<Option<containust_compose::parser::ast::CompositionFile>> {
    let path = Path::new(file);
    if !path.exists() && file == DEFAULT_FILE {
        return Ok(None);
    }
    containust_compose::template::load_file(path)
        .map(Some)
        .map_err(|e| anyhow::anyhow!("cannot read composition {file}: {e}"))
}

/// VM assets are only needed where the native Linux backend is missing,
/// unless the caller forces or skips them.
fn wants_vm_assets(args: &DownloadArgs) -> bool {
    match args.vm {
        VmAssets::Auto => !platform_info().native_available,
        VmAssets::Always => true,
        VmAssets::Never => false,
    }
}

fn download_vm_assets(dry_run: bool) -> anyhow::Result<()> {
    let arch = containust_runtime::backend::vm::assets::host_arch();
    let entry = containust_runtime::backend::vm::assets::asset_for_arch(arch)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("VM boot assets (Alpine {}, {arch}):", entry.alpine_release);
    if dry_run {
        println!("  Would download {}", entry.kernel_url);
        println!("  Would download {}", entry.initramfs_url);
        return Ok(());
    }
    let vm_dir = containust_runtime::backend::vm::prefetch_boot_assets()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("  Kernel and initramfs verified in {}", vm_dir.display());
    Ok(())
}

/// Imports one preset under a stable `<name>-<version>` catalog name.
fn download_preset(data_dir: &Path, preset: &ImagePreset, dry_run: bool) -> anyhow::Result<()> {
    let catalog_name = preset_catalog_name(preset);
    println!(
        "  preset://{}:{} -> {catalog_name}",
        preset.name, preset.version
    );
    if dry_run {
        println!(
            "    Would download {} → sha256:{}",
            preset.url, preset.sha256
        );
        return Ok(());
    }
    let uri = format!("preset://{}:{}", preset.name, preset.version);
    let reference = ImageReference::parse(&uri).map_err(|e| anyhow::anyhow!("{e}"))?;
    let entry = import_image(
        data_dir,
        &reference,
        &ImportRequest::new(&catalog_name, false),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!(
        "    Cached as image://{catalog_name}@sha256:{}",
        entry.digest.as_deref().unwrap_or_default()
    );
    Ok(())
}

fn preset_catalog_name(preset: &ImagePreset) -> String {
    format!("{}-{}", preset.name, preset.version)
}

/// Pulls one registry image exactly like `ctst pull` would.
fn pull_image(data_dir: &Path, image: &str, dry_run: bool) -> anyhow::Result<()> {
    let uri = normalize_oci_uri(image);
    let reference = ImageReference::parse(&uri).map_err(|e| anyhow::anyhow!("{e}"))?;
    if reference.scheme() != ImageScheme::Oci {
        anyhow::bail!("--image only accepts registry images (got {uri})");
    }
    let catalog_name = default_catalog_name(reference.location());
    println!("  {reference} -> {catalog_name}");
    if dry_run {
        println!("    Would pull from the registry");
        return Ok(());
    }
    let request = ImportRequest::new(&catalog_name, false).with_unpinned();
    let entry = import_image(data_dir, &reference, &request).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!(
        "    Pinned reference: image://{catalog_name}@sha256:{}",
        entry.digest.as_deref().unwrap_or_default()
    );
    Ok(())
}

#[derive(Debug, Default)]
struct Summary {
    vm: bool,
    images: usize,
}

impl Summary {
    fn print(&self, data_dir: &Path, dry_run: bool) {
        println!();
        if dry_run {
            println!("Dry run complete. Nothing was downloaded.");
            return;
        }
        println!("Download complete. {} image(s) cached.", self.images);
        println!("  Image store: {}", data_dir.join("images").display());
        if self.vm {
            println!(
                "  VM assets:   {}",
                containust_runtime::backend::vm::vm_cache_dir().display()
            );
        }
        println!("Copy these directories to the on-prem host to run with `ctst --offline`.");
    }
}

/// Tells the user whether `ctst --offline` will accept the composition.
fn report_offline_readiness(
    composition: &containust_compose::parser::ast::CompositionFile,
    dry_run: bool,
) {
    match containust_compose::validate_offline(composition) {
        Ok(()) if dry_run => {
            println!("Offline ready once downloaded: run again without --dry-run.");
        }
        Ok(()) => println!("Offline ready: `ctst --offline run` will not touch the network."),
        Err(error) => {
            println!("Not offline ready: {error}");
            println!(
                "  Replace remote sources with the printed image:// references \
                 so --offline can use the cache."
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn args(file: &str) -> DownloadArgs {
        DownloadArgs {
            file: file.into(),
            images: Vec::new(),
            all_presets: false,
            vm: VmAssets::Never,
            dry_run: true,
        }
    }

    #[test]
    fn download_rejects_offline_mode() {
        let options = super::super::RuntimeOptions {
            offline: true,
            state_file: None,
        };
        let error = execute(args(DEFAULT_FILE), &options).expect_err("offline must fail");
        assert!(error.to_string().contains("network access"));
    }

    #[test]
    fn missing_explicit_file_fails() {
        let error = load_composition("/nonexistent/app.ctst").expect_err("must fail");
        assert!(error.to_string().contains("cannot read composition"));
    }

    #[test]
    fn vm_policy_selects_assets() {
        let mut download = args(DEFAULT_FILE);
        assert!(!wants_vm_assets(&download));
        download.vm = VmAssets::Always;
        assert!(wants_vm_assets(&download));
    }

    #[test]
    fn preset_catalog_name_includes_version() {
        let preset = list_presets()
            .into_iter()
            .next()
            .expect("at least one preset");
        assert_eq!(
            preset_catalog_name(&preset),
            format!("{}-{}", preset.name, preset.version)
        );
    }

    #[test]
    fn image_flag_rejects_non_registry_sources() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = pull_image(dir.path(), "file:///rootfs", true).expect_err("must fail");
        assert!(error.to_string().contains("registry images"));
    }

    #[test]
    fn dry_run_composition_lists_images_without_network() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("app.ctst");
        std::fs::write(
            &file,
            "COMPONENT web {\n    image = \"preset://alpine\"\n}\n",
        )
        .expect("write ctst");
        let mut download = args(file.to_str().expect("utf8 path"));
        download.all_presets = true;
        download.images = vec!["alpine:3.21".into()];
        execute(download, &super::super::RuntimeOptions::default()).expect("dry run succeeds");
        assert!(!dir.path().join(".containust/images/catalog.json").exists());
    }
}
