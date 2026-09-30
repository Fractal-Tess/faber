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

const STDOUT_HOLDER_PROBE_SOURCE: &str = r#"
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    for (;;) {
        pid_t child = fork();
        if (child < 0) {
            return 1;
        }
        if (child > 0) {
            _exit(0);
        }
        usleep(1000);
    }
}
"#;

const SECCOMP_PROBE_SOURCE: &str = r#"
#define _GNU_SOURCE
#include <errno.h>
#include <linux/netlink.h>
#include <sched.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <unistd.h>

#ifndef AF_VSOCK
#define AF_VSOCK 40
#endif

struct syscall_entry {
    const char *name;
    long number;
};

static void ignore_signal(int signal_number) {
    (void)signal_number;
}

int main(int argc, char **argv) {
    if (argc != 2) {
        return 64;
    }
    if (strcmp(argv[1], "clone_newuser") == 0) {
        syscall(SYS_clone, CLONE_NEWUSER | SIGCHLD, 0, 0, 0, 0);
        return 2;
    }
    if (strcmp(argv[1], "socket_vsock") == 0) {
        syscall(SYS_socket, AF_VSOCK, SOCK_STREAM, 0);
        return 2;
    }
    if (strcmp(argv[1], "socket_netlink_audit") == 0) {
        syscall(SYS_socket, AF_NETLINK, SOCK_RAW, NETLINK_AUDIT);
        return 2;
    }
    if (strcmp(argv[1], "handled_violation") == 0) {
        signal(SIGSYS, ignore_signal);
        syscall(SYS_unshare, CLONE_NEWNS);
        return 2;
    }
    if (strcmp(argv[1], "sockets_allowed") == 0) {
        const int sockets[][3] = {
            {AF_UNIX, SOCK_STREAM, 0},
            {AF_INET, SOCK_STREAM, 0},
            {AF_INET6, SOCK_DGRAM, 0},
            {AF_NETLINK, SOCK_RAW, NETLINK_ROUTE},
        };
        for (size_t index = 0; index < sizeof(sockets) / sizeof(sockets[0]); index++) {
            if (socket(sockets[index][0], sockets[index][1], sockets[index][2]) < 0) {
                return 4 + (int)index;
            }
        }
        return 0;
    }
    if (strcmp(argv[1], "clone3_enosys") == 0) {
        errno = 0;
        long result = syscall(SYS_clone3, 0, 0);
        return result == -1 && errno == ENOSYS ? 0 : 3;
    }
    if (strcmp(argv[1], "unlisted_enosys") == 0) {
        errno = 0;
        long result = syscall(SYS_vhangup);
        return result == -1 && errno == ENOSYS ? 0 : 3;
    }
#ifdef __x86_64__
    if (strcmp(argv[1], "x32") == 0) {
        syscall(SYS_getpid | 0x40000000UL);
        return 2;
    }
#endif
    const struct syscall_entry entries[] = {
        {"acct", SYS_acct},
        {"add_key", SYS_add_key},
        {"bpf", SYS_bpf},
        {"clone", SYS_clone},
        {"clone3", SYS_clone3},
        {"delete_module", SYS_delete_module},
        {"fanotify_init", SYS_fanotify_init},
        {"finit_module", SYS_finit_module},
        {"fsconfig", SYS_fsconfig},
        {"fsmount", SYS_fsmount},
        {"fsopen", SYS_fsopen},
        {"fork", SYS_fork},
        {"init_module", SYS_init_module},
        {"io_uring_setup", SYS_io_uring_setup},
        {"kcmp", SYS_kcmp},
        {"kexec_load", SYS_kexec_load},
        {"kexec_file_load", SYS_kexec_file_load},
        {"keyctl", SYS_keyctl},
        {"mount", SYS_mount},
        {"mount_setattr", SYS_mount_setattr},
        {"move_mount", SYS_move_mount},
        {"name_to_handle_at", SYS_name_to_handle_at},
        {"open_by_handle_at", SYS_open_by_handle_at},
        {"open_tree", SYS_open_tree},
        {"perf_event_open", SYS_perf_event_open},
        {"pivot_root", SYS_pivot_root},
        {"pidfd_getfd", SYS_pidfd_getfd},
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
        {"syslog", SYS_syslog},
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
    FILE *routes = fopen("/proc/self/net/route", "r");
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

    routes = fopen("/proc/self/net/ipv6_route", "r");
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

/// Host UIDs/GIDs the runtime leases to requests.
const SANDBOX_IDENTITIES: std::ops::Range<u32> = 100_000..165_536;

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

fn sandbox_cgroups() -> Vec<PathBuf> {
    std::fs::read_dir(faber_cgroup_path())
        .expect("failed to inspect the Faber cgroup")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("req-") || name.starts_with("task-")
            })
        })
        .collect()
}

fn assert_no_task_cgroups() {
    let leaked = sandbox_cgroups();
    assert!(
        leaked.is_empty(),
        "request or task cgroups leaked after execution: {leaked:?}"
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
fn overall_deadline_kills_only_the_expired_request() {
    let _guard = lock_security_tests();
    let expired = std::thread::spawn(|| {
        RuntimeBuilder::default()
            .with_task_group(
                (0..4)
                    .map(|_| ExecutionStep::Single(task("/bin/sleep", &["1"])))
                    .collect(),
            )
            .with_timeout(std::time::Duration::from_secs(5))
            .with_overall_timeout(std::time::Duration::from_millis(2500))
            .build()
            .execute()
    });

    // Start the second request shortly before the first one's deadline so
    // its task is running when the deadline handler fires.
    std::thread::sleep(std::time::Duration::from_millis(1800));
    let survivor = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Single(task(
            "/bin/sh",
            &["-c", "echo start; sleep 1.5; echo finished"],
        ))])
        .with_timeout(std::time::Duration::from_secs(5))
        .build()
        .execute()
        .expect("surviving request failed");
    let _ = expired.join().expect("expired request panicked");

    let RuntimeResult::Success(results) = survivor else {
        panic!("container setup failed: {survivor:?}");
    };
    let TaskResult::Completed {
        stdout,
        exit_code,
        stats,
        ..
    } = single_result(&results[0])
    else {
        panic!("surviving task failed: {:?}", results[0]);
    };
    assert_eq!(stats.outcome, TaskOutcome::Exited, "{stats:?}");
    assert_eq!(*exit_code, 0);
    assert_eq!(stdout, "start\nfinished\n");
    assert_no_task_cgroups();
}

#[test]
fn overall_deadline_returns_completed_steps_and_marks_the_rest() {
    let _guard = lock_security_tests();
    let started = std::time::Instant::now();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![
            ExecutionStep::Single(task("/bin/echo", &["first"])),
            ExecutionStep::Single(task("/bin/sleep", &["3"])),
            ExecutionStep::Parallel(vec![task("/bin/true", &[]), task("/bin/true", &[])]),
        ])
        .with_timeout(std::time::Duration::from_secs(5))
        .with_overall_timeout(std::time::Duration::from_millis(1500))
        .build()
        .execute()
        .expect("overall deadline discarded the completed steps");
    assert!(started.elapsed() < std::time::Duration::from_millis(2500));
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_eq!(results.len(), 3);

    let TaskResult::Completed { stdout, stats, .. } = single_result(&results[0]) else {
        panic!("first step failed: {:?}", results[0]);
    };
    assert_eq!(stdout, "first\n");
    assert_eq!(stats.outcome, TaskOutcome::Exited);

    let TaskResult::Completed { stats, .. } = single_result(&results[1]) else {
        panic!("second step failed: {:?}", results[1]);
    };
    assert_eq!(stats.outcome, TaskOutcome::TimedOut);

    let ExecutionStepResult::Parallel(unstarted) = &results[2] else {
        panic!("expected parallel results");
    };
    assert_eq!(unstarted.len(), 2);
    for result in unstarted {
        let TaskResult::Failed { error, stats } = result else {
            panic!("unstarted task ran: {result:?}");
        };
        assert!(error.contains("overall execution deadline"), "{error}");
        assert_eq!(stats.outcome, TaskOutcome::NotStarted);
    }
    assert_no_task_cgroups();
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
    // Inside, the task is 65534. Outside, it is the one identity leased to
    // this request, the same for its UID and GID.
    let outer_identity = match uid_map.as_slice() {
        ["65534", outer, "1"] => outer.parse::<u32>().expect("outer UID was not a number"),
        _ => panic!("task UID map was not a single 65534 entry: {uid_map:?}"),
    };
    assert!(
        SANDBOX_IDENTITIES.contains(&outer_identity),
        "task UID map exposed an unexpected outer identity: {uid_map:?}"
    );
    assert_eq!(gid_map, uid_map, "task GID map differs from its UID map");
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
        .expect("/sys mount options were missing");
    assert!(
        sys_options.split(',').any(|option| option == "ro"),
        "/sys was not read-only: {sys_mount}"
    );
    assert!(
        sys_mount.contains(" - tmpfs "),
        "/sys exposes something other than an empty tmpfs: {sys_mount}"
    );
    let proc_mount = mountinfo_line(&state.mountinfo, "/proc");
    assert!(
        proc_mount.contains("subset=pid"),
        "procfs exposes more than per-process entries: {proc_mount}"
    );
    assert!(
        state
            .mountinfo
            .lines()
            .all(|line| line.split_whitespace().nth(4) != Some("/sys/fs/cgroup")),
        "task unexpectedly retained a cgroup filesystem mount"
    );
    for mountpoint in ["/bin", "/lib", "/lib64", "/usr"] {
        let mount = mountinfo_line(&state.mountinfo, mountpoint);
        let options = mount
            .split_whitespace()
            .nth(5)
            .unwrap_or_else(|| panic!("{mountpoint} mount options were missing"));
        for required in ["ro", "nodev", "nosuid"] {
            assert!(
                options.split(',').any(|option| option == required),
                "{mountpoint} lacked {required}: {mount}"
            );
        }
    }
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
    assert!(
        first_namespace.trim().starts_with("net:["),
        "unexpected namespace probe output: {first_namespace}"
    );

    // Two runtimes that are alive at the same time must not share a network
    // namespace. (Sequential runtimes may legitimately see the same inode
    // number again: the kernel recycles it once the first namespace is gone.)
    let namespace_of_running_request = || {
        let result = RuntimeBuilder::default()
            .with_task_group(vec![ExecutionStep::Single(task(
                "/bin/sh",
                &["-c", "readlink /proc/self/ns/net; sleep 0.5"],
            ))])
            .build()
            .execute()
            .expect("runtime execution failed");
        let RuntimeResult::Success(results) = result else {
            panic!("container setup failed: {result:?}");
        };
        let TaskResult::Completed {
            stdout, exit_code, ..
        } = single_result(&results[0])
        else {
            panic!(
                "concurrent network namespace probe failed: {:?}",
                results[0]
            );
        };
        assert_eq!(*exit_code, 0);
        stdout.trim().to_string()
    };
    let first = std::thread::spawn(namespace_of_running_request);
    let second = std::thread::spawn(namespace_of_running_request);
    let first = first.join().expect("first runtime panicked");
    let second = second.join().expect("second runtime panicked");
    assert_ne!(
        first, second,
        "concurrent runtimes shared a network namespace"
    );
    assert_no_task_cgroups();
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
fn procfs_and_sysfs_expose_no_host_state() {
    let _guard = lock_security_tests();
    let script = "for file in cmdline meminfo stat loadavg interrupts version modules net sys; do \
                      test ! -e /proc/$file || { echo visible: /proc/$file; exit 1; }; \
                  done; \
                  test -r /proc/self/status && test -r /proc/self/net/route && \
                  test -z \"$(ls -A /sys)\" && \
                  ! touch /sys/faber-write-test 2>/dev/null && \
                  ! touch /proc/faber-write-test 2>/dev/null";
    let results = execute(vec![task("/bin/sh", &["-c", script])]);

    let TaskResult::Completed {
        exit_code,
        stdout,
        stderr,
        ..
    } = single_result(&results[0])
    else {
        panic!("procfs probe did not complete: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0, "stdout: {stdout} stderr: {stderr}");
}

/// Processes whose parent is `parent`, as (pid, real uid, command name).
fn child_processes(parent: u32) -> Vec<(u32, u32, String)> {
    std::fs::read_dir("/proc")
        .expect("failed to list /proc")
        .filter_map(|entry| {
            entry
                .ok()?
                .file_name()
                .to_string_lossy()
                .parse::<u32>()
                .ok()
        })
        .filter_map(|pid| {
            let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
            let field = |name: &str| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .and_then(|value| value.split_whitespace().next().map(str::to_string))
            };
            (field("PPid:")?.parse::<u32>().ok()? == parent).then(|| {
                Some((
                    pid,
                    field("Uid:")?.parse::<u32>().ok()?,
                    field("Name:").unwrap_or_default(),
                ))
            })?
        })
        .collect()
}

#[test]
fn supervisors_are_a_fresh_process_image_and_requests_own_their_user_namespace() {
    let _guard = lock_security_tests();
    // Stands in for the embedding service's listening and client sockets.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind a socket");

    let runtime = std::thread::spawn(|| {
        RuntimeBuilder::default()
            .with_task_group(vec![ExecutionStep::Single(task("/bin/sleep", &["1.5"]))])
            .build()
            .execute()
    });
    wait_for_request_cgroup();

    // This process, then the jailer, then the task's supervisor, then the
    // PID namespace init (root) and the task once it has called exec.
    let mut found = None;
    for _ in 0..200 {
        if let [(jailer, _, _)] = child_processes(std::process::id()).as_slice()
            && let [(supervisor, _, _)] = child_processes(*jailer).as_slice()
        {
            let below = child_processes(*supervisor);
            let init = below.iter().find(|(_, uid, _)| *uid == 0);
            let sleeper = below.iter().find(|(_, _, name)| name == "sleep");
            if let (Some(init), Some(sleeper)) = (init, sleeper) {
                found = Some((*jailer, *supervisor, init.0, sleeper.0, sleeper.1));
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let (jailer, supervisor, init, sleeper, sleeper_uid) =
        found.expect("sandbox processes never appeared");

    let sockets = |pid: u32| -> Vec<String> {
        std::fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap_or_else(|error| panic!("failed to list descriptors of {pid}: {error}"))
            .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
            .map(|target| target.to_string_lossy().into_owned())
            .filter(|target| target.starts_with("socket:"))
            .collect()
    };
    assert!(
        !sockets(std::process::id()).is_empty(),
        "the test process should hold its listener"
    );
    for process in [jailer, supervisor, init] {
        let inherited = sockets(process);
        assert!(
            inherited.is_empty(),
            "sandbox supervisor {process} holds sockets of the embedding process: {inherited:?}"
        );
        // Nothing of the embedding process's environment either: the jailer
        // is started with only its own marker variable.
        let environment = std::fs::read(format!("/proc/{process}/environ"))
            .unwrap_or_else(|error| panic!("failed to read environment of {process}: {error}"));
        assert_eq!(
            String::from_utf8_lossy(&environment),
            "__FABER_JAILER=1\0",
            "sandbox supervisor {process} inherited an environment"
        );
    }
    assert!(
        std::env::vars_os().count() > 1,
        "the test process should have an environment to leak"
    );

    // NS_GET_OWNER_UID: the user that created the namespace, which is the one
    // the kernel charges for the namespace's per-user resource use.
    const NS_GET_OWNER_UID: libc::c_ulong = 0xb704;
    let namespace = std::fs::File::open(format!("/proc/{sleeper}/ns/user"))
        .expect("failed to open the task user namespace");
    let mut owner: libc::uid_t = 0;
    let result = unsafe {
        libc::ioctl(
            std::os::fd::AsRawFd::as_raw_fd(&namespace),
            NS_GET_OWNER_UID as _,
            &mut owner,
        )
    };
    assert_eq!(result, 0, "{}", std::io::Error::last_os_error());
    assert!(
        SANDBOX_IDENTITIES.contains(&owner),
        "the task user namespace is owned by {owner}, not by a sandbox identity"
    );
    assert_eq!(
        owner, sleeper_uid,
        "the task does not run as the namespace owner"
    );

    let result = runtime
        .join()
        .expect("runtime panicked")
        .expect("runtime failed");
    assert!(matches!(result, RuntimeResult::Success(_)), "{result:?}");
    drop(listener);
    assert_no_task_cgroups();
}

#[test]
fn concurrent_requests_run_as_different_identities() {
    let _guard = lock_security_tests();
    let outer_identity = || {
        let result = RuntimeBuilder::default()
            .with_task_group(vec![ExecutionStep::Single(task(
                "/bin/sh",
                &["-c", "cat /proc/self/uid_map; sleep 0.5"],
            ))])
            .build()
            .execute()
            .expect("runtime execution failed");
        let RuntimeResult::Success(results) = result else {
            panic!("container setup failed: {result:?}");
        };
        let TaskResult::Completed { stdout, .. } = single_result(&results[0]) else {
            panic!("identity task failed: {:?}", results[0]);
        };
        stdout
            .split_whitespace()
            .nth(1)
            .and_then(|outer| outer.parse::<u32>().ok())
            .unwrap_or_else(|| panic!("unexpected UID map: {stdout}"))
    };

    let first = std::thread::spawn(outer_identity);
    let second = std::thread::spawn(outer_identity);
    let first = first.join().expect("first runtime panicked");
    let second = second.join().expect("second runtime panicked");

    assert!(SANDBOX_IDENTITIES.contains(&first) && SANDBOX_IDENTITIES.contains(&second));
    assert_ne!(first, second, "two running requests shared a host identity");
    assert_no_task_cgroups();
}

#[test]
fn parallel_tasks_cannot_see_or_signal_each_other() {
    let _guard = lock_security_tests();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Parallel(vec![
            task("/bin/sh", &["-c", "sleep 1; echo survived"]),
            task(
                "/bin/sh",
                &[
                    "-c",
                    "sleep 0.3; kill -9 -1 2>/dev/null; ls /proc | grep -c '^[0-9]'",
                ],
            ),
        ])])
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    let ExecutionStepResult::Parallel(parallel) = &results[0] else {
        panic!("expected parallel results");
    };

    let TaskResult::Completed {
        stdout,
        exit_code,
        stats,
        ..
    } = &parallel[0]
    else {
        panic!("victim task failed: {:?}", parallel[0]);
    };
    assert_eq!(stats.outcome, TaskOutcome::Exited, "{stats:?}");
    assert_eq!((*exit_code, stdout.as_str()), (0, "survived\n"));

    let TaskResult::Completed { stdout, .. } = &parallel[1] else {
        panic!("signalling task failed: {:?}", parallel[1]);
    };
    // The namespace init, the shell, and the pipeline's ls and grep.
    let visible: u32 = stdout
        .trim()
        .parse()
        .expect("process count was not a number");
    assert!(
        visible <= 4,
        "a task saw {visible} processes; its sibling's are visible"
    );
    assert_no_task_cgroups();
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
         ! touch /root-level-escape 2>/dev/null && \
         ! touch /dev/escape-device 2>/dev/null && \
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
    for result in results.iter().take(2) {
        let TaskResult::Completed {
            exit_code, stderr, ..
        } = single_result(result)
        else {
            panic!("filesystem object setup failed: {result:?}");
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
            "i=0; while test $i -lt 300; do rm -rf race; ln -s /tmp race; rm -f race; mkdir race 2>/dev/null || true; i=$((i+1)); done",
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

const DIRECTORY_SWAPPER_SOURCE: &str = r#"
#define _GNU_SOURCE
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#ifndef RENAME_EXCHANGE
#define RENAME_EXCHANGE (1 << 1)
#endif

static double now(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec / 1e9;
}

int main(void) {
    if (symlink("/tmp", "race-alt") != 0) {
        return 1;
    }
    double deadline = now() + 1.5;
    while (now() < deadline) {
        for (int i = 0; i < 1000; i++) {
            syscall(SYS_renameat2, AT_FDCWD, "race", AT_FDCWD, "race-alt", RENAME_EXCHANGE);
        }
    }
    struct stat st;
    if (lstat("race", &st) == 0 && S_ISLNK(st.st_mode)) {
        syscall(SYS_renameat2, AT_FDCWD, "race", AT_FDCWD, "race-alt", RENAME_EXCHANGE);
    }
    return 0;
}
"#;

#[test]
fn parallel_symlink_swaps_cannot_redirect_nested_directory_creation() {
    let _guard = lock_security_tests();
    // The swapper atomically exchanges the `race` directory with a symlink to
    // /tmp while other tasks materialize nested files beneath `race`. A walk
    // that resolves accumulated paths from the workspace root would create
    // directories in /tmp through the symlink.
    let mut parallel_tasks = vec![task("./directory_swapper", &[])];
    let content = "x".repeat(512 * 1024);
    for index in 0..12 {
        let files = (0..16)
            .map(|file| {
                (
                    format!("race/deep-{index}-{file}/deeper/payload"),
                    content.clone(),
                )
            })
            .collect();
        parallel_tasks.push(Task {
            files: Some(files),
            ..task("/bin/true", &[])
        });
    }

    let result = RuntimeBuilder::default()
        .with_task_group(vec![
            ExecutionStep::Single(task_with_file(
                "/bin/sh",
                &[
                    "-c",
                    "mkdir race && exec /usr/bin/gcc directory_swapper.c -o directory_swapper",
                ],
                "directory_swapper.c",
                DIRECTORY_SWAPPER_SOURCE,
            )),
            ExecutionStep::Parallel(parallel_tasks),
            ExecutionStep::Single(task(
                "/bin/sh",
                &["-c", "set -- /tmp/deep-*; test \"$1\" = '/tmp/deep-*'"],
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

    let TaskResult::Completed {
        exit_code, stderr, ..
    } = single_result(&results[0])
    else {
        panic!("swapper build failed: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 0, "{stderr}");
    let ExecutionStepResult::Parallel(parallel_results) = &results[1] else {
        panic!("expected parallel race results");
    };
    let TaskResult::Completed { exit_code, .. } = &parallel_results[0] else {
        panic!("swapper failed: {:?}", parallel_results[0]);
    };
    assert_eq!(*exit_code, 0);
    for result in &parallel_results[1..] {
        match result {
            TaskResult::Completed { exit_code, .. } => assert_eq!(*exit_code, 0),
            TaskResult::Failed { error, .. } => assert!(
                error.contains("without following links")
                    || error.contains("Failed to create task directory")
                    || error.contains("Failed to write task file"),
                "unexpected race rejection: {error}"
            ),
        }
    }
    let TaskResult::Completed {
        exit_code, stderr, ..
    } = single_result(&results[2])
    else {
        panic!("race escape check failed: {:?}", results[2]);
    };
    assert_eq!(
        *exit_code, 0,
        "a task directory was created through a swapped symlink in /tmp: {stderr}"
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
fn request_output_budget_is_shared_by_every_task() {
    let _guard = lock_security_tests();
    let flood = |bytes: usize| {
        task(
            "/bin/sh",
            &["-c", &format!("head -c {bytes} /dev/zero | tr '\\0' x")],
        )
    };
    let result = RuntimeBuilder::default()
        .with_task_group(vec![
            ExecutionStep::Single(flood(6000)),
            ExecutionStep::Single(flood(6000)),
            ExecutionStep::Single(task("/bin/echo", &["hi"])),
            ExecutionStep::Single(task("/bin/true", &[])),
        ])
        .with_output_limit(64 * 1024)
        .with_request_output_limit(10_000)
        .with_timeout(std::time::Duration::from_secs(2))
        .build()
        .execute()
        .expect("runtime execution failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let outcome = |index: usize| {
        let TaskResult::Completed { stdout, stats, .. } = single_result(&results[index]) else {
            panic!("step {index} failed: {:?}", results[index]);
        };
        (stdout.len(), stats.outcome.clone())
    };
    assert_eq!(outcome(0), (6000, TaskOutcome::Exited));
    assert_eq!(outcome(1), (4000, TaskOutcome::OutputLimit));
    assert_eq!(outcome(2), (0, TaskOutcome::OutputLimit));
    assert_eq!(outcome(3), (0, TaskOutcome::Exited));
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
fn sixteen_mebibyte_results_are_transported_quickly() {
    let _guard = lock_security_tests();
    const OUTPUT_SIZE: usize = 1024 * 1024;

    let tasks = (0..16)
        .map(|_| task("/bin/sh", &["-c", "head -c 1048576 /dev/zero | tr '\\0' x"]))
        .collect();
    let started = std::time::Instant::now();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![ExecutionStep::Parallel(tasks)])
        .with_output_limit(OUTPUT_SIZE)
        .with_timeout(std::time::Duration::from_secs(10))
        .build()
        .execute()
        .expect("runtime execution failed");
    let elapsed = started.elapsed();
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    assert_no_task_cgroups();

    let ExecutionStepResult::Parallel(results) = &results[0] else {
        panic!("expected parallel task results");
    };
    assert_eq!(results.len(), 16);
    let mut slowest_task_ms = 0;
    for result in results {
        let TaskResult::Completed {
            stdout,
            exit_code,
            stats,
            ..
        } = result
        else {
            panic!("flood task failed: {result:?}");
        };
        assert_eq!(*exit_code, 0);
        assert_eq!(stdout.len(), OUTPUT_SIZE);
        assert!(!stats.stdout_truncated);
        slowest_task_ms = slowest_task_ms.max(stats.execution_time_ms);
    }

    // Everything outside the tasks themselves: sandbox setup plus moving
    // 16 MiB of results from the task children to this process.
    let overhead = elapsed.saturating_sub(std::time::Duration::from_millis(slowest_task_ms));
    assert!(
        overhead < std::time::Duration::from_secs(1),
        "16 MiB result transport took {overhead:?} (total {elapsed:?})"
    );
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
        // The orphan outlives its parent inside this task: adopted by the
        // namespace init (PPid 1), and gone with the task cgroup afterwards,
        // which assert_no_task_cgroups() in execute() would otherwise catch.
        task(
            "/bin/sh",
            &[
                "-c",
                "./orphan_probe && sleep 0.1 && pid=$(cat orphan.pid) && \
                 grep -q '^PPid:.1$' /proc/$pid/status",
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
fn timeout_kills_fork_successors_that_hold_output_open() {
    let _guard = lock_security_tests();
    let compile = task_with_file(
        "/usr/bin/gcc",
        &["stdout_holder.c", "-o", "stdout_holder"],
        "stdout_holder.c",
        STDOUT_HOLDER_PROBE_SOURCE,
    );
    let run = task("./stdout_holder", &[]);
    let started = std::time::Instant::now();
    let result = RuntimeBuilder::default()
        .with_task_group(vec![
            ExecutionStep::Single(compile),
            ExecutionStep::Single(run),
        ])
        .with_timeout(std::time::Duration::from_secs(2))
        .with_overall_timeout(std::time::Duration::from_secs(5))
        .build()
        .execute()
        .expect("runtime execution failed");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    let TaskResult::Completed { stats, .. } = single_result(&results[1]) else {
        panic!("timeout probe failed: {:?}", results[1]);
    };
    assert_eq!(stats.outcome, TaskOutcome::TimedOut);
    assert!(stats.cleanup_succeeded);
    assert_no_task_cgroups();
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
        "fsconfig",
        "fsmount",
        "fsopen",
        "init_module",
        "io_uring_setup",
        "kcmp",
        "kexec_load",
        "kexec_file_load",
        "keyctl",
        "mount",
        "mount_setattr",
        "move_mount",
        "name_to_handle_at",
        "open_by_handle_at",
        "open_tree",
        "perf_event_open",
        "pivot_root",
        "pidfd_getfd",
        "process_vm_readv",
        "process_vm_writev",
        "ptrace",
        "quotactl",
        "reboot",
        "request_key",
        "setns",
        "swapoff",
        "swapon",
        "syslog",
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
    let mut deny = |profile: SandboxProfile, syscall: &'static str| {
        let mut probe = task("./seccomp_probe", &[syscall]);
        probe.sandbox_profile = Some(profile);
        tasks.push(probe);
        expected.push((profile, syscall));
    };
    for profile in [
        SandboxProfile::CompileV1,
        SandboxProfile::NativeV1,
        SandboxProfile::CompileV2,
        SandboxProfile::NativeV2,
    ] {
        for syscall in COMMON_BLOCKED {
            deny(profile, syscall);
        }
        if profile.allows_processes() {
            // Rules that depend on arguments, and a violation the task tries
            // to survive by handling SIGSYS.
            for syscall in [
                "clone_newuser",
                "socket_vsock",
                "socket_netlink_audit",
                "handled_violation",
            ] {
                deny(profile, syscall);
            }
            #[cfg(target_arch = "x86_64")]
            deny(profile, "x32");
        } else {
            for syscall in NATIVE_ONLY_BLOCKED {
                deny(profile, syscall);
            }
        }
    }
    // Calls that must keep working, last in the task list: the compile
    // profiles keep their sockets and the clone3 fallback, and the v2
    // allowlists answer an unlisted syscall with ENOSYS instead of a kill.
    const MUST_WORK: &[(SandboxProfile, &str)] = &[
        (SandboxProfile::CompileV1, "sockets_allowed"),
        (SandboxProfile::CompileV1, "clone3_enosys"),
        (SandboxProfile::CompileV2, "sockets_allowed"),
        (SandboxProfile::CompileV2, "clone3_enosys"),
        (SandboxProfile::CompileV2, "unlisted_enosys"),
        (SandboxProfile::NativeV2, "unlisted_enosys"),
    ];
    for (profile, probe_name) in MUST_WORK {
        let mut probe = task("./seccomp_probe", &[probe_name]);
        probe.sandbox_profile = Some(*profile);
        tasks.push(probe);
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

    let allowed_results = &results[results.len() - MUST_WORK.len()..];
    for (result, (profile, probe_name)) in allowed_results.iter().zip(MUST_WORK) {
        let TaskResult::Completed {
            exit_code, stats, ..
        } = single_result(result)
        else {
            panic!("{profile:?} {probe_name} probe produced no result");
        };
        assert_eq!(*exit_code, 0, "{profile:?} {probe_name}");
        assert_eq!(
            stats.outcome,
            TaskOutcome::Exited,
            "{profile:?} {probe_name}"
        );
    }
}

#[test]
fn pre_exec_failures_are_distinguished_from_program_exits() {
    let _guard = lock_security_tests();
    let missing_command = task("/nonexistent/command", &[]);
    let bad_working_dir = Task {
        working_dir: Some("/does-not-exist".to_string()),
        ..task("/bin/true", &[])
    };
    let nul_argument = task("/bin/echo", &["before\0after"]);
    let program_exit = task("/bin/sh", &["-c", "exit 127"]);
    let results = execute(vec![
        missing_command,
        bad_working_dir,
        nul_argument,
        program_exit,
    ]);

    let TaskResult::Completed {
        exit_code,
        stderr,
        stats,
        ..
    } = single_result(&results[0])
    else {
        panic!("missing command did not complete: {:?}", results[0]);
    };
    assert_eq!(*exit_code, 127);
    assert_eq!(stats.outcome, TaskOutcome::Exited);
    assert!(
        stderr.contains("failed to execute '/nonexistent/command'")
            && stderr.contains("No such file or directory"),
        "exec failure lacks its errno: {stderr:?}"
    );

    let TaskResult::Failed { error, stats } = single_result(&results[1]) else {
        panic!("bad working directory ran: {:?}", results[1]);
    };
    assert_eq!(stats.outcome, TaskOutcome::InfrastructureFailure);
    assert!(stats.cleanup_succeeded);
    assert!(
        error.contains("working directory '/does-not-exist'")
            && error.contains("No such file or directory"),
        "{error}"
    );

    let TaskResult::Failed { error, stats } = single_result(&results[2]) else {
        panic!("NUL argument ran: {:?}", results[2]);
    };
    assert_eq!(stats.outcome, TaskOutcome::InfrastructureFailure);
    assert!(error.contains("NUL byte"), "{error}");

    let TaskResult::Completed {
        exit_code, stderr, ..
    } = single_result(&results[3])
    else {
        panic!("program exit did not complete: {:?}", results[3]);
    };
    assert_eq!(*exit_code, 127);
    assert!(
        stderr.is_empty(),
        "program exit gained a message: {stderr:?}"
    );
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

/// Wait for this process's single live request cgroup to appear.
fn wait_for_request_cgroup() -> PathBuf {
    for _ in 0..200 {
        if let Some(request) = sandbox_cgroups().into_iter().find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("req-"))
        }) {
            return request;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("request cgroup never appeared");
}

#[test]
fn request_cgroup_limits_cover_its_widest_step() {
    let _guard = lock_security_tests();
    let runtime = std::thread::spawn(|| {
        RuntimeBuilder::default()
            .with_task_group(vec![
                ExecutionStep::Single(task("/bin/sleep", &["0.5"])),
                ExecutionStep::Parallel((0..3).map(|_| task("/bin/sleep", &["0.5"])).collect()),
            ])
            .with_cgroup_config(
                CgroupConfigBuilder::new()
                    .with_memory("32M".to_string())
                    .with_pids(16)
                    .build(),
            )
            .build()
            .execute()
    });

    let request = wait_for_request_cgroup();
    let read = |file: &str| {
        std::fs::read_to_string(request.join(file))
            .unwrap_or_else(|error| panic!("failed to read {file}: {error}"))
            .trim()
            .to_string()
    };
    // Three tasks at 32 MiB plus the supervisor allowance: 64 MiB, two copies
    // of the (here uncapped, so 1 GiB) request output, and the default
    // 128 MiB workspace and 128 MiB /tmp.
    const MIB: u64 = 1024 * 1024;
    let supervisor_allowance = 64 * MIB + 2 * 1024 * MIB + 256 * MIB;
    assert_eq!(
        read("memory.max"),
        (3 * 32 * MIB + supervisor_allowance).to_string()
    );
    assert_eq!(
        read("supervisor/memory.max"),
        supervisor_allowance.to_string()
    );
    // 16 per task plus its supervisor and init, plus the jailer.
    assert_eq!(read("pids.max"), "55");
    assert!(
        !std::fs::read_to_string(request.join("supervisor/cgroup.procs"))
            .expect("failed to read the supervisor cgroup")
            .trim()
            .is_empty(),
        "the jailer is not in the request's supervisor cgroup"
    );

    let result = runtime
        .join()
        .expect("runtime panicked")
        .expect("runtime failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    let ExecutionStepResult::Parallel(parallel) = &results[1] else {
        panic!("expected parallel results");
    };
    for result in parallel {
        let TaskResult::Completed { stats, .. } = result else {
            panic!("parallel task failed: {result:?}");
        };
        assert_eq!(stats.outcome, TaskOutcome::Exited);
    }
    assert_no_task_cgroups();
}

#[test]
fn oom_kills_from_an_ancestor_limit_are_reported_separately() {
    let _guard = lock_security_tests();
    let runtime = std::thread::spawn(|| {
        RuntimeBuilder::default()
            .with_task_group(vec![ExecutionStep::Single(task(
                "/bin/sh",
                &[
                    "-c",
                    "sleep 0.5; exec /bin/dd if=/dev/zero of=/faber/fill.bin bs=1M count=96",
                ],
            ))])
            .with_cgroup_config(
                CgroupConfigBuilder::new()
                    .with_memory("256M".to_string())
                    .build(),
            )
            .with_timeout(std::time::Duration::from_secs(5))
            .build()
            .execute()
    });

    // Lower the request's limit below what the task may use on its own, as
    // pressure from the service or container would.
    let request = wait_for_request_cgroup();
    std::fs::write(request.join("memory.max"), (16 * 1024 * 1024).to_string())
        .expect("failed to lower the request memory limit");

    let result = runtime
        .join()
        .expect("runtime panicked")
        .expect("runtime failed");
    let RuntimeResult::Success(results) = result else {
        panic!("container setup failed: {result:?}");
    };
    let TaskResult::Completed { stats, .. } = single_result(&results[0]) else {
        panic!("fill task failed: {:?}", results[0]);
    };
    assert_eq!(stats.outcome, TaskOutcome::AncestorOutOfMemory, "{stats:?}");
    assert!(stats.oom_kill_count > 0);
    assert_no_task_cgroups();
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
