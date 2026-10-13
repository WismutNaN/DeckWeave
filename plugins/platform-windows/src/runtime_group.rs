// SPDX-License-Identifier: GPL-3.0-or-later
//! Package boundaries are independent of reusable adapters and endpoint schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeGroup {
    // Legacy artifacts remain readable for pinned workspaces until explicit migration.
    Control,
    InputObserve,
    Audio,
    Media,
    Windows,
    Applications,
    Keyboard,
}
impl RuntimeGroup {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::InputObserve => "input-observe",
            Self::Audio => "audio",
            Self::Media => "media",
            Self::Windows => "window",
            Self::Applications => "applications",
            Self::Keyboard => "keyboard",
        }
    }
    pub(crate) fn package(self) -> &'static str {
        match self {
            Self::Control | Self::InputObserve => "org.deckweave.platform.windows",
            Self::Audio => "org.deckweave.platform.windows.audio",
            Self::Media => "org.deckweave.platform.windows.media",
            Self::Windows => "org.deckweave.platform.windows.window",
            Self::Applications => "org.deckweave.platform.windows.applications",
            Self::Keyboard => "org.deckweave.platform.windows.keyboard",
        }
    }
    pub(crate) fn contribution(self) -> &'static str {
        match self {
            Self::Control | Self::Windows => "windows-control",
            Self::InputObserve | Self::Keyboard => "keyboard-observe",
            Self::Audio => "audio-control",
            Self::Media => "media-control",
            Self::Applications => "applications-control",
        }
    }
    pub(crate) fn audio(self) -> bool {
        matches!(self, Self::Control | Self::Audio)
    }
    pub(crate) fn windows(self) -> bool {
        matches!(self, Self::Control | Self::Windows)
    }
    pub(crate) fn input(self) -> bool {
        matches!(self, Self::InputObserve | Self::Keyboard)
    }
    pub(crate) fn broker(self) -> bool {
        matches!(self, Self::Control | Self::Applications)
    }
    pub(crate) fn allows(self, element: &str, capability: &str) -> bool {
        match self {
            Self::Control => true,
            Self::Audio => element == "audio",
            Self::Media => element == "media",
            Self::Windows => {
                element == "applications" && !matches!(capability, "launch" | "shell-open")
            }
            Self::Applications => {
                element == "applications" && matches!(capability, "launch" | "shell-open")
            }
            Self::InputObserve | Self::Keyboard => false,
        }
    }
}
