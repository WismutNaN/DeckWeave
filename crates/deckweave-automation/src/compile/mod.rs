// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure semantic compilation and runtime binding.

mod binding;
mod catalog;
mod compiler;
mod semantics;
mod topology;

pub use binding::{RuntimeBindError, RuntimeBinder};
pub use catalog::{
    CatalogEndpoint, CatalogError, CatalogSchema, DescriptorCatalog, SchemaValueKind,
};
pub use compiler::{CompileAccess, GraphCompileError, GraphPlanCompiler};
