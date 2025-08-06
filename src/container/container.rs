use std::sync::Arc;

use crate::config::FaberConfig;

pub struct Container {
    config: Arc<FaberConfig>,
}

impl Container {
    pub fn new(config: Arc<FaberConfig>) -> Self {
        Self { config }
    }
}
