// SPDX-License-Identifier: GPL-2.0-or-later
//! Rust/WASM behavior for the RF-Organ PLAY surface.

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(any(target_arch = "wasm32", test))]
mod client;
