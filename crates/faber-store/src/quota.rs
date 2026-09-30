use crate::{StoreConfig, StoreError, StoreResult};

/// Content bytes and file count currently held by a store.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Usage {
    pub(crate) bytes: u64,
    pub(crate) entries: usize,
}

impl Usage {
    /// Fail with [`StoreError::StoreFull`] unless a new file of `size` bytes fits.
    pub(crate) fn admit(&self, size: u64, config: &StoreConfig) -> StoreResult<()> {
        if self.entries >= config.max_entries {
            return Err(StoreError::StoreFull(format!(
                "{} files stored (max: {})",
                self.entries, config.max_entries
            )));
        }
        if self.bytes.saturating_add(size) > config.max_total_bytes {
            return Err(StoreError::StoreFull(format!(
                "{} bytes stored, {size} more requested (max: {})",
                self.bytes, config.max_total_bytes
            )));
        }
        Ok(())
    }

    pub(crate) fn add(&mut self, size: u64) {
        self.bytes = self.bytes.saturating_add(size);
        self.entries += 1;
    }

    pub(crate) fn remove(&mut self, size: u64) {
        self.bytes = self.bytes.saturating_sub(size);
        self.entries = self.entries.saturating_sub(1);
    }
}
