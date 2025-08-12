use crate::environment::ContainerEnvironment;
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::types::{FilesystemConfig, Mount, RuntimeLimits};
use rand::{Rng, distr::Alphanumeric};
use std::path::PathBuf;
use tracing::debug;

/// Builder for constructing a `Runtime` with clear, typed configuration.
///
/// Use fluent methods to customize the container root, hostname, bind mounts,
/// working directory, and filesystem sizes. Call [`build`](Self::build) to
/// produce a ready-to-run [`Runtime`].
#[derive(Default)]
pub struct RuntimeBuilder {
    container_root: Option<PathBuf>,
    hostname: Option<String>,
    mounts: Option<Vec<Mount>>,
    work_dir: Option<String>,
    filesystem_config: Option<FilesystemConfig>,

    id: Option<String>,
}

impl RuntimeBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Provide custom bind mounts.
    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.mounts = Some(mounts);
        self
    }

    /// Set an explicit runtime identifier.
    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the working directory path inside the container view.
    pub fn with_workdir(mut self, work_dir: String) -> Self {
        self.work_dir = Some(work_dir);
        self
    }

    /// Set the host path that will become the container root.
    pub fn with_container_root(mut self, container_root: impl Into<PathBuf>) -> Self {
        self.container_root = Some(container_root.into());
        self
    }

    /// Set the container hostname (UTS namespace).
    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = Some(hostname);
        self
    }

    /// Sets the filesystem configuration for tmp and workdir sizes.
    ///
    /// # Arguments
    ///
    /// * `tmp_size` - Size limit for the `/tmp` filesystem (e.g., "128M", "1G")
    /// * `workdir_size` - Size limit for the working directory filesystem (e.g., "256M", "2G")
    ///
    /// # Example
    ///
    /// ```rust
    /// use faber::RuntimeBuilder;
    ///
    /// let runtime = RuntimeBuilder::new()
    ///     .with_filesystem_config("64M", "128M")
    ///     .build()?;
    /// ```
    pub fn with_filesystem_config(
        mut self,
        tmp_size: impl Into<String>,
        workdir_size: impl Into<String>,
    ) -> Self {
        self.filesystem_config = Some(FilesystemConfig {
            tmp_size: tmp_size.into(),
            workdir_size: workdir_size.into(),
        });
        self
    }

    /// Sets the tmp filesystem size.
    ///
    /// # Arguments
    ///
    /// * `tmp_size` - Size limit for the `/tmp` filesystem (e.g., "128M", "1G")
    ///
    /// # Example
    ///
    /// ```rust
    /// use faber::RuntimeBuilder;
    ///
    /// let runtime = RuntimeBuilder::new()
    ///     .with_tmp_size("64M")
    ///     .build()?;
    /// ```
    pub fn with_tmp_size(mut self, tmp_size: impl Into<String>) -> Self {
        let mut config = self.filesystem_config.unwrap_or_default();
        config.tmp_size = tmp_size.into();
        self.filesystem_config = Some(config);
        self
    }

    /// Sets the workdir filesystem size.
    ///
    /// # Arguments
    ///
    /// * `workdir_size` - Size limit for the working directory filesystem (e.g., "256M", "2G")
    ///
    /// # Example
    ///
    /// ```rust
    /// use faber::RuntimeBuilder;
    ///
    /// let runtime = RuntimeBuilder::new()
    ///     .with_workdir_size("128M")
    ///     .build()?;
    /// ```
    pub fn with_workdir_size(mut self, workdir_size: impl Into<String>) -> Self {
        let mut config = self.filesystem_config.unwrap_or_default();
        config.workdir_size = workdir_size.into();
        self.filesystem_config = Some(config);
        self
    }

    /// Finalize the configuration and create a [`Runtime`].
    ///
    /// Performs validation of mount entries and ensures defaults are applied:
    /// - Readonly bind mounts for common system paths (`/bin`, `/lib`, `/usr`, `/lib64`)
    /// - Random ID and container root under `/tmp/faber/containers/{id}`
    /// - Default hostname `"faber"` and workdir `"/faber"`
    pub fn build(self) -> Result<Runtime> {
        debug!("RuntimeBuilder::build: begin validation");
        // Validate required fields
        if let Some(ref mounts) = self.mounts {
            for mount in mounts {
                if mount.source.is_empty() {
                    return Err(Error::Validation {
                        field: "mount source".to_string(),
                        details: "Mount source cannot be empty".to_string(),
                    });
                }
                if mount.target.is_empty() {
                    return Err(Error::Validation {
                        field: "mount target".to_string(),
                        details: "Mount target cannot be empty".to_string(),
                    });
                }
            }
        }

        let flags = vec![
            nix::mount::MsFlags::MS_BIND,
            nix::mount::MsFlags::MS_REC,
            nix::mount::MsFlags::MS_RDONLY,
        ];
        let default_mounts: Vec<Mount> = ["/bin", "/lib", "/usr", "/lib64"]
            .iter()
            .map(|s| Mount {
                source: s.to_string(),
                target: s.to_string(),
                flags: flags.clone(),
                options: vec![],
                data: None,
            })
            .collect();

        let id: String = self.id.unwrap_or_else(|| {
            rand::rng()
                .sample_iter(&Alphanumeric)
                .take(12)
                .map(char::from)
                .collect()
        });
        let container_root = self
            .container_root
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/faber/containers/{id}")));
        let hostname = self.hostname.unwrap_or_else(|| "faber".into());
        let mounts = self.mounts.unwrap_or(default_mounts);
        let work_dir = self.work_dir.unwrap_or_else(|| "/faber".into());
        let filesystem_config = self.filesystem_config.unwrap_or_default();

        debug!(
            %id,
            root = %container_root.display(),
            %hostname,
            work_dir = %work_dir,
            tmp_size = %filesystem_config.tmp_size,
            workdir_size = %filesystem_config.workdir_size,
            mounts = mounts.len(),
            "RuntimeBuilder::build: resolved config"
        );

        // Validate work_dir
        if work_dir.is_empty() {
            return Err(Error::Validation {
                field: "work_dir".to_string(),
                details: "Work directory cannot be empty".to_string(),
            });
        }

        let env = ContainerEnvironment::new(
            container_root,
            hostname,
            mounts,
            work_dir,
            filesystem_config,
        );

        debug!("RuntimeBuilder::build: environment created, returning runtime");
        Ok(Runtime { env })
    }
}
