// SPDX-License-Identifier: GPL-3.0-or-later
//! A bundled, memory-only provider exercised through the normal supervisor.
mod runtime;
mod wire;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    runtime::run()
}

#[cfg(test)]
mod tests;
