//! The macOS installer package: `opentrack --package-macos <dir>` wraps the
//! running binary in an `.app` bundle, ad hoc signs it and builds an installer
//! package (.pkg) that puts the bundle in /Applications.
//!
//! Ad hoc signing ("-" identity) applies to code, so it is the bundle that is
//! signed. A .pkg can only be signed with a "Developer ID Installer"
//! certificate, so the installer wrapper itself is unsigned.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

const CODESIGN: &str = "/usr/bin/codesign";
const PKGBUILD: &str = "/usr/bin/pkgbuild";
const PRODUCTBUILD: &str = "/usr/bin/productbuild";

const APP_NAME: &str = crate::APP_TITLE;
const BUNDLE_ID: &str = "org.opentrack.OpenBudgetTracker";
const MIN_MACOS: &str = "11.0";
const COMPONENT_PKG: &str = "opentrack-app.pkg";

/// Builds `Open Budget Tracker.app` from the running executable, ad hoc signs
/// it, and writes an installer package for it to `out_dir`. Returns the
/// package's path. The bundle is left in `out_dir/root` for trying it out.
pub fn build(out_dir: &Path) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("could not locate the running executable")?;
    // pkgbuild installs everything in the root folder, so it holds only the bundle.
    let root = out_dir.join("root");
    if root.exists() {
        std::fs::remove_dir_all(&root).context("could not remove the previous bundle")?;
    }
    let app = root.join(format!("{APP_NAME}.app"));
    let contents = app.join("Contents");
    let macos_dir = contents.join("MacOS");
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(contents.join("Resources"))?;

    let bundled_exe = macos_dir.join(env!("CARGO_PKG_NAME"));
    std::fs::copy(&exe, &bundled_exe).context("could not copy the executable into the bundle")?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bundled_exe, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::write(contents.join("Info.plist"), info_plist())?;
    std::fs::write(contents.join("PkgInfo"), b"APPL????")?;

    // Ad hoc signature ("-" identity). Apple Silicon refuses to run unsigned
    // arm64 code, and the hardened runtime is enabled for good measure.
    run(Command::new(CODESIGN)
        .args(["--force", "--sign", "-", "--identifier", BUNDLE_ID])
        .args(["--timestamp=none", "--options", "runtime"])
        .arg(&app))
    .context("codesign could not sign the bundle")?;
    run(Command::new(CODESIGN).args(["--verify", "--strict", "--verbose=2"]).arg(&app))
        .context("the bundle's signature did not verify")?;

    // The component package installs the bundle; the product package around
    // it gives the installer its title and refuses Macs and macOS versions the
    // app can't run on.
    let work = out_dir.join("pkg-work");
    if work.exists() {
        std::fs::remove_dir_all(&work)?;
    }
    std::fs::create_dir_all(&work)?;
    let component_plist = work.join("component.plist");
    std::fs::write(&component_plist, component_plist_xml())?;
    let distribution = work.join("distribution.xml");
    std::fs::write(&distribution, distribution_xml())?;

    run(Command::new(PKGBUILD)
        .arg("--root")
        .arg(&root)
        .arg("--component-plist")
        .arg(&component_plist)
        .args(["--identifier", BUNDLE_ID, "--version", env!("CARGO_PKG_VERSION")])
        .args(["--install-location", "/Applications"])
        .arg(work.join(COMPONENT_PKG)))
    .context("pkgbuild could not build the component package")?;

    let pkg = out_dir.join(format!(
        "{}-{}-macos-{}.pkg",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH
    ));
    if pkg.exists() {
        std::fs::remove_file(&pkg)?;
    }
    run(Command::new(PRODUCTBUILD).arg("--distribution").arg(&distribution).arg("--package-path").arg(&work).arg(&pkg))
        .context("productbuild could not build the installer package")?;
    std::fs::remove_dir_all(&work)?;
    Ok(pkg)
}

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let status = cmd.status().with_context(|| format!("could not run {program}"))?;
    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

/// The installer's architecture check: the architecture this binary was built for.
fn host_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    }
}

/// Without this, pkgbuild marks the bundle relocatable, and the installer
/// would update a copy found anywhere else on the Mac instead of putting it in
/// /Applications.
fn component_plist_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
    <dict>
        <key>BundleHasStrictIdentifier</key><true/>
        <key>BundleIsRelocatable</key><false/>
        <key>BundleIsVersionChecked</key><false/>
        <key>BundleOverwriteAction</key><string>upgrade</string>
        <key>RootRelativeBundlePath</key><string>{APP_NAME}.app</string>
    </dict>
</array>
</plist>
"#
    )
}

fn distribution_xml() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let arch = host_architecture();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<installer-gui-script minSpecVersion="2">
    <title>{APP_NAME}</title>
    <options customize="never" require-scripts="false" hostArchitectures="{arch}"/>
    <domains enable_localSystem="true"/>
    <volume-check>
        <allowed-os-versions><os-version min="{MIN_MACOS}"/></allowed-os-versions>
    </volume-check>
    <choices-outline>
        <line choice="{BUNDLE_ID}"/>
    </choices-outline>
    <choice id="{BUNDLE_ID}" visible="false">
        <pkg-ref id="{BUNDLE_ID}"/>
    </choice>
    <pkg-ref id="{BUNDLE_ID}" version="{version}" onConclusion="none">{COMPONENT_PKG}</pkg-ref>
</installer-gui-script>
"#
    )
}

fn info_plist() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let exe = env!("CARGO_PKG_NAME");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>CFBundleDisplayName</key><string>{APP_NAME}</string>
    <key>CFBundleName</key><string>{APP_NAME}</string>
    <key>CFBundleExecutable</key><string>{exe}</string>
    <key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>{version}</string>
    <key>CFBundleVersion</key><string>{version}</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.finance</string>
    <key>LSMinimumSystemVersion</key><string>{MIN_MACOS}</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packages the test binary and checks the app inside the installer still
    /// carries a valid ad hoc signature.
    #[test]
    fn builds_signed_pkg() {
        let dir = std::env::temp_dir().join(format!("opentrack-pkg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pkg = build(&dir).unwrap();
        assert!(pkg.is_file());

        let expanded = dir.join("expanded");
        run(Command::new("/usr/sbin/pkgutil").arg("--expand-full").arg(&pkg).arg(&expanded)).unwrap();
        let app = expanded.join(COMPONENT_PKG).join("Payload").join(format!("{APP_NAME}.app"));
        run(Command::new(CODESIGN).args(["--verify", "--strict"]).arg(&app)).unwrap();
        let details = Command::new(CODESIGN).args(["--display", "--verbose=2"]).arg(&app).output().unwrap();
        assert!(String::from_utf8_lossy(&details.stderr).contains("Signature=adhoc"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
