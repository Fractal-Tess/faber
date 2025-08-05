use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub default_security_level: String,
    pub namespaces: NamespaceConfig,
    pub seccomp: SeccompConfig,
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
    pub allowed: Vec<String>,
    pub disallowed: Vec<String>,
}
