// SPDX-License-Identifier: GPL-3.0-or-later
//! Native data-root, instance-lock and opt-in autostart adapter.

#[cfg(windows)]
use crate::windows_autostart;
use crate::{LaunchOptions, installed_root};
#[cfg(not(windows))]
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use deckweave_domain::DeploymentMode;
use deckweave_platform::{HostLease, HostPlatform, PlatformError};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

#[cfg(windows)]
#[allow(unsafe_code)]
fn session_key() -> Result<String, PlatformError> {
    let mut id = 0;
    // SAFETY: указатель на живой u32; PID относится к текущему процессу.
    let result = unsafe {
        windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId(std::process::id(), &mut id)
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(id.to_string())
}

#[cfg(not(windows))]
fn session_key() -> Result<String, PlatformError> {
    use std::hash::{Hash, Hasher};
    let session = std::env::var("XDG_SESSION_ID")
        .or_else(|_| std::env::var("WAYLAND_DISPLAY"))
        .or_else(|_| std::env::var("DISPLAY"))
        .unwrap_or_else(|_| "user".into());
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    session.hash(&mut hash);
    Ok(format!("{:x}", hash.finish()))
}

/// Владелец advisory OS lock. Файл остаётся после выхода, блокировка — нет.
pub struct FileLease {
    _file: File,
}

fn is_lock_contention(error: &std::io::Error) -> bool {
    // Windows ERROR_LOCK_VIOLATION не отображается в ErrorKind::WouldBlock.
    error.kind() == std::io::ErrorKind::WouldBlock
        || error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
}
impl FileLease {
    pub fn acquire(path: &Path) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        fs2::FileExt::try_lock_exclusive(&file)?;
        Ok(Self { _file: file })
    }
}

struct NativeLease {
    _session: FileLease,
    _root: FileLease,
}
impl HostLease for NativeLease {}

pub struct NativePlatform {
    root: PathBuf,
    session_dir: PathBuf,
    mode: DeploymentMode,
    executable: PathBuf,
}

impl NativePlatform {
    pub fn new(
        options: &LaunchOptions,
        executable: PathBuf,
        cwd: &Path,
    ) -> Result<Self, PlatformError> {
        let installed = installed_root()?;
        let requested = options.requested_root(&executable, cwd, &installed)?;
        // Канонизация выполняется до Store/API/hardware. Directory creation
        // не изменяет профиль и не даёт права владения им.
        fs::create_dir_all(&requested)?;
        let root = fs::canonicalize(requested)?;
        let session_dir = installed.join("run");
        Ok(Self {
            root,
            session_dir,
            mode: options.mode(),
            executable,
        })
    }

    #[cfg(not(windows))]
    fn auto_launch(&self) -> Result<AutoLaunch, PlatformError> {
        if self.mode == DeploymentMode::Portable {
            return Err(PlatformError::PortableAutostart);
        }
        let executable = self
            .executable
            .to_str()
            .ok_or_else(|| PlatformError::Unavailable("Путь executable не UTF-8".into()))?;
        let root = self
            .root
            .to_str()
            .ok_or_else(|| PlatformError::Unavailable("Путь данных не UTF-8".into()))?;
        AutoLaunchBuilder::new()
            .set_app_name("DeckWeave")
            .set_app_path(executable)
            .set_args(&["--background", "--data-root", root])
            .build()
            .map_err(|error| PlatformError::Unavailable(error.to_string()))
    }
}

impl HostPlatform for NativePlatform {
    fn data_root(&self) -> &Path {
        &self.root
    }
    fn mode(&self) -> DeploymentMode {
        self.mode
    }

    fn acquire(&self) -> Result<Box<dyn HostLease>, PlatformError> {
        fs::create_dir_all(&self.session_dir)?;
        let session = FileLease::acquire(
            &self
                .session_dir
                .join(format!("session-{}.lock", session_key()?)),
        )
        .map_err(|error| {
            if is_lock_contention(&error) {
                PlatformError::SessionBusy
            } else {
                PlatformError::Io(error)
            }
        })?;
        let root = FileLease::acquire(&self.root.join(".writer.lock")).map_err(|error| {
            if is_lock_contention(&error) {
                PlatformError::RootBusy(self.root.clone())
            } else {
                PlatformError::Io(error)
            }
        })?;
        Ok(Box::new(NativeLease {
            _session: session,
            _root: root,
        }))
    }

    fn autostart_enabled(&self) -> Result<bool, PlatformError> {
        #[cfg(windows)]
        {
            if self.mode == DeploymentMode::Portable {
                return Err(PlatformError::PortableAutostart);
            }
            windows_autostart::registered(&self.executable, &self.root)
        }
        #[cfg(not(windows))]
        self.auto_launch()?
            .is_enabled()
            .map_err(|error| PlatformError::Unavailable(error.to_string()))
    }

    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError> {
        #[cfg(windows)]
        {
            if self.mode == DeploymentMode::Portable {
                return Err(PlatformError::PortableAutostart);
            }
            windows_autostart::set(&self.executable, &self.root, enabled)
        }
        #[cfg(not(windows))]
        {
            let launcher = self.auto_launch()?;
            let result = if enabled {
                launcher.enable()
            } else {
                launcher.disable()
            };
            result.map_err(|error| PlatformError::Unavailable(error.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NativeAtomicFileStore;
    use deckweave_platform::AtomicFileStore;
    use std::ffi::OsString;
    #[test]
    fn portable_and_explicit_root_are_independent_of_cwd() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let exe = temp.path().join("bin").join("DeckWeave.exe");
        let installed = temp.path().join("installed");
        let portable = LaunchOptions {
            portable: true,
            ..Default::default()
        };
        assert_eq!(
            portable.requested_root(&exe, temp.path(), &installed)?,
            temp.path().join("bin/DeckWeave-data")
        );
        let selected = LaunchOptions {
            data_root: Some("my-data".into()),
            ..portable
        };
        assert_eq!(
            selected.requested_root(&exe, temp.path(), &installed)?,
            temp.path().join("my-data")
        );
        Ok(())
    }

    #[test]
    fn invalid_arguments_do_not_silently_select_default_root() {
        for args in [
            vec!["--data-root"],
            vec!["--data-root", "--portable"],
            vec!["--unknown"],
            vec!["--data-root", "a", "--data-root", "b"],
        ] {
            assert!(LaunchOptions::parse(args.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn file_lock_is_released_without_deleting_lock_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("writer.lock");
        let lease = FileLease::acquire(&path)?;
        assert!(FileLease::acquire(&path).is_err());
        drop(lease);
        let _reacquired = FileLease::acquire(&path)?;
        Ok(())
    }

    fn test_platform(root: PathBuf, session_dir: PathBuf) -> NativePlatform {
        NativePlatform {
            root,
            session_dir,
            mode: DeploymentMode::Portable,
            executable: PathBuf::from("test-executable"),
        }
    }

    #[test]
    fn session_rejects_another_root_and_releases_after_exit()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let first_root = temp.path().join("first");
        let other_root = temp.path().join("other");
        fs::create_dir_all(&first_root)?;
        fs::create_dir_all(&other_root)?;
        let first = test_platform(first_root, temp.path().join("run"));
        let other = test_platform(other_root.clone(), temp.path().join("run"));
        let lease = first.acquire()?;
        assert!(matches!(other.acquire(), Err(PlatformError::SessionBusy)));
        assert!(!other_root.join(".writer.lock").exists());
        drop(lease);
        let _after_exit = other.acquire()?;
        Ok(())
    }

    #[test]
    fn root_conflict_releases_the_new_session_lock() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let first = test_platform(temp.path().to_owned(), temp.path().join("session-a"));
        let other = test_platform(temp.path().to_owned(), temp.path().join("session-b"));
        let first_lease = first.acquire()?;
        assert!(matches!(other.acquire(), Err(PlatformError::RootBusy(_))));
        let other_session = other
            .session_dir
            .join(format!("session-{}.lock", session_key()?));
        let _released_session = FileLease::acquire(&other_session)?;
        drop(first_lease);
        let _released_root = FileLease::acquire(&temp.path().join(".writer.lock"))?;
        Ok(())
    }

    #[test]
    fn selected_root_is_canonical_before_services_start() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        fs::create_dir_all(temp.path().join("nested"))?;
        let options = LaunchOptions {
            portable: true,
            data_root: Some(temp.path().join("nested/..")),
            ..Default::default()
        };
        let platform = NativePlatform::new(&options, temp.path().join("app"), temp.path())?;
        assert_eq!(platform.data_root(), temp.path().canonicalize()?);
        assert_eq!(platform.mode(), DeploymentMode::Portable);
        Ok(())
    }

    #[test]
    fn portable_autostart_is_never_registered() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let platform = NativePlatform {
            root: temp.path().into(),
            session_dir: temp.path().join("run"),
            mode: DeploymentMode::Portable,
            executable: temp.path().join("app"),
        };
        assert!(matches!(
            platform.set_autostart(true),
            Err(PlatformError::PortableAutostart)
        ));
        assert!(matches!(
            platform.autostart_enabled(),
            Err(PlatformError::PortableAutostart)
        ));
        Ok(())
    }

    #[test]
    fn atomic_store_replaces_target_and_keeps_previous_backup()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("profile.json");
        let backup = directory.path().join("profile.json.bak");
        let store = NativeAtomicFileStore;
        store.write_atomic(&target, b"first", None)?;
        store.write_atomic(&target, b"second", Some(&backup))?;
        assert_eq!(fs::read(&target)?, b"second");
        assert_eq!(fs::read(&backup)?, b"first");
        store.write_atomic(&target, b"third", Some(&backup))?;
        assert_eq!(fs::read(&target)?, b"third");
        assert_eq!(fs::read(&backup)?, b"second");
        assert!(
            fs::read_dir(directory.path())?
                .filter_map(Result::ok)
                .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-"))
        );
        Ok(())
    }
}
