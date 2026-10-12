// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-owned, invocation-scoped interaction sessions.

mod model;
mod ports;
pub mod presentation;
mod views;
pub use views::DesktopViews;
mod sessions;

pub use model::{
    CancelReason, InteractionItem, InteractionPage, InteractionRequest, InteractionResult,
    InteractionSession, InteractionViewKind, QueryFence,
};
pub use ports::{InteractionPresenter, InteractionProvider};
pub use sessions::{InteractionError, InteractionSessions};
