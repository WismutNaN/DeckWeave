// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(windows)]
use deckweave_platform::{
    ExactLaunch, UserProcessFailure, UserProcessLauncher, UserProcessRequest,
};
use deckweave_platform_host::NativeUserProcessLauncher;
use std::{
    error::Error,
    fs,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn exact_unicode_executable_argv_and_cwd_round_trip() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let directory = temp.path().join("каталог с пробелами");
    fs::create_dir(&directory)?;
    let source = directory.join("fixture.rs");
    fs::write(
        &source,
        r#"fn main() {
        let args:Vec<_> = std::env::args().skip(1).collect();
        let root = std::path::Path::new(&args[0]);
        for (i,arg) in args.iter().skip(1).enumerate() { std::fs::write(root.join(i.to_string()),arg).unwrap(); }
        std::fs::write(root.join("cwd"),std::env::current_dir().unwrap().to_str().unwrap()).unwrap();
        std::fs::write(root.join("done"),"").unwrap();
    }"#,
    )?;
    let binary = directory.join("проверка аргументов.exe");
    assert!(
        Command::new("rustc")
            .args(["--crate-name", "broker_fixture"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()?
            .success()
    );
    let values = [
        "with spaces",
        "цитата \"hello\"",
        "trailing\\",
        "",
        "& whoami",
        "line\nsecond",
    ];
    let arguments = std::iter::once(directory.to_string_lossy().into_owned())
        .chain(values.iter().map(|v| (*v).into()))
        .collect();
    let receipt = NativeUserProcessLauncher
        .execute(
            &UserProcessRequest::Launch(ExactLaunch {
                executable: binary.to_string_lossy().into_owned(),
                arguments,
                working_directory: Some(directory.to_string_lossy().into_owned()),
            }),
            Instant::now() + Duration::from_secs(3),
        )
        .map_err(|e| format!("{e:?}"))?;
    assert!(receipt.process_id.is_some());
    let deadline = Instant::now() + Duration::from_secs(3);
    while !directory.join("done").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    for (i, expected) in values.iter().enumerate() {
        assert_eq!(
            fs::read_to_string(directory.join(i.to_string()))?,
            *expected
        );
    }
    assert_eq!(
        fs::canonicalize(fs::read_to_string(directory.join("cwd"))?)?,
        fs::canonicalize(&directory)?
    );
    std::thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[test]
fn expired_relative_or_batch_launch_is_rejected_before_creation() {
    for (path, deadline, expected) in [
        (
            "relative.exe",
            Instant::now() + Duration::from_secs(1),
            UserProcessFailure::InvalidInput,
        ),
        (
            "C:\\script.cmd",
            Instant::now() + Duration::from_secs(1),
            UserProcessFailure::InvalidInput,
        ),
        ("C:\\test.exe", Instant::now(), UserProcessFailure::Deadline),
    ] {
        assert_eq!(
            NativeUserProcessLauncher.execute(
                &UserProcessRequest::Launch(ExactLaunch {
                    executable: path.into(),
                    arguments: vec![],
                    working_directory: None
                }),
                deadline
            ),
            Err(expected)
        );
    }
}
