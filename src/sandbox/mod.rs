//! Container-based sandboxing implementation
//!
//! This module provides secure container isolation for task execution,
//! similar to go-judge functionality.

pub mod error;
pub mod sandbox;

pub use sandbox::Sandbox;
