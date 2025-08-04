use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub default_security_level: String,
    pub namespaces: NamespaceConfig,
    pub seccomp: SeccompConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompConfig {
    pub enabled: bool,
    pub default_action: String,
    pub architectures: Vec<String>,
    pub syscalls: SyscallsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallsConfig {
    pub allowed: Vec<String>,
    pub disallowed: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            default_security_level: "standard".to_string(),
            namespaces: NamespaceConfig::default(),
            seccomp: SeccompConfig::default(),
        }
    }
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            pid: false,
            mount: true,
            network: true,
            ipc: true,
            uts: true,
            user: true,
            time: false,
            cgroup: true,
        }
    }
}

impl Default for SeccompConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_action: "SCMP_ACT_ERRNO".to_string(),
            architectures: vec![
                "SCMP_ARCH_X86_64".to_string(),
                "SCMP_ARCH_X86".to_string(),
                "SCMP_ARCH_AARCH64".to_string(),
            ],
            syscalls: SyscallsConfig::default(),
        }
    }
}

impl Default for SyscallsConfig {
    fn default() -> Self {
        Self {
            allowed: vec![
                // Basic file operations
                "read".to_string(),
                "write".to_string(),
                "open".to_string(),
                "close".to_string(),
                "fstat".to_string(),
                "stat".to_string(),
                "lstat".to_string(),
                "lseek".to_string(),
                // Memory management
                "mmap".to_string(),
                "mprotect".to_string(),
                "munmap".to_string(),
                "brk".to_string(),
                "mremap".to_string(),
                "msync".to_string(),
                "mincore".to_string(),
                "madvise".to_string(),
                // Process control
                "clone".to_string(),
                "fork".to_string(),
                "vfork".to_string(),
                "execve".to_string(),
                "exit".to_string(),
                "exit_group".to_string(),
                "wait4".to_string(),
                "waitid".to_string(),
                // Signal handling
                "rt_sigaction".to_string(),
                "rt_sigprocmask".to_string(),
                "rt_sigreturn".to_string(),
                "sigaltstack".to_string(),
                "rt_sigsuspend".to_string(),
                // I/O operations
                "ioctl".to_string(),
                "pread64".to_string(),
                "pwrite64".to_string(),
                "readv".to_string(),
                "writev".to_string(),
                "sendfile".to_string(),
                // File system operations
                "access".to_string(),
                "pipe".to_string(),
                "dup".to_string(),
                "dup2".to_string(),
                "dup3".to_string(),
                "fcntl".to_string(),
                "flock".to_string(),
                "fsync".to_string(),
                "fdatasync".to_string(),
                // Directory operations
                "getdents".to_string(),
                "getdents64".to_string(),
                "getcwd".to_string(),
                "chdir".to_string(),
                "fchdir".to_string(),
                // File creation and modification
                "creat".to_string(),
                "link".to_string(),
                "unlink".to_string(),
                "symlink".to_string(),
                "readlink".to_string(),
                "chmod".to_string(),
                "fchmod".to_string(),
                "chown".to_string(),
                "fchown".to_string(),
                "lchown".to_string(),
                // Time and scheduling
                "gettimeofday".to_string(),
                "nanosleep".to_string(),
                "clock_gettime".to_string(),
                "clock_getres".to_string(),
                "clock_nanosleep".to_string(),
                // Process information
                "getpid".to_string(),
                "getppid".to_string(),
                "getuid".to_string(),
                "geteuid".to_string(),
                "getgid".to_string(),
                "getegid".to_string(),
                "gettid".to_string(),
                // Resource limits
                "getrlimit".to_string(),
                "setrlimit".to_string(),
                "getrusage".to_string(),
                "prlimit64".to_string(),
                // System information
                "uname".to_string(),
                "sysinfo".to_string(),
                "times".to_string(),
                "syslog".to_string(),
                // Network operations (basic)
                "socket".to_string(),
                "connect".to_string(),
                "accept".to_string(),
                "bind".to_string(),
                "listen".to_string(),
                "getsockname".to_string(),
                "getpeername".to_string(),
                // Network I/O
                "sendto".to_string(),
                "recvfrom".to_string(),
                "sendmsg".to_string(),
                "recvmsg".to_string(),
                "shutdown".to_string(),
                "setsockopt".to_string(),
                "getsockopt".to_string(),
                // Process groups and sessions
                "setpgid".to_string(),
                "getpgid".to_string(),
                "getpgrp".to_string(),
                "setsid".to_string(),
                "getsid".to_string(),
                // User and group management
                "setuid".to_string(),
                "setgid".to_string(),
                "setreuid".to_string(),
                "setregid".to_string(),
                "setresuid".to_string(),
                "getresuid".to_string(),
                "setresgid".to_string(),
                "getresgid".to_string(),
                // Supplementary groups
                "getgroups".to_string(),
                "setgroups".to_string(),
                // File system attributes
                "umask".to_string(),
                "statfs".to_string(),
                "fstatfs".to_string(),
                // Advanced file operations
                "truncate".to_string(),
                "ftruncate".to_string(),
                "rename".to_string(),
                "mkdir".to_string(),
                "rmdir".to_string(),
                // Memory locking
                "mlock".to_string(),
                "munlock".to_string(),
                "mlockall".to_string(),
                "munlockall".to_string(),
                // Modern features
                "getrandom".to_string(),
                "memfd_create".to_string(),
                "eventfd".to_string(),
                "eventfd2".to_string(),
                "timerfd_create".to_string(),
                "timerfd_settime".to_string(),
                "timerfd_gettime".to_string(),
                // Epoll for I/O multiplexing
                "epoll_create".to_string(),
                "epoll_create1".to_string(),
                "epoll_ctl".to_string(),
                "epoll_wait".to_string(),
                "epoll_pwait".to_string(),
                // Futex for synchronization
                "futex".to_string(),
                "set_robust_list".to_string(),
                "get_robust_list".to_string(),
                // Scheduler operations
                "sched_yield".to_string(),
                "sched_setparam".to_string(),
                "sched_getparam".to_string(),
                "sched_setscheduler".to_string(),
                "sched_getscheduler".to_string(),
                // Priority operations
                "getpriority".to_string(),
                "setpriority".to_string(),
                "ioprio_set".to_string(),
                "ioprio_get".to_string(),
                // Advanced I/O
                "io_setup".to_string(),
                "io_destroy".to_string(),
                "io_getevents".to_string(),
                "io_submit".to_string(),
                "io_cancel".to_string(),
                // File descriptor operations
                "close_range".to_string(),
                "pidfd_open".to_string(),
                "pidfd_getfd".to_string(),
                // Modern file operations
                "openat".to_string(),
                "mkdirat".to_string(),
                "mknodat".to_string(),
                "fchownat".to_string(),
                "futimesat".to_string(),
                "newfstatat".to_string(),
                "unlinkat".to_string(),
                "renameat".to_string(),
                "linkat".to_string(),
                "symlinkat".to_string(),
                "readlinkat".to_string(),
                "fchmodat".to_string(),
                "faccessat".to_string(),
                "faccessat2".to_string(),
                // Extended attributes
                "setxattr".to_string(),
                "lsetxattr".to_string(),
                "fsetxattr".to_string(),
                "getxattr".to_string(),
                "lgetxattr".to_string(),
                "fgetxattr".to_string(),
                "listxattr".to_string(),
                "llistxattr".to_string(),
                "flistxattr".to_string(),
                "removexattr".to_string(),
                "lremovexattr".to_string(),
                "fremovexattr".to_string(),
                // Process memory operations
                "process_vm_readv".to_string(),
                "process_vm_writev".to_string(),
                "process_madvise".to_string(),
                "process_mrelease".to_string(),
                // Modern system calls
                "statx".to_string(),
                "copy_file_range".to_string(),
                "preadv2".to_string(),
                "pwritev2".to_string(),
                "pkey_mprotect".to_string(),
                "pkey_alloc".to_string(),
                "pkey_free".to_string(),
                // Restart syscall (for signal handling)
                "restart_syscall".to_string(),
            ],
            disallowed: vec![],
        }
    }
}
