// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::localization::LocaleContext;
use deckweave_platform::{LocalePreferenceStore, PreferenceError};
use deckweave_platform_host::{JsonLocalePreferenceStore, NativeAtomicFileStore};
use std::{error::Error, sync::Arc};

#[test]
fn locale_survives_restart_retries_and_rejects_stale_or_corrupt_settings()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("locale.json");
    let open = || JsonLocalePreferenceStore::new(path.clone(), Arc::new(NativeAtomicFileStore));
    let next = LocaleContext {
        revision: 2,
        requested_locale: "ru-RU".into(),
        time_zone: "Asia/Yekaterinburg".into(),
    };
    assert_eq!(open().load()?, LocaleContext::default());
    open().compare_exchange(1, &next)?;
    open().compare_exchange(1, &next)?;
    assert_eq!(open().load()?, next);
    let other = LocaleContext {
        requested_locale: "en".into(),
        ..next
    };
    assert!(matches!(
        open().compare_exchange(1, &other),
        Err(PreferenceError::Conflict)
    ));
    std::fs::write(&path, b"invalid")?;
    assert!(matches!(open().load(), Err(PreferenceError::Invalid)));
    Ok(())
}
