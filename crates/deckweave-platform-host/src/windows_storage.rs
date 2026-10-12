// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(unsafe_code)]

use deckweave_platform::StorageError;
use std::{os::windows::ffi::OsStrExt, path::Path, ptr};
use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACEFILE_WRITE_THROUGH, ReplaceFileW,
};

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn replace_file(
    target: &Path,
    replacement: &Path,
    backup: Option<&Path>,
) -> Result<(), StorageError> {
    let target_wide = wide(target);
    let replacement_wide = wide(replacement);
    if target.exists() {
        let backup_wide = backup.map(wide);
        let backup_pointer = backup_wide
            .as_ref()
            .map_or(ptr::null(), |value| value.as_ptr());
        // SAFETY: all paths are nul-terminated and remain alive for the call.
        let result = unsafe {
            ReplaceFileW(
                target_wide.as_ptr(),
                replacement_wide.as_ptr(),
                backup_pointer,
                REPLACEFILE_WRITE_THROUGH,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    } else {
        // SAFETY: both paths are nul-terminated and remain alive for the call.
        let result = unsafe {
            MoveFileExW(
                replacement_wide.as_ptr(),
                target_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}
