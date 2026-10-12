// SPDX-License-Identifier: GPL-3.0-or-later
//! Host-owned, invocation-scoped interaction sessions.

mod model;
mod ports;
mod sessions;

pub use model::{
    CancelReason, InteractionItem, InteractionPage, InteractionRequest, InteractionResult,
    InteractionSession, InteractionViewKind,
};
pub use ports::{InteractionPresenter, InteractionProvider};
pub use sessions::{InteractionError, InteractionSessions};
