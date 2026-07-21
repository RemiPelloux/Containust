//! Pinned preset download catalog.
//!
//! Digests are taken from Alpine's published `.sha256` sidecars on
//! `dl-cdn.alpinelinux.org`. Update URL and digest together when bumping.

/// A single curated download entry.
pub struct PresetEntry {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub arch: &'static str,
    pub default_latest: bool,
}

/// Official Alpine minirootfs archives used by `preset://alpine` and
/// `preset://busybox`.
pub const PRESETS: &[PresetEntry] = &[
    PresetEntry {
        name: "alpine",
        version: "3.22",
        description: "Alpine Linux 3.22 minirootfs (~3–4 MiB)",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/x86_64/alpine-minirootfs-3.22.5-x86_64.tar.gz",
        sha256: "4b4daa9fe2fc696c4919c4412a4c3d3e770d8fb70292a004a2c72f5096175282",
        arch: "x86_64",
        default_latest: true,
    },
    PresetEntry {
        name: "alpine",
        version: "3.22",
        description: "Alpine Linux 3.22 minirootfs (~3–4 MiB)",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/alpine-minirootfs-3.22.5-aarch64.tar.gz",
        sha256: "3fbc6285032ed46821b511292633d7b2a6306a2e254f590e92bdafff56cf2f70",
        arch: "aarch64",
        default_latest: true,
    },
    PresetEntry {
        name: "alpine",
        version: "3.21",
        description: "Alpine Linux 3.21 minirootfs (~3–4 MiB)",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-minirootfs-3.21.7-x86_64.tar.gz",
        sha256: "8cba1ea3e8b500ea986a313d8eecf3d5952a2a0d23a69117bb81c023d9ceac05",
        arch: "x86_64",
        default_latest: false,
    },
    PresetEntry {
        name: "alpine",
        version: "3.21",
        description: "Alpine Linux 3.21 minirootfs (~3–4 MiB)",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/aarch64/alpine-minirootfs-3.21.7-aarch64.tar.gz",
        sha256: "d1d1a3fae5f4d6146e9742790a47fcb116199622cfb8439f218a4d5fbe5000da",
        arch: "aarch64",
        default_latest: false,
    },
    PresetEntry {
        name: "busybox",
        version: "1.37",
        description: "BusyBox via Alpine 3.22 minirootfs (~3–4 MiB)",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/x86_64/alpine-minirootfs-3.22.5-x86_64.tar.gz",
        sha256: "4b4daa9fe2fc696c4919c4412a4c3d3e770d8fb70292a004a2c72f5096175282",
        arch: "x86_64",
        default_latest: true,
    },
    PresetEntry {
        name: "busybox",
        version: "1.37",
        description: "BusyBox via Alpine 3.22 minirootfs (~3–4 MiB)",
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.22/releases/aarch64/alpine-minirootfs-3.22.5-aarch64.tar.gz",
        sha256: "3fbc6285032ed46821b511292633d7b2a6306a2e254f590e92bdafff56cf2f70",
        arch: "aarch64",
        default_latest: true,
    },
];

/// Names users often expect from Docker Hub that need full OCI pull.
pub const UNSUPPORTED_PRESETS: &[(&str, &str)] = &[
    (
        "node",
        "preset://node is not curated yet (Docker Hub OCI pull is planned). \
         Use preset://alpine and install Node with apk, or import a local rootfs.",
    ),
    (
        "php",
        "preset://php is not curated yet (Docker Hub OCI pull is planned). \
         Use preset://alpine and install PHP with apk, or import a local rootfs.",
    ),
    (
        "python",
        "preset://python is not curated yet (Docker Hub OCI pull is planned). \
         Use preset://alpine and install Python with apk, or import a local rootfs.",
    ),
    (
        "nginx",
        "preset://nginx is not curated yet (Docker Hub OCI pull is planned). \
         Use preset://alpine and install nginx with apk, or import a local rootfs.",
    ),
    (
        "debian",
        "preset://debian is not curated yet. Prefer preset://alpine for a \
         minimal rootfs, or import a Debian rootfs with file:// / tar://.",
    ),
    (
        "ubuntu",
        "preset://ubuntu is not curated yet. Prefer preset://alpine for a \
         minimal rootfs, or import an Ubuntu rootfs with file:// / tar://.",
    ),
];
