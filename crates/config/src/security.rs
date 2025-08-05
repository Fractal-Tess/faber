use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_security_level")]
    pub default_security_level: String,
    pub namespaces: NamespaceConfig,
    pub seccomp: SeccompConfig,
}

fn default_security_level() -> String {
    "medium".to_string()
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
