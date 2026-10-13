// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[derive(Default)]
struct FakeBackend {
    windows: Vec<NativeWindow>,
    foreground: Option<u64>,
    deny_activation: bool,
    replace_before_activation: bool,
}

impl WindowBackend for FakeBackend {
    fn list_windows(&mut self) -> Result<Vec<NativeWindow>, WindowFailure> {
        Ok(self.windows.clone())
    }

    fn foreground_token(&mut self) -> Result<Option<u64>, WindowFailure> {
        Ok(self.foreground)
    }

    fn activate(
        &mut self,
        window: &NativeWindow,
        _timeout: Duration,
    ) -> Result<(), WindowCommandError> {
        if self.replace_before_activation {
            self.windows[0].incarnation += 1;
        }
        if !self
            .windows
            .iter()
            .any(|current| same_identity(current, window))
        {
            return Err(WindowCommandError::before_effect(
                WindowFailure::StaleReference,
            ));
        }
        if self.deny_activation {
            return Err(WindowCommandError::after_attempt(
                WindowFailure::ForegroundDenied,
            ));
        }
        self.foreground = Some(window.token);
        Ok(())
    }
}

fn record(token: u64, pid: u32, executable: &str, title: &str) -> NativeWindow {
    NativeWindow {
        token,
        incarnation: 1,
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
        replace_before_activation: false,
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
        replace_before_activation: false,
    };
    let mut service = WindowService::new(backend, 8);
    let catalog = service.catalog()?;
    let old = WindowReference {
        generation: 8,
        reference: catalog.windows[0].window_ref.clone(),
    };
    service.backend.windows = vec![record(11, 5, "C:\\App\\app.exe", "New")];
    service.backend.windows[0].incarnation += 1;
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
        replace_before_activation: false,
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
            replace_before_activation: false,
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

#[test]
fn title_change_updates_catalog_but_preserves_its_reference() -> Result<(), WindowFailure> {
    let backend = FakeBackend {
        windows: vec![record(11, 5, "app.exe", "old")],
        foreground: Some(11),
        deny_activation: false,
        replace_before_activation: false,
    };
    let mut service = WindowService::new(backend, 8);
    let before = service.catalog()?;
    service.backend.windows[0].title = "new".into();
    let after = service.catalog()?;
    assert_eq!(before.windows[0].window_ref, after.windows[0].window_ref);
    assert!(after.revision > before.revision);
    assert!(
        service
            .activate_reference(
                &WindowReference {
                    generation: 8,
                    reference: before.windows[0].window_ref.clone()
                },
                Duration::from_millis(10)
            )
            .is_ok()
    );
    Ok(())
}

#[test]
fn reuse_between_catalog_and_activation_is_a_pre_effect_rejection() -> Result<(), WindowFailure> {
    let backend = FakeBackend {
        windows: vec![record(11, 5, "app.exe", "title")],
        foreground: None,
        deny_activation: false,
        replace_before_activation: true,
    };
    let mut service = WindowService::new(backend, 8);
    let before = service.catalog()?;
    assert_eq!(
        service.activate_reference(
            &WindowReference {
                generation: 8,
                reference: before.windows[0].window_ref.clone()
            },
            Duration::from_millis(10)
        ),
        Err(WindowCommandError::before_effect(
            WindowFailure::StaleReference
        ))
    );
    assert_eq!(service.backend.foreground, None);
    Ok(())
}
