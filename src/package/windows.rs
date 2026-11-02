//! The Windows installer: `opentrack --package-msi <dir>` builds an `.msi`
//! that installs the running executable into
//! `C:\Program Files\Open Budget Tracker` and adds a Start menu shortcut.
//!
//! The installer is described in WiX source (.wxs) written here, and built
//! with the WiX Toolset's `wix` command, which must be on the PATH
//! (`dotnet tool install --global wix --version 6.0.2`). Neither the program
//! nor the installer is signed, since that needs a code signing certificate.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

const APP_NAME: &str = crate::APP_TITLE;
const MANUFACTURER: &str = "mediaswing";

/// Ties every version of the installer together, so installing a newer one
/// replaces the old one instead of installing beside it. Never change it.
const UPGRADE_CODE: &str = "286AA6A4-9817-4D22-B185-0A3B551D7111";

/// The WiX command. `OPENTRACK_WIX` can point at a `wix.exe` that isn't on the PATH.
fn wix() -> String {
    std::env::var("OPENTRACK_WIX").unwrap_or_else(|_| "wix".into())
}

/// WiX's name for the architecture this binary was built for.
fn wix_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "x86",
        other => other,
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The installer's WiX source, installing the executable at `exe`.
fn wxs(exe: &Path) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let exe_name = concat!(env!("CARGO_PKG_NAME"), ".exe");
    let source = xml_escape(&exe.display().to_string());
    let program_files = if wix_arch() == "x86" { "ProgramFilesFolder" } else { "ProgramFiles64Folder" };
    // The shortcut is advertised, so it needs no registry value of its own
    // (ICE43); an advertised shortcut takes its icon from the Icon table, and
    // that icon's id has to end in the target's extension (ICE50).
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="{APP_NAME}" Manufacturer="{MANUFACTURER}" Version="{version}"
           UpgradeCode="{UPGRADE_CODE}" Scope="perMachine" Compressed="yes">
    <MajorUpgrade DowngradeErrorMessage="A newer version of {APP_NAME} is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Icon Id="{exe_name}" SourceFile="{source}" />
    <Property Id="ARPPRODUCTICON" Value="{exe_name}" />
    <Property Id="ARPURLINFOABOUT" Value="https://github.com/mediaswing/opentrack" />
    <StandardDirectory Id="{program_files}">
      <Directory Id="INSTALLFOLDER" Name="{APP_NAME}">
        <Component>
          <File Id="MainExe" Source="{source}" Name="{exe_name}" KeyPath="yes">
            <Shortcut Id="StartMenuShortcut" Directory="ProgramMenuFolder" Name="{APP_NAME}"
                      Description="Track your spending by month and category"
                      WorkingDirectory="INSTALLFOLDER" Icon="{exe_name}" Advertise="yes" />
          </File>
        </Component>
      </Directory>
    </StandardDirectory>
  </Package>
</Wix>
"#
    )
}

/// Builds `opentrack-<version>-windows-<arch>.msi` in `out_dir` from the
/// running executable and returns its path.
pub fn build(out_dir: &Path) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("could not locate the running executable")?;
    std::fs::create_dir_all(out_dir)?;
    // wix writes a .wixpdb beside its output, so build in a work folder and
    // move only the .msi out.
    let work = out_dir.join("msi-work");
    if work.exists() {
        std::fs::remove_dir_all(&work)?;
    }
    std::fs::create_dir_all(&work)?;
    let source = work.join("opentrack.wxs");
    std::fs::write(&source, wxs(&exe))?;

    let name = format!(
        "{}-{}-windows-{}.msi",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH
    );
    let built = work.join(&name);
    let wix = wix();
    let status = Command::new(&wix)
        .args(["build", "-arch", wix_arch(), "-o"])
        .arg(&built)
        .arg(&source)
        .status()
        .with_context(|| {
            format!("could not run {wix}; install the WiX Toolset with `dotnet tool install --global wix --version 6.0.2`")
        })?;
    if !status.success() {
        bail!("{wix} build exited with {status}");
    }

    let msi = out_dir.join(&name);
    if msi.exists() {
        std::fs::remove_file(&msi)?;
    }
    std::fs::rename(&built, &msi).context("could not move the installer out of the work folder")?;
    std::fs::remove_dir_all(&work)?;
    Ok(msi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wxs_escapes_the_source_path() {
        let source = wxs(Path::new(r"C:\a & b\opentrack.exe"));
        assert!(source.contains(r#"Source="C:\a &amp; b\opentrack.exe""#));
        assert!(source.contains(&format!(r#"Version="{}""#, env!("CARGO_PKG_VERSION"))));
    }

    /// Packages the test binary when WiX is installed (CI installs it).
    #[test]
    fn builds_msi() {
        if Command::new(wix()).arg("--version").output().is_err() {
            eprintln!("WiX not found; skipping the .msi build");
            return;
        }
        let dir = std::env::temp_dir().join(format!("opentrack-msi-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let msi = build(&dir).unwrap();
        let bytes = std::fs::read(&msi).unwrap();
        // An .msi is an OLE compound file.
        assert!(bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
