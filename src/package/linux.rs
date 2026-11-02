//! The Debian and Ubuntu package: `opentrack --package-deb <dir>` writes a
//! `.deb` containing the running binary, assembled in Rust (ar + tar + xz) so
//! that releasing needs no shell script or extra packaging tools. It is not
//! signed: Ubuntu doesn't check signatures on a .deb installed from a file.

use std::path::{Path, PathBuf};

use anyhow::Context;

/// zlib is linked (MariaDB/MySQL compression). The GUI loads OpenGL and
/// xkbcommon with dlopen, so they don't show up as ELF dependencies. libc6's
/// minimum version is read from the binary.
const DEB_DEPENDS: &str = "libgcc-s1, zlib1g, libgl1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0";
const DEB_RECOMMENDS: &str = "libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrandr2";

const DESKTOP_ENTRY: &str = "[Desktop Entry]
Type=Application
Name=Open Budget Tracker
GenericName=Budget tracker
Comment=Track your spending by month and category
Exec=opentrack
Icon=accessories-calculator
Terminal=false
Categories=Office;Finance;
Keywords=budget;money;spending;finance;expenses;
StartupWMClass=opentrack
";

fn deb_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "i386",
        "arm" => "armhf",
        other => other,
    }
}

/// One file to place in the package: (path inside the package, mode, bytes).
type DebFile = (&'static str, u32, Vec<u8>);

/// xz rather than gzip makes the package much smaller. dpkg has read xz since 1.17.
fn tar_xz(dirs: &[&str], files: &[DebFile], mtime: u64) -> anyhow::Result<Vec<u8>> {
    let xz = xz2::write::XzEncoder::new(Vec::new(), 9);
    let mut tar = tar::Builder::new(xz);
    for dir in dirs {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(0o755);
        h.set_uid(0);
        h.set_gid(0);
        h.set_mtime(mtime);
        h.set_size(0);
        tar.append_data(&mut h, dir, std::io::empty())?;
    }
    for (path, mode, data) in files {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Regular);
        h.set_mode(*mode);
        h.set_uid(0);
        h.set_gid(0);
        h.set_mtime(mtime);
        h.set_size(data.len() as u64);
        tar.append_data(&mut h, path, data.as_slice())?;
    }
    Ok(tar.into_inner()?.finish()?)
}

fn ar_member(out: &mut Vec<u8>, name: &str, data: &[u8], mtime: u64) {
    out.extend_from_slice(
        format!("{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", name, mtime, 0, 0, "100644", data.len()).as_bytes(),
    );
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(b'\n');
    }
}

/// libc6 at the newest `GLIBC_x.y` symbol version the binary references.
fn libc_depends(binary: &[u8]) -> String {
    let needle = b"GLIBC_";
    let mut newest: Option<Vec<u32>> = None;
    for start in binary.windows(needle.len()).enumerate().filter(|(_, w)| *w == needle).map(|(i, _)| i + needle.len()) {
        let end = binary[start..]
            .iter()
            .position(|b| !(b.is_ascii_digit() || *b == b'.'))
            .map_or(binary.len(), |n| start + n);
        let version: Option<Vec<u32>> =
            std::str::from_utf8(&binary[start..end]).ok().and_then(|v| v.split('.').map(|n| n.parse().ok()).collect());
        if let Some(v) = version.filter(|v| v.len() >= 2)
            && newest.as_ref().is_none_or(|n| v > *n)
        {
            newest = Some(v);
        }
    }
    match newest {
        Some(v) => format!("libc6 (>= {})", v.iter().map(u32::to_string).collect::<Vec<_>>().join(".")),
        None => "libc6".into(),
    }
}

/// Builds `opentrack_<version>_<arch>.deb` in `out_dir` from the running binary.
pub fn build(out_dir: &Path) -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("could not locate the running executable")?;
    let binary = std::fs::read(&exe)?;
    let version = env!("CARGO_PKG_VERSION");
    let arch = deb_arch();
    let mtime = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    });
    let libc = libc_depends(&binary);

    let files: Vec<DebFile> = vec![
        ("./usr/bin/opentrack", 0o755, binary),
        ("./usr/share/applications/opentrack.desktop", 0o644, DESKTOP_ENTRY.as_bytes().to_vec()),
        ("./usr/share/doc/opentrack/README.md", 0o644, include_bytes!("../../README.md").to_vec()),
    ];
    let dirs = [
        "./",
        "./usr/",
        "./usr/bin/",
        "./usr/share/",
        "./usr/share/applications/",
        "./usr/share/doc/",
        "./usr/share/doc/opentrack/",
    ];
    let installed_kib = files.iter().map(|f| f.2.len() as u64).sum::<u64>().div_ceil(1024);

    let control = format!(
        "Package: opentrack\n\
         Version: {version}\n\
         Architecture: {arch}\n\
         Maintainer: wrichards <mediaswing@outlook.com>\n\
         Installed-Size: {installed_kib}\n\
         Depends: {libc}, {DEB_DEPENDS}\n\
         Recommends: {DEB_RECOMMENDS}\n\
         Section: utils\n\
         Priority: optional\n\
         Description: Open Budget Tracker, a personal budget tracker\n \
         Records spending, groups it into categories and shows where the money\n \
         goes month by month. Keeps its data in a built-in SQLite database or a\n \
         MariaDB/MySQL server.\n"
    );

    let debian_binary = b"2.0\n".to_vec();
    let control_tar = tar_xz(&["./"], &[("./control", 0o644, control.into_bytes())], mtime)?;
    let data_tar = tar_xz(&dirs, &files, mtime)?;

    let mut deb = b"!<arch>\n".to_vec();
    ar_member(&mut deb, "debian-binary", &debian_binary, mtime);
    ar_member(&mut deb, "control.tar.xz", &control_tar, mtime);
    ar_member(&mut deb, "data.tar.xz", &data_tar, mtime);

    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join(format!("opentrack_{version}_{arch}.deb"));
    std::fs::write(&path, deb)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// The ar members of a .deb, in order, as (name, contents).
    fn ar_members(deb: &[u8]) -> anyhow::Result<Vec<(String, &[u8])>> {
        anyhow::ensure!(deb.starts_with(b"!<arch>\n"), "not an ar archive");
        let mut members = Vec::new();
        let mut pos = 8;
        while pos + 60 <= deb.len() {
            let header = &deb[pos..pos + 60];
            let name = String::from_utf8_lossy(&header[..16]).trim_end().trim_end_matches('/').to_string();
            let size: usize = String::from_utf8_lossy(&header[48..58]).trim().parse().context("bad ar member size")?;
            let start = pos + 60;
            anyhow::ensure!(start + size <= deb.len(), "truncated ar member {name}");
            members.push((name, &deb[start..start + size]));
            pos = start + size + size % 2;
        }
        Ok(members)
    }

    /// Packages the test binary and, where dpkg is installed, checks that dpkg
    /// accepts the result.
    #[test]
    fn builds_valid_deb() {
        let dir = std::env::temp_dir().join(format!("opentrack-deb-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let deb = build(&dir).unwrap();
        let bytes = std::fs::read(&deb).unwrap();
        assert!(bytes.starts_with(b"!<arch>\ndebian-binary   "));
        let names: Vec<String> = ar_members(&bytes).unwrap().into_iter().map(|m| m.0).collect();
        assert_eq!(names, ["debian-binary", "control.tar.xz", "data.tar.xz"]);

        let dpkg = Path::new("/usr/bin/dpkg-deb");
        if dpkg.exists() {
            let info = Command::new(dpkg).arg("--info").arg(&deb).output().unwrap();
            assert!(info.status.success(), "{}", String::from_utf8_lossy(&info.stderr));
            let contents = Command::new(dpkg).arg("--contents").arg(&deb).output().unwrap();
            assert!(String::from_utf8_lossy(&contents.stdout).contains("usr/bin/opentrack"));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_newest_glibc() {
        assert_eq!(libc_depends(b"\0GLIBC_2.17\0GLIBC_2.34\0GLIBC_2.3.4\0"), "libc6 (>= 2.34)");
        assert_eq!(libc_depends(b"nothing here"), "libc6");
    }
}
