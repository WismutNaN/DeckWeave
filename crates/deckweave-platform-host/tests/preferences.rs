// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::localization::LocaleContext;
use deckweave_platform::{LocalePreferenceStore, PreferenceError};
use deckweave_platform_host::{JsonLocalePreferenceStore, NativeAtomicFileStore};
use std::{error::Error, sync::Arc};

#[test]
fn permissions_are_durable_bounded_and_compare_exchanged() -> Result<(), Box<dyn Error>> {
    use deckweave_platform::{
        PermissionPreferenceStore, PermissionPreferences, PermissionSelection,
    };
    use deckweave_platform_host::JsonPermissionPreferenceStore;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("host/permissions.json");
    let open = || JsonPermissionPreferenceStore::new(path.clone(), Arc::new(NativeAtomicFileStore));
    let next = PermissionPreferences {
        revision: 2,
        selected: vec![PermissionSelection {
            package_id: deckweave_domain::ids::PackageId::try_new("org.test.provider")?,
            manifest_sha256: "a".repeat(64),
            permission_index: 0,
        }],
    };
    assert_eq!(open().load()?.revision, 1);
    open().compare_exchange(1, &next)?;
    open().compare_exchange(1, &next)?;
    assert_eq!(open().load()?, next);
    assert!(matches!(
        open().compare_exchange(
            1,
            &PermissionPreferences {
                revision: 2,
                selected: vec![]
            }
        ),
        Err(PreferenceError::Conflict)
    ));
    let revoked = PermissionPreferences {
        revision: 3,
        selected: vec![],
    };
    open().compare_exchange(2, &revoked)?;
    assert_eq!(open().load()?, revoked);
    std::fs::write(&path, vec![b' '; 1024 * 1024 + 1])?;
    assert!(open().load().is_err());
    std::fs::write(&path, b"invalid")?;
    assert!(open().load().is_err());
    Ok(())
}

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
