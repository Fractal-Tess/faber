use faber_core::{FaberError, Task};

pub fn validate_tasks(tasks: &[Task], max_tasks: usize) -> Result<(), FaberError> {
    if tasks.len() > max_tasks {
        return Err(FaberError::Validation(format!(
            "Too many tasks: {} (max: {})",
            tasks.len(),
            max_tasks
        )));
    }

    for (i, task) in tasks.iter().enumerate() {
        validate_task(task, i)?;
    }

    Ok(())
}

fn validate_task(task: &Task, index: usize) -> Result<(), FaberError> {
    // Validate command
    if task.command.is_empty() {
        return Err(FaberError::Validation(format!(
            "Task {}: command cannot be empty",
            index
        )));
    }

    if task.command.len() > 1024 {
        return Err(FaberError::Validation(format!(
            "Task {}: command too long ({} chars, max: 1024)",
            index,
            task.command.len()
        )));
    }

    // Validate arguments
    if let Some(args) = &task.args {
        for (arg_index, arg) in args.iter().enumerate() {
            if arg.len() > 1024 {
                return Err(FaberError::Validation(format!(
                    "Task {}: argument {} too long ({} chars, max: 1024)",
                    index,
                    arg_index,
                    arg.len()
                )));
            }
        }
    }

    // Validate environment variables
    if let Some(env) = &task.env {
        for (key, value) in env {
            if key.len() > 256 {
                return Err(FaberError::Validation(format!(
                    "Task {}: environment variable key too long ({} chars, max: 256)",
                    index,
                    key.len()
                )));
            }
            if value.len() > 4096 {
                return Err(FaberError::Validation(format!(
                    "Task {}: environment variable value too long ({} chars, max: 4096)",
                    index,
                    value.len()
                )));
            }
        }
    }

    // Validate files
    if let Some(files) = &task.files {
        if files.len() > 100 {
            return Err(FaberError::Validation(format!(
                "Task {}: too many files ({} files, max: 100)",
                index,
                files.len()
            )));
        }

        for (file_path, content) in files {
            if file_path.len() > 256 {
                return Err(FaberError::Validation(format!(
                    "Task {}: file path too long ({} chars, max: 256)",
                    index,
                    file_path.len()
                )));
            }
            if content.len() > 1024 * 1024 {
                // 1MB
                return Err(FaberError::Validation(format!(
                    "Task {}: file content too large ({} bytes, max: 1MB)",
                    index,
                    content.len()
                )));
            }
        }
    }

    // Check for dangerous commands
    validate_dangerous_commands(task, index)?;

    Ok(())
}

fn validate_dangerous_commands(task: &Task, index: usize) -> Result<(), FaberError> {
    let dangerous_commands = [
        "rm",
        "dd",
        "mkfs",
        "fdisk",
        "parted",
        "mount",
        "umount",
        "chroot",
        "sudo",
        "su",
        "passwd",
        "useradd",
        "userdel",
        "groupadd",
        "groupdel",
        "systemctl",
        "service",
        "init",
        "telinit",
        "reboot",
        "shutdown",
        "halt",
        "poweroff",
        "wall",
        "write",
        "mesg",
        "talk",
        "finger",
        "rsh",
        "rlogin",
        "rcp",
        "ftp",
        "tftp",
        "nc",
        "netcat",
        "socat",
        "ssh",
        "scp",
        "rsync",
        "wget",
        "curl",
        "lynx",
        "links",
        "elinks",
        "mail",
        "mailx",
        "mutt",
        "pine",
        "alpine",
        "nano",
        "vim",
        "vi",
        "emacs",
        "ed",
        "sed",
        "awk",
        "perl",
        "python",
        "python3",
        "ruby",
        "php",
        "node",
        "npm",
        "yarn",
        "pip",
        "gem",
        "cpan",
        "cpanm",
        "apt",
        "apt-get",
        "yum",
        "dnf",
        "pacman",
        "zypper",
        "brew",
        "docker",
        "podman",
        "lxc",
        "lxd",
        "rkt",
        "singularity",
        "kubectl",
        "oc",
        "helm",
        "kustomize",
        "skaffold",
        "git",
        "svn",
        "hg",
        "bzr",
        "cvs",
        "rsync",
        "tar",
        "zip",
        "unzip",
        "gzip",
        "bzip2",
        "xz",
        "7z",
        "chmod",
        "chown",
        "chgrp",
        "umask",
        "ulimit",
        "nice",
        "renice",
        "kill",
        "killall",
        "pkill",
        "pgrep",
        "pidof",
        "ps",
        "top",
        "htop",
        "iotop",
        "iotop-c",
        "iotop-p",
        "iotop-u",
        "iotop-d",
        "iotop-t",
        "strace",
        "ltrace",
        "gdb",
        "lldb",
        "valgrind",
        "perf",
        "oprofile",
        "tcpdump",
        "wireshark",
        "tshark",
        "nmap",
        "netstat",
        "ss",
        "lsof",
        "fuser",
        "lsof",
        "fuser",
        "lsof",
        "fuser",
        "lsof",
        "fuser",
    ];

    let command_lower = task.command.to_lowercase();
    for dangerous_cmd in &dangerous_commands {
        if command_lower == *dangerous_cmd
            || command_lower.starts_with(&format!("{} ", dangerous_cmd))
        {
            return Err(FaberError::Validation(format!(
                "Task {}: dangerous command '{}' is not allowed",
                index, task.command
            )));
        }
    }

    Ok(())
}
