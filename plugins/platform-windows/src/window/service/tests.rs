// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[derive(Default)]
struct FakeBackend {
    windows: Vec<NativeWindow>,
    foreground: Option<u64>,
    deny_activation: bool,
}

impl WindowBackend for FakeBackend {
    fn list_windows(&mut self) -> Result<Vec<NativeWindow>, WindowFailure> {
        Ok(self.windows.clone())
    }

    fn foreground_token(&mut self) -> Result<Option<u64>, WindowFailure> {
        Ok(self.foreground)
    }

    fn activate(&mut self, token: u64, _timeout: Duration) -> Result<(), WindowFailure> {
        if self.deny_activation {
            return Err(WindowFailure::ForegroundDenied);
        }
        self.foreground = Some(token);
        Ok(())
    }
}

fn record(token: u64, pid: u32, executable: &str, title: &str) -> NativeWindow {
    NativeWindow {
        token,
        process_id: pid,
        process_started_at: Some(u64::from(pid) * 10),
        executable: Some(executable.into()),
        title: title.into(),
    }
}

#[test]
fn two_windows_of_one_executable_are_distinct_and_selector_prefers_foreground()
-> Result<(), WindowFailure> {
    let backend = FakeBackend {
        windows: vec![
            record(11, 5, "C:\\App\\app.exe", "One"),
            record(12, 5, "C:\\App\\app.exe", "Two"),
        ],
        foreground: Some(12),
        deny_activation: false,
    };
    let mut service = WindowService::new(backend, 7);
    let catalog = service.catalog()?;
    assert_eq!(catalog.windows.len(), 2);
    assert_ne!(catalog.windows[0].window_ref, catalog.windows[1].window_ref);
    let state = service
        .focus_selector(
            &WindowSelector {
                executable: "c:/app/APP.exe".into(),
                title_contains: None,
            },
            Duration::from_millis(10),
        )
        .map_err(|error| error.failure)?;
    assert_eq!(state.title, "Two");
    Ok(())
}

#[test]
fn close_and_reuse_invalidates_the_old_reference() -> Result<(), WindowFailure> {
    let backend = FakeBackend {
        windows: vec![record(11, 5, "C:\\App\\app.exe", "Old")],
        foreground: Some(11),
        deny_activation: false,
    };
    let mut service = WindowService::new(backend, 8);
    let catalog = service.catalog()?;
    let old = WindowReference {
        generation: 8,
        reference: catalog.windows[0].window_ref.clone(),
    };
    service.backend.windows = vec![record(11, 5, "C:\\App\\app.exe", "New")];
    assert_eq!(
        service.activate_reference(&old, Duration::from_millis(10)),
        Err(WindowCommandError::before_effect(
            WindowFailure::StaleReference
        ))
    );
    Ok(())
}

#[test]
fn activation_is_not_success_until_foreground_is_confirmed() -> Result<(), WindowFailure> {
    let backend = FakeBackend {
        windows: vec![record(11, 5, "C:\\App\\app.exe", "App")],
        foreground: None,
        deny_activation: true,
    };
    let mut service = WindowService::new(backend, 9);
    let error = service
        .focus_selector(
            &WindowSelector {
                executable: "C:\\App\\app.exe".into(),
                title_contains: None,
            },
            Duration::from_millis(10),
        )
        .err()
        .ok_or(WindowFailure::OsApiUnavailable)?;
    assert_eq!(error.failure, WindowFailure::ForegroundDenied);
    assert_eq!(error.effect, super::super::EffectKnowledge::PossiblyApplied);
    Ok(())
}

#[test]
fn missing_executable_evidence_is_catalogued_but_never_matches_selector()
-> Result<(), WindowFailure> {
    let mut hidden = record(11, 5, "C:\\App\\app.exe", "Elevated");
    hidden.executable = None;
    let mut service = WindowService::new(
        FakeBackend {
            windows: vec![hidden],
            foreground: Some(11),
            deny_activation: false,
        },
        10,
    );
    assert_eq!(service.catalog()?.windows.len(), 1);
    let error = service
        .focus_selector(
            &WindowSelector {
                executable: "C:\\App\\app.exe".into(),
                title_contains: None,
            },
            Duration::from_millis(10),
        )
        .err()
        .ok_or(WindowFailure::OsApiUnavailable)?;
    assert_eq!(error.failure, WindowFailure::NoMatchingWindow);
    Ok(())
}
