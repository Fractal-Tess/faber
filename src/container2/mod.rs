pub mod builder;

/// Placeholder public API for a future pool wrapper around `ContainerRuntime`.
///
/// The pool is expected to own one or more prepared runtimes (hot workers)
/// and provide checked-out execution via a simple `run(tasks)` call.
///
/// This is intentionally not implemented yet; it exists to make the
/// `container2` public API forward-compatible.
#[derive(Debug, Clone)]
pub struct ContainerRuntimePool;
