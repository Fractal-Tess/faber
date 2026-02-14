#[cfg(feature = "memory")]
mod memory;

#[cfg(feature = "memory")]
pub use memory::MemoryStore;

#[cfg(feature = "filesystem")]
mod filesystem;

#[cfg(feature = "filesystem")]
pub use filesystem::FilesystemStore;

#[cfg(all(feature = "memory", feature = "filesystem"))]
mod hybrid;

#[cfg(all(feature = "memory", feature = "filesystem"))]
pub use hybrid::HybridStore;
