// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(windows)]
use deckweave_platform::ChildProcessPolicy;
use deckweave_platform_host::NativeChildProcessPolicy;
use std::{
    error::Error,
    fs,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn plugin_cannot_execute_before_job_assignment() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("fixture.rs");
    let binary = directory.path().join("fixture.exe");
    let marker = directory.path().join("started");
    fs::write(
        &source,
        r#"fn main() { std::fs::write(std::env::args().nth(1).unwrap(), b"running").unwrap(); std::thread::sleep(std::time::Duration::from_secs(30)); }"#,
    )?;
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()?
            .success()
    );
    let mut command = Command::new(binary);
    command.arg(&marker);
    NativeChildProcessPolicy.configure(&mut command)?;
    let mut child = command.spawn()?;
    std::thread::sleep(Duration::from_millis(100));
    let prematurely_started = marker.exists();
    let guard = NativeChildProcessPolicy.attach(&mut child);
    if let Err(error) = guard {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }
    let guard = guard?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let resumed = marker.exists();
    let stopping = Instant::now();
    drop(guard);
    child.wait()?;
    assert!(
        !prematurely_started,
        "plugin code ran before its cleanup job was attached"
    );
    assert!(resumed, "plugin did not resume after job assignment");
    // KILL_ON_JOB_CLOSE does not specify a non-zero process exit code.
    assert!(
        stopping.elapsed() < Duration::from_secs(3),
        "child outlived its cleanup job"
    );
    Ok(())
}
