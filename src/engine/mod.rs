//! Trading engine module.
//!
//! This module contains the trading engines for both live and simulated execution:
//! - `live`: Real trading engine that connects to the exchange
//! - `simulation`: Dry-run engine for previewing orders without execution
//! - `common`: Shared utilities between engines
//! - `context`: Strategy execution context

pub mod common;
pub mod context;
pub mod live;
pub mod simulation;

// Re-export main types for convenient imports
pub use live::Engine;
pub use simulation::SimulationEngine;
