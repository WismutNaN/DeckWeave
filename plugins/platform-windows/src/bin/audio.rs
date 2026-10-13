// SPDX-License-Identifier: GPL-3.0-or-later
fn main() -> Result<(), Box<dyn std::error::Error>> {
    deckweave_platform_windows::run(deckweave_platform_windows::RuntimeGroup::Audio)
}
