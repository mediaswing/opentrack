//! The Debian package: `opentrack --package-deb <dir>` puts the running
//! binary, a desktop entry and the icon in a package (.deb) for the release
//! page, for Ubuntu and other Debian-based systems. Keeping it here means the
//! release workflow needs no shell script, as with the macOS package.

use anyhow::{Context, bail};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const DPKG_DEB: &str = "dpkg-deb";

const PACKAGE: &str = "opentrack";
const MAINTAINER: &str = "The Open Budget Tracker contributors <https://github.com/mediaswing/opentrack/issues>";

/// The libraries the app needs that aren't found by its imports: winit,
/// glutin and xkbcommon load these when the window opens, for Wayland or X11.
const LOADED_LIBRARIES: &[&str] = &[
    "libegl1",
    "libgl1",
    "libwayland-client0",
    "libwayland-egl1",
    "libx11-6",
    "libx11-xcb1",
    "libxcursor1",
    "libxi6",
    "libxkbcommon0",
    "libxkbcommon-x11-0",
    "libxrandr2",
];

/// The file dialogs ask the desktop through its portal, and the message boxes
/// (and the file dialogs, without a portal) use zenity.
const RECOMMENDS: &str = "xdg-desktop-portal, zenity";

/// Builds a package from the running executable and writes it to `out_dir`.
/// Returns the package's path. The files it installs are left in
/// `out_dir/deb-root` for looking at.
pub fn build(out_dir: &Path) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("could not locate the running executable")?;
    let binary = std::fs::read(&exe).context("could not read the running executable")?;
    let root = out_dir.join("deb-root");
    if root.exists() {
        std::fs::remove_dir_all(&root).context("could not remove the previous package files")?;
    }

    // The window's app id ("opentrack", set in main.rs) must match the desktop
    // entry's name, or Wayland desktops show the app without its icon.
    put(&root.join("usr/bin/opentrack"), &binary, 0o755)?;
    put(&root.join("usr/share/applications/opentrack.desktop"), desktop_entry().as_bytes(), 0o644)?;
    put(
        &root.join("usr/share/icons/hicolor/256x256/apps/opentrack.png"),
        include_bytes!("../../assets/icon/opentrack.png"),
        0o644,
    )?;
    put(&root.join("usr/share/doc/opentrack/copyright"), copyright().as_bytes(), 0o644)?;

    // Installed-Size is in KiB, and counts the files the package installs.
    let installed_kib = size_of(&root)?.div_ceil(1024);
    put(&root.join("DEBIAN/control"), control(installed_kib, &binary)?.as_bytes(), 0o644)?;

    let deb = out_dir.join(format!(
        "opentrack-{}-linux-{}.deb",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH
    ));
    if deb.exists() {
        std::fs::remove_file(&deb)?;
    }
    // xz rather than dpkg's default, which on Ubuntu is zstd: older Debian
    // releases can't read zstd packages. Files are owned by root whoever
    // builds the package.
    let status = Command::new(DPKG_DEB)
        .args(["--root-owner-group", "-Zxz", "--build"])
        .arg(&root)
        .arg(&deb)
        .status()
        .context("could not run dpkg-deb")?;
    if !status.success() {
        bail!("dpkg-deb could not build the package");
    }
    Ok(deb)
}

/// Writes a file with the given permissions, making its folders (0755) first.
/// The permissions are set explicitly so the builder's umask can't change
/// what the package installs.
fn put(path: &Path, contents: &[u8], mode: u32) -> anyhow::Result<()> {
    let dir = path.parent().expect("package files are in a folder");
    let mut missing = Vec::new();
    let mut d = dir;
    while !d.exists() {
        missing.push(d.to_path_buf());
        d = d.parent().expect("the package root's parent exists or is created");
    }
    std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    for d in missing {
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::write(path, contents).with_context(|| format!("could not write {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

fn size_of(dir: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        total += if meta.is_dir() { size_of(&entry.path())? } else { meta.len() };
    }
    Ok(total)
}

fn control(installed_kib: u64, binary: &[u8]) -> anyhow::Result<String> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => bail!("no Debian architecture is known for {other}"),
    };
    let mut depends = vec![match glibc_needed(binary) {
        Some(version) => format!("libc6 (>= {version})"),
        None => "libc6".to_string(),
    }];
    // Rust's standard library unwinds panics with libgcc_s.
    depends.push("libgcc-s1".to_string());
    depends.extend(LOADED_LIBRARIES.iter().map(|l| l.to_string()));
    Ok(format!(
        "Package: {PACKAGE}
Version: {version}
Architecture: {arch}
Maintainer: {MAINTAINER}
Installed-Size: {installed_kib}
Depends: {depends}
Recommends: {RECOMMENDS}
Section: misc
Priority: optional
Homepage: {homepage}
Description: {description}
 Open Budget Tracker keeps track of what you spend, sorts it into categories,
 and shows it against a monthly budget for each one. Every feature works
 from the keyboard, and the whole interface is exposed to screen readers.
",
        version = debian_version(env!("CARGO_PKG_VERSION")),
        depends = depends.join(", "),
        homepage = env!("CARGO_PKG_REPOSITORY"),
        description = env!("CARGO_PKG_DESCRIPTION"),
    ))
}

/// A Cargo version as a Debian one. A pre-release such as 1.3.0-beta.1 must
/// sort before 1.3.0, which in Debian versions takes a tilde.
fn debian_version(cargo: &str) -> String {
    cargo.replacen('-', "~", 1)
}

/// The newest glibc version the binary asks for (from the version names, such
/// as GLIBC_2.34, that its imports carry), which is the oldest glibc it runs
/// with.
fn glibc_needed(binary: &[u8]) -> Option<String> {
    const PREFIX: &[u8] = b"GLIBC_";
    let mut newest: Option<Vec<u32>> = None;
    let mut rest = binary;
    while let Some(at) = rest.windows(PREFIX.len()).position(|w| w == PREFIX) {
        rest = &rest[at + PREFIX.len()..];
        let len = rest.iter().take_while(|b| b.is_ascii_digit() || **b == b'.').count();
        let text = std::str::from_utf8(&rest[..len]).unwrap_or_default();
        let parts: Option<Vec<u32>> = text.split('.').map(|p| p.parse().ok()).collect();
        if let Some(parts) = parts.filter(|p| p.len() >= 2)
            && newest.as_ref().is_none_or(|n| parts > *n)
        {
            newest = Some(parts);
        }
    }
    newest.map(|v| v.iter().map(u32::to_string).collect::<Vec<_>>().join("."))
}

fn desktop_entry() -> String {
    let name = crate::APP_TITLE;
    format!(
        "[Desktop Entry]
Type=Application
Name={name}
GenericName=Budget Tracker
Comment=Keep track of spending against a monthly budget
Exec=opentrack
Icon=opentrack
Terminal=false
Categories=Office;Finance;
Keywords=budget;money;spending;
StartupWMClass=opentrack
"
    )
}

/// The machine-readable copyright file Debian packages carry. The embedded
/// fonts' licence must travel with them, so it is copied in whole.
fn copyright() -> String {
    let ofl: String = include_str!("../../assets/fonts/OFL.txt")
        .lines()
        .map(|l| if l.trim().is_empty() { " .\n".to_string() } else { format!(" {l}\n") })
        .collect();
    format!(
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: {name}
Source: {source}

Files: *
Copyright: The Open Budget Tracker contributors
License: GPL-3+
 This program is free software: you can redistribute it and/or modify it
 under the terms of the GNU General Public License as published by the Free
 Software Foundation, either version 3 of the License, or (at your option)
 any later version.
 .
 On Debian systems, the full text of the GNU General Public License version 3
 can be found in /usr/share/common-licenses/GPL-3.

Files: assets/fonts/*
Copyright: 2025 The Google Sans Project Authors (github.com/googlefonts/googlesans)
License: OFL-1.1
{ofl}",
        name = crate::APP_TITLE,
        source = env!("CARGO_PKG_REPOSITORY"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_newest_glibc_version() {
        let binary = b"\0GLIBC_2.2.5\0GLIBC_2.34\0GLIBC_PRIVATE\0GLIBC_2.17\0GLIBC_2.3.4\0";
        assert_eq!(glibc_needed(binary).as_deref(), Some("2.34"));
        assert_eq!(glibc_needed(b"no versions here"), None);
    }

    #[test]
    fn pre_releases_sort_before_releases() {
        assert_eq!(debian_version("1.2.0"), "1.2.0");
        assert_eq!(debian_version("1.3.0-beta.1"), "1.3.0~beta.1");
    }
}
