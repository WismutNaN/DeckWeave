// SPDX-License-Identifier: GPL-3.0-or-later
//! Только HKCU: никогда не изменяем автозапуск других пользователей.
use deckweave_platform::PlatformError;
use std::path::Path;
use windows_registry::CURRENT_USER;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "DeckWeave";

// Quoting для Windows argv, не для cmd.exe/PowerShell (shell не запускается).
fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
        } else {
            output.push_str(&"\\".repeat(if ch == '"' { slashes * 2 + 1 } else { slashes }));
            output.push(ch);
            slashes = 0;
        }
    }
    output.push_str(&"\\".repeat(slashes * 2));
    output.push('"');
    output
}

fn command(executable: &Path, root: &Path) -> Result<String, PlatformError> {
    let exe = executable
        .to_str()
        .ok_or_else(|| PlatformError::Unavailable("Путь executable не UTF-8".into()))?;
    let root = root
        .to_str()
        .ok_or_else(|| PlatformError::Unavailable("Путь данных не UTF-8".into()))?;
    Ok(format!(
        "{} --background --data-root {}",
        quote(exe),
        quote(root)
    ))
}

pub(super) fn registered(executable: &Path, root: &Path) -> Result<bool, PlatformError> {
    let expected = command(executable, root)?;
    match CURRENT_USER
        .open(RUN_KEY)
        .and_then(|key| key.get_string(APP_NAME))
        .map_err(std::io::Error::from)
    {
        Ok(value) => Ok(value == expected),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn set(executable: &Path, root: &Path, enabled: bool) -> Result<(), PlatformError> {
    let result = if enabled {
        let value = command(executable, root)?;
        CURRENT_USER
            .create(RUN_KEY)
            .and_then(|key| key.set_string(APP_NAME, value))
    } else {
        CURRENT_USER
            .options()
            .write()
            .open(RUN_KEY)
            .and_then(|key| key.remove_value(APP_NAME))
    };
    match result.map_err(std::io::Error::from) {
        Ok(()) => Ok(()),
        Err(error) if !enabled && error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_quotes_and_trailing_slashes_are_preserved() -> Result<(), PlatformError> {
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote("C:\\"), "\"C:\\\\\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(
            command(
                Path::new(r"C:\Program Files\DeckWeave\app.exe"),
                Path::new(r"G:\Мои данные")
            )?,
            "\"C:\\Program Files\\DeckWeave\\app.exe\" --background --data-root \"G:\\Мои данные\""
        );
        Ok(())
    }
}
