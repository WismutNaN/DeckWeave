// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::DeploymentMode;
use deckweave_platform::PlatformError;
use directories::BaseDirs;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LaunchOptions {
    pub portable: bool,
    pub data_root: Option<PathBuf>,
    pub background: bool,
}

impl LaunchOptions {
    /// Parses arguments without the executable name.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--portable") => options.portable = true,
                Some("--background") => options.background = true,
                Some("--data-root") => {
                    if options.data_root.is_some() {
                        return Err("--data-root указан дважды".into());
                    }
                    let value = args.next().ok_or("--data-root требует путь")?;
                    if value.is_empty() || value.to_string_lossy().starts_with("--") {
                        return Err("--data-root требует непустой путь".into());
                    }
                    options.data_root = Some(PathBuf::from(value));
                }
                _ => return Err(format!("Неизвестный аргумент: {}", arg.to_string_lossy())),
            }
        }
        Ok(options)
    }

    pub fn requested_root(
        &self,
        executable: &Path,
        cwd: &Path,
        installed: &Path,
    ) -> Result<PathBuf, PlatformError> {
        let path = if let Some(root) = &self.data_root {
            root.clone()
        } else if self.portable {
            executable
                .parent()
                .ok_or_else(|| PlatformError::Unavailable("Не найден каталог приложения".into()))?
                .join("DeckWeave-data")
        } else {
            installed.to_owned()
        };
        Ok(if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        })
    }

    #[must_use]
    pub fn mode(&self) -> DeploymentMode {
        if self.portable {
            DeploymentMode::Portable
        } else {
            DeploymentMode::Installed
        }
    }
}

pub fn installed_root() -> Result<PathBuf, PlatformError> {
    BaseDirs::new()
        .map(|dirs| dirs.data_local_dir().join("DeckWeave"))
        .ok_or_else(|| PlatformError::Unavailable("Нет пользовательского каталога данных".into()))
}
