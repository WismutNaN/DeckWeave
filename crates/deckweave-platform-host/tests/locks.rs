// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_platform_host::FileLease;
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn child_lock_holder() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::var_os("DECKWEAVE_TEST_LOCK") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    let _lease = FileLease::acquire(&path)?;
    std::fs::write(path.with_extension("ready"), b"ready")?;
    std::thread::sleep(Duration::from_secs(30));
    Ok(())
}

#[test]
fn crash_releases_writer_lock() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let lock = temp.path().join("writer.lock");
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--exact", "child_lock_holder", "--nocapture"])
        .env("DECKWEAVE_TEST_LOCK", &lock)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lock.with_extension("ready").exists() {
        if Instant::now() > deadline {
            child.kill()?;
            child.wait()?;
            return Err("Child не получил lock".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let rejected = FileLease::acquire(&lock).is_err();
    child.kill()?;
    child.wait()?;
    assert!(rejected, "Одновременный writer был допущен");
    let _after_crash = FileLease::acquire(&lock)?;
    Ok(())
}
