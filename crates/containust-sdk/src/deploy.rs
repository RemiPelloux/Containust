//! Declarative deployment: plan and apply a `.ctst` composition.
//!
//! Re-exports the runtime engine API that powers `ctst plan` and
//! `ctst run`, so library users get the same idempotent apply semantics:
//! unchanged components are kept, stopped ones started, changed ones
//! replaced in dependency order (with no downtime for components that set
//! `update = "start-first"`), and undeclared ones removed with `prune`.
//!
//! # Example
//!
//! ```rust,no_run
//! use std::path::Path;
//! use containust_sdk::deploy::{ApplyOptions, Engine, EngineOptions, PlanAction};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let file = Path::new("stack.ctst");
//! let data_dir = containust_common::constants::project_dir(file);
//! let engine = Engine::with_options(EngineOptions {
//!     state_file: data_dir.join("state").join("state.json"),
//!     data_dir,
//!     offline: false,
//! });
//!
//! let plan = engine.plan(file)?;
//! for change in plan.changes.iter().filter(|c| c.action != PlanAction::Keep) {
//!     println!("{} {} ({:?})", change.action.symbol(), change.name, change.strategy);
//! }
//! if !plan.is_noop() {
//!     let outcome = engine.apply(file, ApplyOptions { prune: false })?;
//!     println!("{} component(s) running", outcome.deployed.len());
//! }
//! # Ok(())
//! # }
//! ```

pub use containust_common::types::UpdateStrategy;
pub use containust_runtime::engine::{
    ApplyOptions, ApplyOutcome, DEFAULT_STOP_GRACE, DeployedComponent, Engine, EngineOptions,
};
pub use containust_runtime::plan::{DeployPlan, PlanAction, PlannedChange};
