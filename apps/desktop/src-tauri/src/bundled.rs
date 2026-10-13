// SPDX-License-Identifier: GPL-3.0-or-later
//! Install first-party providers once; existing package bytes remain pinned.
use deckweave_plugin_supervisor::package::{PackageStager, StagingLimits};
use std::{error::Error, fs, path::Path};
#[cfg(test)]
mod tests;

pub(crate) fn ensure_bundled(root: &Path) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let bundled = executable
        .parent()
        .ok_or("executable directory")?
        .join("bundled");
    install(
        root,
        &bundled.join("virtual-device"),
        "deckweave-virtual",
        "org.deckweave.virtual",
    )?;
    #[cfg(windows)]
    for name in ["audio", "media", "window", "applications", "keyboard"] {
        install(
            root,
            &bundled.join(format!("windows-{name}")),
            &format!("windows-{name}"),
            &format!("org.deckweave.platform.windows.{name}"),
        )?;
    }
    Ok(())
}

fn install(
    root: &Path,
    source: &Path,
    folder: &str,
    package_id: &str,
) -> Result<(), Box<dyn Error>> {
    // Cargo-only builds may intentionally omit desktop assets; standard npm dev/build prepares them.
    if !source.exists() {
        return Ok(());
    }
    let staged = PackageStager::inspect(source, &StagingLimits::default())?;
    if staged.package.package_id.as_str() != package_id {
        return Err("bundled package identity mismatch".into());
    }
    let destination = root.join(format!(
        "packages/{folder}/{}/content",
        staged.package.version
    ));
    if destination.exists() {
        let installed = PackageStager::inspect(&destination, &StagingLimits::default())?;
        if installed.package.package_id.as_str() != package_id
            || installed.package.version != staged.package.version
        {
            return Err("bundled package identity mismatch".into());
        }
        // Same-version development rebuilds must never overwrite pinned bytes or grants.
        return Ok(());
    }
    fs::create_dir_all(root)?;
    let staging = tempfile::Builder::new()
        .prefix(&format!(".{folder}-staging-"))
        .tempdir_in(root)?;
    let temporary = staging.path().join("content");
    copy_tree(source, &temporary)?;
    let copied = PackageStager::inspect(&temporary, &StagingLimits::default())?;
    if copied.package.manifest_sha256 != staged.package.manifest_sha256 {
        return Err("bundled package changed during staging".into());
    }
    fs::create_dir_all(destination.parent().ok_or("package parent")?)?;
    fs::rename(temporary, destination)?;
    Ok(())
}
fn copy_tree(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("bundle symlink".into());
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err("unsupported bundle file".into());
        }
    }
    Ok(())
}
