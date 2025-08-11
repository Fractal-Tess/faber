use crate::cgroup::CgroupManager;
use crate::environment::ContainerEnvironment;
use crate::runtime::Runtime;
use crate::types::{CgroupConfig, Mount, RuntimeLimits};
use rand::{Rng, distr::Alphanumeric};
use std::path::PathBuf;

#[derive(Default)]
pub struct RuntimeBuilder {
    container_root: Option<PathBuf>,
    hostname: Option<String>,
    mounts: Option<Vec<Mount>>,
    work_dir: Option<String>,
    limits: Option<RuntimeLimits>,
    cgroups: Option<CgroupConfig>,
    id: Option<String>,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.mounts = Some(mounts);
        self
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_workdir(mut self, work_dir: String) -> Self {
        self.work_dir = Some(work_dir);
        self
    }
    pub fn with_container_root(mut self, container_root: impl Into<PathBuf>) -> Self {
        self.container_root = Some(container_root.into());
        self
    }
    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = Some(hostname);
        self
    }
    pub fn with_runtime_limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = Some(limits);
        self
    }
    pub fn with_cgroups(mut self, cgroups: CgroupConfig) -> Self {
        self.cgroups = Some(cgroups);
        self
    }

    pub fn build(self) -> Runtime {
        let flags = vec![
            nix::mount::MsFlags::MS_BIND,
            nix::mount::MsFlags::MS_REC,
            nix::mount::MsFlags::MS_RDONLY,
        ];
        let default_mounts: Vec<Mount> = ["/bin", "/lib", "/usr", "/lib64", "/sbin"]
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

        let env = ContainerEnvironment::new(container_root, hostname, mounts, work_dir);
        let limits = self.limits.unwrap_or_default();
        let cgroups = self.cgroups.unwrap_or_default();

        // Create the cgroup manager during build
        println!("=== Builder: Creating cgroup manager ===");
        let cgroup_manager = CgroupManager::new(&id).expect("Failed to create cgroup manager");
        println!("✓ Builder: Cgroup manager created successfully");

        // Note: Cgroup validation removed - simplified approach just creates directory and sets pids.max
        println!("✓ Builder: Cgroup manager created with simplified approach");

        // Apply cgroup configuration if provided
        if cgroups.enabled {
            println!("=== Builder: Applying cgroup configuration ===");
            if let Some(pids_max) = cgroups.pids_max {
                println!("Setting pids.max to {}", pids_max);
                if let Err(e) = cgroup_manager.set_max_procs(pids_max) {
                    eprintln!("Warning: Failed to set max procs during build: {:?}", e);
                } else {
                    println!("✓ Builder: pids.max set successfully");
                }
            }
            // TODO: Add support for memory_max and cpu_max when those methods are implemented
        } else {
            println!("Builder: No cgroup configuration provided, using defaults");
        }

        println!("=== Builder: Cgroup setup complete ===");

        Runtime {
            id,
            env,
            limits,
            cgroup_manager,
        }
    }
}
