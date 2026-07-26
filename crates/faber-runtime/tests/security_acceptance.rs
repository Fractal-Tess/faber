use faber_runtime::{
    CgroupConfigBuilder, ContainerConfigBuilder, ExecutionStep, ExecutionStepResult,
    RuntimeBuilder, RuntimeResult, SandboxProfile, Task, TaskOutcome, TaskResult,
};
use nix::libc;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

static SECURITY_TEST_LOCK: Mutex<()> = Mutex::new(());

const SECURITY_PROBE_SOURCE: &str = include_str!("fixtures/security_probe.c");

#[derive(Debug, Deserialize)]
struct RlimitState {
    soft: u64,
    hard: u64,
}

#[derive(Debug, Deserialize)]
struct SecurityState {
    pid: u32,
    ppid: u32,
    uid: u32,
    euid: u32,
    gid: u32,
    egid: u32,
    groups: Vec<u32>,
    namespaces: HashMap<String, u64>,
    uid_map: String,
    gid_map: String,
    status: String,
    cgroup: String,
    mountinfo: String,
    route4: String,
    route6: String,
    rlimits: HashMap<String, RlimitState>,
}

const PID_PROBE_SOURCE: &str = r#"
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    pid_t children[32];
    int started = 0;
    int fork_error = 0;

    while (started < 32) {
        pid_t child = fork();
        if (child == 0) {
            pause();
            _exit(0);
        }
        if (child < 0) {
            fork_error = errno;
            break;
        }
        children[started++] = child;
    }

    printf("%d %d\n", started, fork_error);
    fflush(stdout);

    for (int i = 0; i < started; i++) {
        kill(children[i], SIGKILL);
    }
    for (int i = 0; i < started; i++) {
        waitpid(children[i], NULL, 0);
    }

    return started < 32 && fork_error == EAGAIN ? 0 : 1;
}
"#;

const ORPHAN_PROBE_SOURCE: &str = r#"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    pid_t child = fork();
    if (child < 0) {
        return 1;
    }
    if (child == 0) {
        int marker = open("orphan.pid", O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0644);
        if (marker < 0) {
            _exit(2);
        }
        dprintf(marker, "%d\n", getpid());
        close(marker);
        close(STDIN_FILENO);
        close(STDOUT_FILENO);
        close(STDERR_FILENO);
        pause();
        _exit(0);
    }

    for (int attempt = 0; attempt < 100; attempt++) {
        if (access("orphan.pid", F_OK) == 0) {
            return 0;
        }
        usleep(1000);
    }
    return 3;
}
"#;

const SECCOMP_PROBE_SOURCE: &str = r#"
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

struct syscall_entry {
    const char *name;
    long number;
};

int main(int argc, char **argv) {
    if (argc != 2) {
        return 64;
    }
    const struct syscall_entry entries[] = {
        {"acct", SYS_acct},
        {"add_key", SYS_add_key},
        {"bpf", SYS_bpf},
        {"clone", SYS_clone},
        {"clone3", SYS_clone3},
        {"delete_module", SYS_delete_module},
        {"fanotify_init", SYS_fanotify_init},
        {"finit_module", SYS_finit_module},
        {"fork", SYS_fork},
        {"init_module", SYS_init_module},
        {"io_uring_setup", SYS_io_uring_setup},
        {"kcmp", SYS_kcmp},
        {"kexec_load", SYS_kexec_load},
        {"keyctl", SYS_keyctl},
        {"mount", SYS_mount},
        {"open_by_handle_at", SYS_open_by_handle_at},
        {"perf_event_open", SYS_perf_event_open},
        {"pivot_root", SYS_pivot_root},
        {"process_vm_readv", SYS_process_vm_readv},
        {"process_vm_writev", SYS_process_vm_writev},
        {"ptrace", SYS_ptrace},
        {"quotactl", SYS_quotactl},
        {"reboot", SYS_reboot},
        {"request_key", SYS_request_key},
        {"setns", SYS_setns},
        {"socket", SYS_socket},
        {"socketpair", SYS_socketpair},
        {"swapoff", SYS_swapoff},
        {"swapon", SYS_swapon},
        {"umount2", SYS_umount2},
        {"unshare", SYS_unshare},
        {"userfaultfd", SYS_userfaultfd},
        {"vfork", SYS_vfork},
    };
    for (size_t index = 0; index < sizeof(entries) / sizeof(entries[0]); index++) {
        if (strcmp(argv[1], entries[index].name) == 0) {
            syscall(entries[index].number, 0, 0, 0, 0, 0, 0);
            return 2;
        }
    }
    fprintf(stderr, "unknown syscall: %s\n", argv[1]);
    return 65;
}
"#;

const PRIVILEGE_ESCAPE_PROBE_SOURCE: &str = r#"
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <linux/capability.h>
#include <linux/limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/types.h>
#include <unistd.h>

static int failures = 0;

static void failed(const char *operation) {
    fprintf(stderr, "%s unexpectedly succeeded or exposed privileged state (errno=%d)\n", operation, errno);
    failures++;
}

static void require_failure(const char *operation, long result) {
    if (result != -1) {
        failed(operation);
    }
}

static void require_map_write_failure(const char *path) {
    int fd = open(path, O_WRONLY | O_CLOEXEC);
    if (fd < 0) {
        return;
    }
    errno = 0;
    if (write(fd, "0 0 1\n", 6) >= 0) {
        failed(path);
    }
    close(fd);
}

static void verify_file_descriptors(void) {
    DIR *directory = opendir("/proc/self/fd");
    if (directory == NULL) {
        failed("opendir(/proc/self/fd)");
        return;
    }
    int directory_fd = dirfd(directory);
    struct dirent *entry;
    while ((entry = readdir(directory)) != NULL) {
        char *end = NULL;
        long fd = strtol(entry->d_name, &end, 10);
        if (*entry->d_name != '\0' && end != NULL && *end == '\0' &&
            fd > STDERR_FILENO && fd != directory_fd) {
            fprintf(stderr, "inherited descriptor %ld\n", fd);
            failures++;
        }
    }
    closedir(directory);
}

static void verify_visible_processes(void) {
    DIR *directory = opendir("/proc");
    if (directory == NULL) {
        failed("opendir(/proc)");
        return;
    }
    int process_count = 0;
    struct dirent *entry;
    while ((entry = readdir(directory)) != NULL) {
        char *end = NULL;
        strtol(entry->d_name, &end, 10);
        if (*entry->d_name != '\0' && end != NULL && *end == '\0') {
            process_count++;
        }
    }
    closedir(directory);
    if (process_count > 2) {
        fprintf(stderr, "procfs exposed %d processes\n", process_count);
        failures++;
    }
}

int main(void) {
    gid_t root_group = 0;
    require_failure("setuid(0)", setuid(0));
    require_failure("seteuid(0)", seteuid(0));
    require_failure("setgid(0)", setgid(0));
    require_failure("setegid(0)", setegid(0));
    require_failure("setgroups(root)", setgroups(1, &root_group));
    require_failure("chroot", chroot("/"));
    require_failure("sethostname", sethostname("escaped", 7));
    require_failure("mknod device", mknod("escape-device", S_IFCHR | 0600, makedev(1, 3)));
    require_failure("kill namespace init", kill(1, SIGKILL));
    require_failure("unset no_new_privs", prctl(PR_SET_NO_NEW_PRIVS, 0, 0, 0, 0));
    require_failure(
        "raise ambient CAP_SYS_ADMIN",
        prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, CAP_SYS_ADMIN, 0, 0)
    );

    struct __user_cap_header_struct header = {
        .version = _LINUX_CAPABILITY_VERSION_3,
        .pid = 0,
    };
    struct __user_cap_data_struct capabilities[2] = {0};
    capabilities[CAP_TO_INDEX(CAP_SYS_ADMIN)].effective = CAP_TO_MASK(CAP_SYS_ADMIN);
    capabilities[CAP_TO_INDEX(CAP_SYS_ADMIN)].permitted = CAP_TO_MASK(CAP_SYS_ADMIN);
    require_failure("capset CAP_SYS_ADMIN", syscall(SYS_capset, &header, capabilities));

    require_map_write_failure("/proc/self/uid_map");
    require_map_write_failure("/proc/self/gid_map");
    require_map_write_failure("/proc/self/setgroups");

    int root_fd = open("/proc/1/root/bin/sh", O_RDONLY | O_CLOEXEC);
    if (root_fd >= 0) {
        close(root_fd);
        failed("open(/proc/1/root/bin/sh)");
    }
    char root_target[PATH_MAX];
    require_failure("readlink(/proc/1/root)", readlink("/proc/1/root", root_target, sizeof(root_target)));

    if (access("/sys/fs/cgroup/cgroup.procs", F_OK) == 0) {
        failed("visible cgroup control filesystem");
    }
    if (getuid() != 65534 || geteuid() != 65534 || getgid() != 65534 || getegid() != 65534) {
        failed("effective nobody identity");
    }

    verify_file_descriptors();
    verify_visible_processes();
    return failures == 0 ? 0 : 1;
}
"#;

const FILESYSTEM_OBJECT_PROBE_SOURCE: &str = r#"
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <sys/un.h>
#include <unistd.h>

int main(void) {
    if (mkdir("object-directory", 0700) != 0 || mkfifo("object-fifo", 0600) != 0) {
        return 1;
    }

    int socket_fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (socket_fd < 0) {
        return 2;
    }
    struct sockaddr_un address = { .sun_family = AF_UNIX };
    const char socket_path[] = "object-socket";
    for (size_t index = 0; index < sizeof(socket_path); index++) {
        address.sun_path[index] = socket_path[index];
    }
    if (bind(socket_fd, (struct sockaddr *)&address, sizeof(address)) != 0) {
        return 3;
    }
    close(socket_fd);

    if (symlink("/proc/self/fd/1", "object-magic-link") != 0) {
        return 4;
    }
    errno = 0;
    if (link("/bin/sh", "object-hard-link") == 0) {
        return 5;
    }
    return 0;
}
"#;

const NETWORK_ESCAPE_PROBE_SOURCE: &str = r#"
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

static int failures = 0;

static void connect_must_fail(int family, const void *address, socklen_t length) {
    int fd = socket(family, SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK, 0);
    if (fd < 0) {
        return;
    }
    int result = connect(fd, address, length);
    if (result == 0) {
        fprintf(stderr, "external connect succeeded for family %d\n", family);
        failures++;
        close(fd);
        return;
    }
    if (errno == EINPROGRESS) {
        struct pollfd poll_fd = { .fd = fd, .events = POLLOUT };
        if (poll(&poll_fd, 1, 100) > 0) {
            int socket_error = 0;
            socklen_t error_length = sizeof(socket_error);
            if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &socket_error, &error_length) == 0 && socket_error == 0) {
                fprintf(stderr, "external async connect succeeded for family %d\n", family);
                failures++;
            }
        }
    }
    close(fd);
}

static void verify_no_default_routes(void) {
    FILE *routes = fopen("/proc/net/route", "r");
    if (routes == NULL) {
        failures++;
        return;
    }
    char line[512];
    fgets(line, sizeof(line), routes);
    while (fgets(line, sizeof(line), routes) != NULL) {
        char interface[64];
        char destination[64];
        if (sscanf(line, "%63s %63s", interface, destination) == 2 &&
            strcmp(destination, "00000000") == 0) {
            fprintf(stderr, "IPv4 default route visible: %s", line);
            failures++;
        }
    }
    fclose(routes);

    routes = fopen("/proc/net/ipv6_route", "r");
    if (routes == NULL) {
        failures++;
        return;
    }
    while (fgets(line, sizeof(line), routes) != NULL) {
        char destination[65];
        char source[65];
        char next_hop[65];
        char interface[64];
        unsigned int prefix = 1;
        unsigned int source_prefix = 1;
        unsigned int metric, reference_count, use_count, flags;
        if (sscanf(
                line,
                "%64s %x %64s %x %64s %x %x %x %x %63s",
                destination,
                &prefix,
                source,
                &source_prefix,
                next_hop,
                &metric,
                &reference_count,
                &use_count,
                &flags,
                interface
            ) == 10 && prefix == 0 && strcmp(interface, "lo") != 0) {
            fprintf(stderr, "external IPv6 default route visible: %s", line);
            failures++;
        }
    }
    fclose(routes);
}

int main(void) {
    struct sockaddr_in ipv4 = {
        .sin_family = AF_INET,
        .sin_port = htons(53),
    };
    inet_pton(AF_INET, "1.1.1.1", &ipv4.sin_addr);
    connect_must_fail(AF_INET, &ipv4, sizeof(ipv4));

    struct sockaddr_in6 ipv6 = {
        .sin6_family = AF_INET6,
        .sin6_port = htons(53),
    };
    inet_pton(AF_INET6, "2606:4700:4700::1111", &ipv6.sin6_addr);
    connect_must_fail(AF_INET6, &ipv6, sizeof(ipv6));

    verify_no_default_routes();
    if (access("/etc/resolv.conf", F_OK) == 0) {
        fprintf(stderr, "resolver configuration visible\n");
        failures++;
    }
    return failures == 0 ? 0 : 1;
}
"#;

const RLIMIT_ENFORCEMENT_PROBE_SOURCE: &str = r#"
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int test_nofile(void) {
    int descriptors[512];
    int count = 0;
    while (count < 512) {
        int fd = open("/dev/null", O_RDONLY | O_CLOEXEC);
        if (fd < 0) {
            break;
        }
        descriptors[count++] = fd;
    }
    int open_error = errno;
    for (int index = 0; index < count; index++) {
        close(descriptors[index]);
    }
    if (open_error != EMFILE || count < 200 || count > 253) {
        fprintf(stderr, "nofile count=%d errno=%d\n", count, open_error);
        return 1;
    }
    return 0;
}

static int test_fsize(void) {
    signal(SIGXFSZ, SIG_IGN);
    int fd = open("large-output", O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    if (fd < 0) {
        return 2;
    }
    static unsigned char chunk[1024 * 1024];
    unsigned long long written = 0;
    int write_error = 0;
    while (written < 80ULL * 1024ULL * 1024ULL) {
        ssize_t result = write(fd, chunk, sizeof(chunk));
        if (result < 0) {
            write_error = errno;
            break;
        }
        written += (unsigned long long)result;
    }
    close(fd);
    struct stat metadata;
    if (stat("large-output", &metadata) != 0 || write_error != EFBIG ||
        written != 64ULL * 1024ULL * 1024ULL ||
        (unsigned long long)metadata.st_size != written) {
        fprintf(stderr, "fsize written=%llu size=%llu errno=%d\n", written,
                (unsigned long long)metadata.st_size, write_error);
        return 3;
    }
    return 0;
}

__attribute__((noinline)) static void consume_stack(unsigned int depth) {
    volatile unsigned char frame[65536];
    memset((void *)frame, (int)depth, sizeof(frame));
    consume_stack(depth + 1);
    if (frame[depth % sizeof(frame)] == 255) {
        _exit(99);
    }
}

int main(int argc, char **argv) {
    if (argc != 2) {
        return 64;
    }
    if (strcmp(argv[1], "nofile") == 0) {
        return test_nofile();
    }
    if (strcmp(argv[1], "fsize") == 0) {
        return test_fsize();
    }
    if (strcmp(argv[1], "stack") == 0) {
        consume_stack(1);
    }
    if (strcmp(argv[1], "core") == 0) {
        abort();
    }
    if (strcmp(argv[1], "cpu") == 0) {
        for (;;) {
            __asm__ volatile("" ::: "memory");
        }
    }
    return 65;
}
"#;

const MEMORY_PROBE_SOURCE: &str = r#"
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    const size_t allocation = 128UL * 1024UL * 1024UL;
    const long page_size = sysconf(_SC_PAGESIZE);
    volatile unsigned char *memory = malloc(allocation);
    if (memory == NULL) {
        return 2;
    }

    for (size_t offset = 0; offset < allocation; offset += (size_t)page_size) {
        memory[offset] = 1;
    }

    return 0;
}
"#;

fn lock_security_tests() -> MutexGuard<'static, ()> {
    SECURITY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn faber_cgroup_path() -> PathBuf {
    let membership = std::fs::read_to_string("/proc/self/cgroup")
        .expect("failed to read the test process cgroup membership");
    let relative_path = membership
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .expect("test process is not in a cgroup v2 hierarchy");
    let own_path = PathBuf::from("/sys/fs/cgroup").join(relative_path.trim_start_matches('/'));

    own_path
        .ancestors()
        .map(|ancestor| ancestor.join("faber"))
        .find(|candidate| candidate.is_dir())
        .expect("failed to locate the Faber cgroup")
}

fn container_roots() -> HashSet<PathBuf> {
    std::fs::read_dir("/tmp/faber")
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect()
}

fn assert_no_task_cgroups() {
    let leaked: Vec<PathBuf> = std::fs::read_dir(faber_cgroup_path())
        .expect("failed to inspect the Faber cgroup")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("task-"))
        })
        .collect();

    assert!(
        leaked.is_empty(),
        "task cgroups leaked after execution: {leaked:?}"
    );
}

fn task(cmd: &str, args: &[&str]) -> Task {
    Task {
        cmd: cmd.to_string(),
        args: Some(args.iter().map(|arg| (*arg).to_string()).collect()),
        env: None,
        stdin: None,
        files: None,
        working_dir: None,
        sandbox_profile: None,
    }
}

fn task_with_file(cmd: &str, args: &[&str], path: &str, content: &str) -> Task {
    let mut files = HashMap::new();
    files.insert(path.to_string(), content.to_string());

    Task {
        files: Some(files),
        ..task(cmd, args)
    }
}

fn execute(tasks: Vec<Task>) -> Vec<ExecutionStepResult> {
    let task_group = tasks.into_iter().map(ExecutionStep::Single).collect();
    let result = RuntimeBuilder::default()
        .with_task_group(task_group)
        .build()
        .execute()
        .expect("runtime execution failed");

    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();
    results
}

fn single_result(result: &ExecutionStepResult) -> &TaskResult {
    let ExecutionStepResult::Single(result) = result else {
        panic!("expected a single task result");
    };
    result
}

fn status_field<'a>(status: &'a str, name: &str) -> &'a str {
    status
        .lines()
        .find_map(|line| line.strip_prefix(name))
        .map(str::trim)
        .unwrap_or_else(|| panic!("missing {name} in /proc/self/status"))
}

fn mountinfo_line<'a>(mountinfo: &'a str, mountpoint: &str) -> &'a str {
    mountinfo
        .lines()
        .rfind(|line| line.split_whitespace().nth(4) == Some(mountpoint))
        .unwrap_or_else(|| panic!("missing {mountpoint} in mountinfo"))
}

fn namespace_inode(name: &str) -> u64 {
    std::fs::metadata(format!("/proc/self/ns/{name}"))
        .unwrap_or_else(|error| panic!("failed to inspect outer {name} namespace: {error}"))
        .ino()
}

#[test]
fn container_setup_failures_remove_partial_roots_and_cgroups() {
    let _guard = lock_security_tests();
    let roots_before = container_roots();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(task("/bin/true", &[]))])
        .with_container_config(
            ContainerConfigBuilder::new()
                .with_tmpdir_size("not-a-size".to_string())
                .build(),
        )
        .build()
        .execute()
        .expect("runtime controller failed");

    let RuntimeResult::ContainerSetupFailed { error } = result else {
        panic!("invalid mount unexpectedly produced a runtime: {result:?}");
    };
    assert!(error.contains("Container setup failed"));
    assert_eq!(
        container_roots(),
        roots_before,
        "partial container root leaked"
    );
    assert_no_task_cgroups();
}

#[test]
fn security_probe_records_identity_namespaces_mounts_and_limits() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file(
            "/usr/bin/gcc",
            &["security_probe.c", "-o", "security_probe"],
            "security_probe.c",
            SECURITY_PROBE_SOURCE,
        ),
        task("./security_probe", &[]),
    ]);

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("security probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(
        *compile_exit, 0,
        "security probe did not compile: {compile_stderr}; result: {:?}",
        results[0]
    );

    let TaskResult::Completed {
        stdout,
        stderr,
        exit_code,
        ..
    } = single_result(&results[1])
    else {
        panic!("security probe did not complete: {:?}", results[1]);
    };
    assert_eq!(*exit_code, 0, "security probe failed: {stderr}");

    let state: SecurityState =
        serde_json::from_str(stdout).expect("security probe emitted invalid JSON");
    assert!(
        state.pid > 1,
        "task replaced the namespace reaper: {state:?}"
    );
    assert!(state.ppid <= 1, "unexpected visible parent PID: {state:?}");
    assert_eq!((state.uid, state.euid), (65534, 65534));
    assert_eq!((state.gid, state.egid), (65534, 65534));

    for capability_set in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
        assert_eq!(
            status_field(&state.status, capability_set),
            "0000000000000000",
            "{capability_set} was not cleared"
        );
    }
    assert_eq!(status_field(&state.status, "NoNewPrivs:"), "1");
    assert_eq!(status_field(&state.status, "Seccomp:"), "2");

    for namespace in ["mnt", "pid", "net", "uts", "ipc", "user"] {
        let inner = state.namespaces[namespace];
        assert_ne!(
            inner,
            namespace_inode(namespace),
            "task did not enter a distinct {namespace} namespace"
        );
    }
    assert!(
        state.namespaces["cgroup"] > 0,
        "missing cgroup namespace evidence"
    );

    let uid_map: Vec<&str> = state.uid_map.split_whitespace().collect();
    let gid_map: Vec<&str> = state.gid_map.split_whitespace().collect();
    assert_eq!(
        uid_map,
        ["65534", "65534", "1"],
        "task UID map exposed an unexpected outer identity"
    );
    assert_eq!(
        gid_map,
        ["65534", "65534", "1"],
        "task GID map exposed an unexpected outer identity"
    );
    assert!(
        state.groups.is_empty(),
        "supplementary groups were not cleared: {:?}",
        state.groups
    );
    assert!(
        state.cgroup.contains("task-"),
        "unexpected cgroup: {}",
        state.cgroup
    );
    assert!(!state.mountinfo.contains("/oldroot"));

    let root_mount = mountinfo_line(&state.mountinfo, "/");
    let root_optional_fields = root_mount
        .split(" - ")
        .next()
        .expect("root mountinfo lacked a separator");
    assert!(!root_optional_fields.contains("shared:"));
    assert!(!root_optional_fields.contains("master:"));

    let sys_mount = mountinfo_line(&state.mountinfo, "/sys");
    let sys_options = sys_mount
        .split_whitespace()
        .nth(5)
        .expect("sysfs mount options were missing");
    assert!(
        sys_options.split(',').any(|option| option == "ro"),
        "sysfs was not read-only: {sys_mount}"
    );
    assert!(
        state
            .mountinfo
            .lines()
            .all(|line| line.split_whitespace().nth(4) != Some("/sys/fs/cgroup")),
        "task unexpectedly retained a cgroup filesystem mount"
    );
    for mountpoint in ["/faber", "/tmp"] {
        let mount = mountinfo_line(&state.mountinfo, mountpoint);
        let options = mount
            .split_whitespace()
            .nth(5)
            .unwrap_or_else(|| panic!("{mountpoint} mount options were missing"));
        assert!(
            options.split(',').any(|option| option == "nodev"),
            "{mountpoint} allowed device access: {mount}"
        );
        assert!(
            options.split(',').any(|option| option == "nosuid"),
            "{mountpoint} allowed set-ID execution: {mount}"
        );
    }

    let _ = (&state.route4, &state.route6);

    for resource in ["cpu", "fsize", "nofile", "nproc", "stack", "core"] {
        let limit = state
            .rlimits
            .get(resource)
            .unwrap_or_else(|| panic!("missing {resource} rlimit evidence"));
        assert!(
            limit.soft <= limit.hard,
            "invalid {resource} rlimit: {limit:?}"
        );
    }
    for (resource, expected) in [
        ("cpu", 5),
        ("fsize", 64 * 1024 * 1024),
        ("nofile", 256),
        ("stack", 8 * 1024 * 1024),
        ("core", 0),
    ] {
        let limit = &state.rlimits[resource];
        assert_eq!((limit.soft, limit.hard), (expected, expected));
    }
}

#[test]
fn network_routes_dns_and_external_sockets_are_isolated() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file(
            "/usr/bin/gcc",
            &["network_escape_probe.c", "-o", "network_escape_probe"],
            "network_escape_probe.c",
            NETWORK_ESCAPE_PROBE_SOURCE,
        ),
        task("./network_escape_probe", &[]),
        task("/usr/bin/readlink", &["/proc/self/ns/net"]),
    ]);

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("network probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(*compile_exit, 0, "network probe: {compile_stderr}");
    let TaskResult::Completed {
        exit_code,
        stderr,
        stats,
        ..
    } = single_result(&results[1])
    else {
        panic!("network probe produced no result: {:?}", results[1]);
    };
    assert_eq!(*exit_code, 0, "network escape succeeded: {stderr}");
    assert_eq!(stats.outcome, TaskOutcome::Exited);

    let TaskResult::Completed {
        stdout: first_namespace,
        ..
    } = single_result(&results[2])
    else {
        panic!("network namespace probe failed: {:?}", results[2]);
    };
    let second_results = execute(vec![task("/usr/bin/readlink", &["/proc/self/ns/net"])]);
    let TaskResult::Completed {
        stdout: second_namespace,
        exit_code: second_exit,
        ..
    } = single_result(&second_results[0])
    else {
        panic!(
            "second network namespace probe failed: {:?}",
            second_results[0]
        );
    };
    assert_eq!(*second_exit, 0);
    assert_ne!(
        first_namespace.trim(),
        second_namespace.trim(),
        "independent runtimes reused a network namespace"
    );
}

#[test]
fn identity_procfs_and_descriptor_escape_attempts_fail() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file(
            "/usr/bin/gcc",
            &["privilege_escape_probe.c", "-o", "privilege_escape_probe"],
            "privilege_escape_probe.c",
            PRIVILEGE_ESCAPE_PROBE_SOURCE,
        ),
        task("./privilege_escape_probe", &[]),
        task("/bin/kill", &["-0", "1"]),
    ]);

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("privilege probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(*compile_exit, 0, "privilege probe: {compile_stderr}");

    let TaskResult::Completed {
        exit_code,
        stderr,
        stats,
        ..
    } = single_result(&results[1])
    else {
        panic!("privilege probe produced no result: {:?}", results[1]);
    };
    assert_eq!(*exit_code, 0, "privilege escape succeeded: {stderr}");
    assert_eq!(stats.outcome, TaskOutcome::Exited);
    assert!(stats.cleanup_succeeded);

    let TaskResult::Completed {
        exit_code: init_exit,
        ..
    } = single_result(&results[2])
    else {
        panic!("namespace init liveness probe failed: {:?}", results[2]);
    };
    assert_eq!(*init_exit, 1, "task could signal namespace PID 1");
}

#[test]
fn filesystem_hides_outer_root_and_keeps_toolchains_read_only() {
    let _guard = lock_security_tests();
    let marker_path = format!("/root/faber-host-marker-{}", std::process::id());
    std::fs::write(&marker_path, "must remain outside the sandbox")
        .expect("failed to create outer-root marker");

    let script = format!(
        "test ! -e {marker_path} && test ! -e /oldroot && \
         ! touch /bin/faber-write-test 2>/dev/null && \
         ! touch /usr/bin/faber-write-test 2>/dev/null && \
         test ! -w /sys/fs/cgroup/cgroup.procs"
    );
    let results = execute(vec![task("/bin/sh", &["-c", &script])]);
    std::fs::remove_file(&marker_path).expect("failed to remove outer-root marker");

    let TaskResult::Completed {
        exit_code, stderr, ..
    } = single_result(&results[0])
    else {
        panic!("filesystem probe did not complete: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0, "filesystem boundary probe failed: {stderr}");
}

#[test]
fn submitted_files_reject_absolute_and_parent_paths() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file("/bin/true", &[], "/tmp/absolute-escape", "blocked"),
        task_with_file("/bin/true", &[], "../parent-escape", "blocked"),
    ]);

    for result in results {
        let TaskResult::Failed { error, stats } = single_result(&result) else {
            panic!("unsafe task file path was accepted: {result:?}");
        };
        assert_eq!(stats.outcome, TaskOutcome::InfrastructureFailure);
        assert!(
            error.contains("paths must be normalized and relative"),
            "unexpected path rejection: {error}"
        );
    }
}

#[test]
fn submitted_files_do_not_follow_workspace_symlinks() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task("/bin/ln", &["-s", "/tmp", "escape"]),
        task_with_file("/bin/true", &[], "escape/escaped.txt", "blocked"),
    ]);

    let TaskResult::Completed { exit_code, .. } = single_result(&results[0]) else {
        panic!("failed to create the adversarial symlink: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0);

    let TaskResult::Failed { error, .. } = single_result(&results[1]) else {
        panic!("workspace symlink was followed: {:?}", results[1]);
    };
    assert!(
        error.contains("without following links"),
        "unexpected symlink rejection: {error}"
    );
}

#[test]
fn submitted_files_reject_every_non_regular_target_without_blocking() {
    let _guard = lock_security_tests();
    let started = std::time::Instant::now();
    let mut tasks = vec![
        task_with_file(
            "/usr/bin/gcc",
            &["filesystem_object_probe.c", "-o", "filesystem_object_probe"],
            "filesystem_object_probe.c",
            FILESYSTEM_OBJECT_PROBE_SOURCE,
        ),
        task("./filesystem_object_probe", &[]),
    ];
    for target in [
        "object-directory",
        "object-fifo",
        "object-socket",
        "object-magic-link",
    ] {
        tasks.push(task_with_file("/bin/true", &[], target, "blocked"));
    }
    tasks.push(task("/bin/true", &[]));

    let results = execute(tasks);
    for index in 0..2 {
        let TaskResult::Completed {
            exit_code, stderr, ..
        } = single_result(&results[index])
        else {
            panic!("filesystem object setup failed: {:?}", results[index]);
        };
        assert_eq!(*exit_code, 0, "filesystem object setup: {stderr}");
    }
    for result in &results[2..6] {
        let TaskResult::Failed { .. } = single_result(result) else {
            panic!("non-regular task file target was accepted: {result:?}");
        };
    }
    let TaskResult::Completed { exit_code, .. } = single_result(&results[6]) else {
        panic!(
            "runtime did not recover after object rejection: {:?}",
            results[6]
        );
    };
    assert_eq!(*exit_code, 0);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "FIFO target blocked task materialization"
    );
}

#[test]
fn parallel_symlink_swaps_cannot_redirect_submitted_files() {
    let _guard = lock_security_tests();
    let mut parallel_tasks = vec![task(
        "/bin/sh",
        &[
            "-c",
            "i=0; while test $i -lt 1000; do rm -rf race; ln -s /tmp race; rm -f race; mkdir race 2>/dev/null || true; i=$((i+1)); done",
        ],
    )];
    for index in 0..8 {
        parallel_tasks.push(task_with_file(
            "/bin/true",
            &[],
            &format!("race/payload-{index}"),
            &"x".repeat(1024 * 1024),
        ));
    }

    let result = RuntimeBuilder::default()
        .with_task_group(vec![
            ExecutionStep::Single(task("/bin/mkdir", &["race"])),
            ExecutionStep::Parallel(parallel_tasks),
            ExecutionStep::Single(task(
                "/bin/sh",
                &[
                    "-c",
                    "set -- /tmp/payload-*; test \"$1\" = '/tmp/payload-*'",
                ],
            )),
        ])
        .with_timeout(std::time::Duration::from_secs(5))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed { exit_code, .. } = single_result(&results[0]) else {
        panic!("race directory setup failed: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0);
    let ExecutionStepResult::Parallel(parallel_results) = &results[1] else {
        panic!("expected parallel race results");
    };
    assert_eq!(parallel_results.len(), 9);
    for (index, result) in parallel_results.iter().enumerate() {
        match result {
            TaskResult::Completed {
                exit_code, stats, ..
            } if index == 0 => assert!(
                *exit_code == 0 || stats.outcome == TaskOutcome::TimedOut,
                "symlink swapper failed unexpectedly: {result:?}"
            ),
            TaskResult::Completed { exit_code, .. } => assert_eq!(*exit_code, 0),
            TaskResult::Failed { error, .. } => assert!(
                error.contains("without following links")
                    || error.contains("Failed to write task file"),
                "unexpected race rejection: {error}"
            ),
        }
    }
    let TaskResult::Completed {
        exit_code: escape_check,
        stderr,
        ..
    } = single_result(&results[2])
    else {
        panic!("race escape check failed: {:?}", results[2]);
    };
    assert_eq!(
        *escape_check, 0,
        "submitted file escaped into /tmp: {stderr}"
    );
}

#[test]
fn output_streams_are_drained_bounded_and_report_truncation() {
    let _guard = lock_security_tests();
    const OUTPUT_LIMIT: usize = 4096;

    for (command, args, stdout_should_truncate) in [
        ("/usr/bin/yes", vec!["stdout"], true),
        ("/bin/sh", vec!["-c", "exec /usr/bin/yes stderr >&2"], false),
    ] {
        let result = RuntimeBuilder::default()
            .with_task_group(vec![ExecutionStep::Single(task(command, &args))])
            .with_output_limit(OUTPUT_LIMIT)
            .with_timeout(std::time::Duration::from_secs(2))
            .build()
            .execute()
            .expect("runtime execution failed");
        let RuntimeResult::Success(results) = result else {
            panic!("container setup failed: {result:?}");
        };
        assert_no_task_cgroups();

        let TaskResult::Completed {
            stdout,
            stderr,
            exit_code,
            stats,
        } = single_result(&results[0])
        else {
            panic!("output flood did not produce a result: {:?}", results[0]);
        };

        assert_eq!(*exit_code, 137, "output-limited task was not killed");
        assert!(stdout.len() <= OUTPUT_LIMIT);
        assert!(stderr.len() <= OUTPUT_LIMIT);
        assert_eq!(stats.stdout_truncated, stdout_should_truncate);
        assert_eq!(stats.stderr_truncated, !stdout_should_truncate);
        assert_eq!(stats.outcome, TaskOutcome::OutputLimit);
        assert_eq!(stats.termination_signal, Some(9));
        assert!(stats.cleanup_succeeded);
    }
}

#[test]
fn stdin_and_stdout_progress_concurrently_without_pipe_deadlock() {
    let _guard = lock_security_tests();
    let input = "x".repeat(256 * 1024);
    let mut cat_task = task("/bin/cat", &[]);
    cat_task.stdin = Some(input.clone());

    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(cat_task)])
        .with_output_limit(input.len())
        .with_timeout(std::time::Duration::from_secs(2))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        stdout,
        exit_code,
        stats,
        ..
    } = single_result(&results[0])
    else {
        panic!("cat did not produce a result: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0);
    assert_eq!(stdout, &input);
    assert!(!stats.stdout_truncated);
}

#[test]
fn parallel_large_results_do_not_deadlock_result_transport() {
    let _guard = lock_security_tests();
    const OUTPUT_SIZE: usize = 128 * 1024;

    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Parallel(vec![
            task("/usr/bin/head", &["-c", "131072", "/dev/zero"]),
            task("/usr/bin/head", &["-c", "131072", "/dev/zero"]),
        ])])
        .with_output_limit(OUTPUT_SIZE * 2)
        .with_timeout(std::time::Duration::from_secs(2))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let ExecutionStepResult::Parallel(results) = &results[0] else {
        panic!("expected parallel task results");
    };
    assert_eq!(results.len(), 2);
    for result in results {
        let TaskResult::Completed {
            stdout,
            exit_code,
            stats,
            ..
        } = result
        else {
            panic!("large parallel task failed: {result:?}");
        };
        assert_eq!(*exit_code, 0);
        assert_eq!(stdout.len(), OUTPUT_SIZE);
        assert!(!stats.stdout_truncated);
    }
}

#[test]
fn concurrent_tasks_use_distinct_cgroups_and_cleanup_all_of_them() {
    let _guard = lock_security_tests();
    let parallel_tasks = (0..8)
        .map(|_| task("/bin/sh", &["-c", "cat /proc/self/cgroup; sleep 0.05"]))
        .collect();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Parallel(parallel_tasks)])
        .with_timeout(std::time::Duration::from_secs(2))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let ExecutionStepResult::Parallel(results) = &results[0] else {
        panic!("expected parallel task results");
    };
    let mut memberships = HashSet::new();
    for result in results {
        let TaskResult::Completed {
            stdout,
            exit_code,
            stats,
            ..
        } = result
        else {
            panic!("concurrent task failed: {result:?}");
        };
        assert_eq!(*exit_code, 0);
        assert_eq!(stats.outcome, TaskOutcome::Exited);
        assert!(stats.cleanup_succeeded);
        assert!(stdout.contains("task-"), "unexpected membership: {stdout}");
        memberships.insert(stdout.trim().to_string());
    }
    assert_eq!(memberships.len(), 8, "parallel tasks shared task cgroups");
}

#[test]
fn namespace_init_reaps_orphaned_task_descendants() {
    let _guard = lock_security_tests();
    let results = execute(vec![
        task_with_file(
            "/usr/bin/gcc",
            &["orphan_probe.c", "-o", "orphan_probe"],
            "orphan_probe.c",
            ORPHAN_PROBE_SOURCE,
        ),
        task("./orphan_probe", &[]),
        task(
            "/bin/sh",
            &[
                "-c",
                "sleep 0.1; pid=$(cat orphan.pid); test ! -e /proc/$pid",
            ],
        ),
    ]);

    for (index, result) in results.iter().enumerate() {
        let TaskResult::Completed {
            exit_code, stderr, ..
        } = single_result(result)
        else {
            panic!("orphan lifecycle step {index} failed: {result:?}");
        };
        assert_eq!(*exit_code, 0, "orphan lifecycle step {index}: {stderr}");
    }
}

#[test]
fn every_seccomp_profile_rule_reports_a_policy_violation() {
    let _guard = lock_security_tests();
    const COMMON_BLOCKED: &[&str] = &[
        "acct",
        "add_key",
        "bpf",
        "delete_module",
        "fanotify_init",
        "finit_module",
        "init_module",
        "io_uring_setup",
        "kcmp",
        "kexec_load",
        "keyctl",
        "mount",
        "open_by_handle_at",
        "perf_event_open",
        "pivot_root",
        "process_vm_readv",
        "process_vm_writev",
        "ptrace",
        "quotactl",
        "reboot",
        "request_key",
        "setns",
        "swapoff",
        "swapon",
        "umount2",
        "unshare",
        "userfaultfd",
    ];
    const NATIVE_ONLY_BLOCKED: &[&str] =
        &["clone", "clone3", "fork", "socket", "socketpair", "vfork"];

    let mut tasks = vec![task_with_file(
        "/usr/bin/gcc",
        &["seccomp_probe.c", "-o", "seccomp_probe"],
        "seccomp_probe.c",
        SECCOMP_PROBE_SOURCE,
    )];
    let mut expected = Vec::new();
    for (profile, syscall) in COMMON_BLOCKED
        .iter()
        .map(|syscall| (SandboxProfile::CompileV1, *syscall))
        .chain(
            COMMON_BLOCKED
                .iter()
                .chain(NATIVE_ONLY_BLOCKED)
                .map(|syscall| (SandboxProfile::NativeV1, *syscall)),
        )
    {
        let mut probe = task("./seccomp_probe", &[syscall]);
        probe.sandbox_profile = Some(profile);
        tasks.push(probe);
        expected.push((profile, syscall));
    }

    let results = execute(tasks);
    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("seccomp probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(*compile_exit, 0, "seccomp probe: {compile_stderr}");

    for (result, (profile, syscall)) in results.iter().skip(1).zip(expected) {
        let TaskResult::Completed {
            exit_code, stats, ..
        } = single_result(result)
        else {
            panic!("{profile:?} {syscall} produced no result: {result:?}");
        };
        assert_eq!(*exit_code, 128 + libc::SIGSYS, "{profile:?} {syscall}");
        assert_eq!(
            stats.outcome,
            TaskOutcome::PolicyViolation,
            "{profile:?} {syscall}"
        );
        assert_eq!(stats.termination_signal, Some(libc::SIGSYS));
        assert!(stats.cleanup_succeeded);
    }
}

#[test]
fn signal_termination_is_reported_explicitly() {
    let _guard = lock_security_tests();
    let results = execute(vec![task("/bin/sh", &["-c", "kill -TERM $$"])]);

    let TaskResult::Completed {
        exit_code, stats, ..
    } = single_result(&results[0])
    else {
        panic!(
            "signal-terminated task produced no result: {:?}",
            results[0]
        );
    };
    assert_eq!(*exit_code, 143);
    assert_eq!(stats.outcome, TaskOutcome::Signaled);
    assert_eq!(stats.termination_signal, Some(15));
    assert!(stats.cleanup_succeeded);
}

#[test]
fn timeout_kills_the_complete_task_process_tree() {
    let _guard = lock_security_tests();
    let started = std::time::Instant::now();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(task(
            "/bin/sh",
            &["-c", "sleep 30 & wait"],
        ))])
        .with_timeout(std::time::Duration::from_millis(150))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        exit_code, stats, ..
    } = single_result(&results[0])
    else {
        panic!(
            "timed-out process tree produced no result: {:?}",
            results[0]
        );
    };
    assert_eq!(*exit_code, 137);
    assert_eq!(stats.outcome, TaskOutcome::TimedOut);
    assert_eq!(stats.termination_signal, Some(9));
    assert!(stats.cleanup_succeeded);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "task process tree was not terminated promptly"
    );
}

#[test]
fn file_descriptor_file_size_stack_core_and_cpu_rlimits_are_enforced() {
    let _guard = lock_security_tests();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![
            ExecutionStep::Single(task_with_file(
                "/usr/bin/gcc",
                &["-O0", "rlimit_probe.c", "-o", "rlimit_probe"],
                "rlimit_probe.c",
                RLIMIT_ENFORCEMENT_PROBE_SOURCE,
            )),
            ExecutionStep::Single(task("./rlimit_probe", &["nofile"])),
            ExecutionStep::Single(task("./rlimit_probe", &["fsize"])),
            ExecutionStep::Single(task("./rlimit_probe", &["stack"])),
            ExecutionStep::Single(task("./rlimit_probe", &["core"])),
            ExecutionStep::Single(task(
                "/bin/sh",
                &[
                    "-c",
                    "test ! -e core; set -- core.*; test \"$1\" = 'core.*'",
                ],
            )),
            ExecutionStep::Single(task("./rlimit_probe", &["cpu"])),
        ])
        .with_timeout(std::time::Duration::from_millis(2500))
        .with_cpu_time_limit(std::time::Duration::from_secs(1))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    for index in [0, 1, 2, 5] {
        let TaskResult::Completed {
            exit_code, stderr, ..
        } = single_result(&results[index])
        else {
            panic!("rlimit step {index} failed: {:?}", results[index]);
        };
        assert_eq!(*exit_code, 0, "rlimit step {index}: {stderr}");
    }
    for (index, signal) in [(3, libc::SIGSEGV), (4, libc::SIGABRT), (6, libc::SIGKILL)] {
        let TaskResult::Completed {
            exit_code, stats, ..
        } = single_result(&results[index])
        else {
            panic!("rlimit signal step {index} failed: {:?}", results[index]);
        };
        assert_eq!(*exit_code, 128 + signal, "rlimit step {index}");
        assert_eq!(stats.outcome, TaskOutcome::Signaled, "rlimit step {index}");
        assert_eq!(stats.termination_signal, Some(signal));
        assert!(stats.cleanup_succeeded);
    }
    let TaskResult::Completed { stats, .. } = single_result(&results[6]) else {
        unreachable!();
    };
    assert!(
        stats.execution_time_ms < 2500,
        "wall timeout fired before RLIMIT_CPU: {stats:?}"
    );
    assert!(
        stats.cpu_nr_throttled > 0,
        "cpu.max never throttled: {stats:?}"
    );
    assert!(
        stats.cpu_throttled_usec > 0,
        "cpu.stat reported no throttled time: {stats:?}"
    );
}

#[test]
fn pids_cgroup_enforces_the_process_limit() {
    let _guard = lock_security_tests();
    let task_group = vec![
        ExecutionStep::Single(task_with_file(
            "/usr/bin/gcc",
            &["pid_probe.c", "-o", "pid_probe"],
            "pid_probe.c",
            PID_PROBE_SOURCE,
        )),
        ExecutionStep::Single(task("./pid_probe", &[])),
    ];
    let result = RuntimeBuilder::default()
        .with_task_group(task_group)
        .with_cgroup_config(CgroupConfigBuilder::new().with_pids(8).build())
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("PID probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(
        *compile_exit, 0,
        "PID probe did not compile: {compile_stderr}"
    );

    let TaskResult::Completed {
        stdout,
        stderr,
        exit_code,
        stats,
    } = single_result(&results[1])
    else {
        panic!("PID probe did not complete: {:?}", results[1]);
    };
    assert_eq!(*exit_code, 0, "PID probe failed: {stderr}");

    let values: Vec<u32> = stdout
        .split_whitespace()
        .map(|value| value.parse().expect("PID probe emitted a non-number"))
        .collect();
    assert_eq!(values.len(), 2, "unexpected PID probe output: {stdout}");
    assert_eq!(values[1], 11, "fork should fail with EAGAIN: {stdout}");
    assert!(values[0] < 32, "all child processes were created: {stdout}");
    assert!(
        stats.pids_peak <= 8,
        "pids.peak exceeded pids.max: {:?}",
        stats
    );
    assert_eq!(
        stats.pids_peak, 8,
        "the configured PID ceiling was not reached"
    );
    assert_eq!(stats.outcome, TaskOutcome::PidsLimit);
    assert!(stats.pids_limit_hit_count > 0);
    assert!(stats.cleanup_succeeded);
}

#[test]
fn memory_cgroup_kills_a_process_that_exceeds_memory_max() {
    let _guard = lock_security_tests();
    const MEMORY_LIMIT: u64 = 64 * 1024 * 1024;

    let task_group = vec![
        ExecutionStep::Single(task_with_file(
            "/usr/bin/gcc",
            &["memory_probe.c", "-o", "memory_probe"],
            "memory_probe.c",
            MEMORY_PROBE_SOURCE,
        )),
        ExecutionStep::Single(task("./memory_probe", &[])),
    ];
    let result = RuntimeBuilder::default()
        .with_task_group(task_group)
        .with_cgroup_config(
            CgroupConfigBuilder::new()
                .with_memory(MEMORY_LIMIT.to_string())
                .build(),
        )
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let TaskResult::Completed {
        exit_code: compile_exit,
        stderr: compile_stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("memory probe compilation failed: {:?}", results[0]);
    };
    assert_eq!(
        *compile_exit, 0,
        "memory probe did not compile within its cgroup: {compile_stderr}"
    );

    let TaskResult::Completed {
        exit_code, stats, ..
    } = single_result(&results[1])
    else {
        panic!("memory probe did not produce a result: {:?}", results[1]);
    };
    assert_eq!(
        *exit_code, 137,
        "expected the kernel OOM kill signal, got stats: {stats:?}"
    );
    assert_eq!(stats.outcome, TaskOutcome::OutOfMemory);
    assert_eq!(stats.termination_signal, Some(9));
    assert!(stats.oom_kill_count > 0);
    assert!(stats.cleanup_succeeded);
    assert!(
        stats.memory_peak_bytes >= MEMORY_LIMIT / 2,
        "memory limit was not approached: {:?}",
        stats
    );
    assert!(
        stats.memory_peak_bytes <= MEMORY_LIMIT + 8 * 1024 * 1024,
        "memory.peak substantially exceeded memory.max: {:?}",
        stats
    );
}
