use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub namespaces: NamespaceConfig,
    pub seccomp: SeccompConfig,
    pub capabilities: CapabilityConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceConfig {
    pub pid: bool,
    pub mount: bool,
    pub network: bool,
    pub ipc: bool,
    pub uts: bool,
    pub user: bool,
    pub time: bool,
    pub cgroup: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeccompConfig {
    pub enabled: bool,
    pub default_action: String,
    pub architectures: Vec<String>,
    pub syscalls: SyscallsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyscallsConfig {
    #[serde(default)]
    pub allowed: Vec<String>,
    pub blocked: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CapabilityConfig {
    pub enabled: bool,
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub drop_all: bool,
}
