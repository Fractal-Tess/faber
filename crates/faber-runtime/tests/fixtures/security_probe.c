#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

static char *read_file(const char *path) {
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) {
        return NULL;
    }

    size_t capacity = 4096;
    size_t length = 0;
    char *buffer = malloc(capacity);
    if (buffer == NULL) {
        close(fd);
        return NULL;
    }

    for (;;) {
        if (length + 2048 + 1 > capacity) {
            capacity *= 2;
            char *expanded = realloc(buffer, capacity);
            if (expanded == NULL) {
                free(buffer);
                close(fd);
                return NULL;
            }
            buffer = expanded;
        }

        ssize_t bytes = read(fd, buffer + length, capacity - length - 1);
        if (bytes < 0) {
            if (errno == EINTR) {
                continue;
            }
            free(buffer);
            close(fd);
            return NULL;
        }
        if (bytes == 0) {
            break;
        }
        length += (size_t)bytes;
    }

    close(fd);
    buffer[length] = '\0';
    return buffer;
}

static void print_json_string(const char *value) {
    putchar('"');
    if (value != NULL) {
        for (const unsigned char *cursor = (const unsigned char *)value; *cursor; cursor++) {
            switch (*cursor) {
                case '"':
                    fputs("\\\"", stdout);
                    break;
                case '\\':
                    fputs("\\\\", stdout);
                    break;
                case '\b':
                    fputs("\\b", stdout);
                    break;
                case '\f':
                    fputs("\\f", stdout);
                    break;
                case '\n':
                    fputs("\\n", stdout);
                    break;
                case '\r':
                    fputs("\\r", stdout);
                    break;
                case '\t':
                    fputs("\\t", stdout);
                    break;
                default:
                    if (*cursor < 0x20) {
                        printf("\\u%04x", *cursor);
                    } else {
                        putchar(*cursor);
                    }
            }
        }
    }
    putchar('"');
}

static void print_file_field(const char *name, const char *path, int trailing_comma) {
    char *contents = read_file(path);
    print_json_string(name);
    putchar(':');
    print_json_string(contents == NULL ? "" : contents);
    if (trailing_comma) {
        putchar(',');
    }
    free(contents);
}

static unsigned long long namespace_inode(const char *name) {
    char path[128];
    struct stat metadata;
    snprintf(path, sizeof(path), "/proc/self/ns/%s", name);
    if (stat(path, &metadata) != 0) {
        return 0;
    }
    return (unsigned long long)metadata.st_ino;
}

static void print_rlimit(const char *name, int resource, int trailing_comma) {
    struct rlimit limit;
    print_json_string(name);
    putchar(':');
    if (getrlimit(resource, &limit) != 0) {
        fputs("{\"soft\":0,\"hard\":0}", stdout);
    } else {
        printf(
            "{\"soft\":%llu,\"hard\":%llu}",
            (unsigned long long)limit.rlim_cur,
            (unsigned long long)limit.rlim_max
        );
    }
    if (trailing_comma) {
        putchar(',');
    }
}

int main(void) {
    int group_count = getgroups(0, NULL);
    gid_t *groups = NULL;
    if (group_count > 0) {
        groups = calloc((size_t)group_count, sizeof(gid_t));
        if (groups == NULL || getgroups(group_count, groups) < 0) {
            free(groups);
            groups = NULL;
            group_count = 0;
        }
    } else if (group_count < 0) {
        group_count = 0;
    }

    printf(
        "{\"pid\":%d,\"ppid\":%d,\"uid\":%u,\"euid\":%u,\"gid\":%u,\"egid\":%u,",
        getpid(),
        getppid(),
        getuid(),
        geteuid(),
        getgid(),
        getegid()
    );

    fputs("\"groups\":[", stdout);
    for (int index = 0; index < group_count; index++) {
        if (index > 0) {
            putchar(',');
        }
        printf("%u", groups[index]);
    }
    fputs("],", stdout);
    free(groups);

    const char *namespaces[] = {"mnt", "pid", "net", "uts", "ipc", "user", "cgroup"};
    fputs("\"namespaces\":{", stdout);
    for (size_t index = 0; index < sizeof(namespaces) / sizeof(namespaces[0]); index++) {
        if (index > 0) {
            putchar(',');
        }
        print_json_string(namespaces[index]);
        printf(":%llu", namespace_inode(namespaces[index]));
    }
    fputs("},", stdout);

    print_file_field("uid_map", "/proc/self/uid_map", 1);
    print_file_field("gid_map", "/proc/self/gid_map", 1);
    print_file_field("status", "/proc/self/status", 1);
    print_file_field("cgroup", "/proc/self/cgroup", 1);
    print_file_field("mountinfo", "/proc/self/mountinfo", 1);
    print_file_field("route4", "/proc/net/route", 1);
    print_file_field("route6", "/proc/net/ipv6_route", 1);

    fputs("\"rlimits\":{", stdout);
    print_rlimit("cpu", RLIMIT_CPU, 1);
    print_rlimit("fsize", RLIMIT_FSIZE, 1);
    print_rlimit("nofile", RLIMIT_NOFILE, 1);
    print_rlimit("nproc", RLIMIT_NPROC, 1);
    print_rlimit("stack", RLIMIT_STACK, 1);
    print_rlimit("core", RLIMIT_CORE, 0);
    fputs("}}\n", stdout);

    return ferror(stdout) ? 1 : 0;
}
