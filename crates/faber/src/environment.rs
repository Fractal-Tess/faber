//! Container environment lifecycle helpers.
//!
//! This module encapsulates low-level operations for setting up an isolated
//! filesystem and basic devices using Linux namespaces and mount operations.
//! It exposes top-level functions that are invoked by higher-level types.

use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::CloneFlags,
    sys::stat::{Mode, SFlag, makedev, mknod},
    unistd::{Gid, Uid, pivot_root, setgid, sethostname, setuid},
};

use crate::{prelude::*, types::Mount};

use std::env::set_current_dir;
use std::path::{Path, PathBuf};
use std::{
    collections::HashMap,
    fs::{create_dir_all, remove_dir_all, write},
};

/// Cleans up the container environment by removing the container root directory.
pub(crate) fn cleanup(host_container_root: &Path) -> Result<()> {
    remove_dir_all(host_container_root).map_err(|source| Error::RemoveDir {
        path: host_container_root.to_path_buf(),
        source,
        details: "Failed to remove container root".to_string(),
    })?;
    Ok(())
}
/// Creates the container root directory.
pub(crate) fn create_container_root(host_container_root: &Path) -> Result<()> {
    create_dir_all(host_container_root).map_err(|source| Error::CreateDir {
        path: host_container_root.to_path_buf(),
        source,
        details: "Failed to create container root".to_string(),
    })?;
    Ok(())
}

/// Unshares namespaces to isolate the container from the host system.
pub(crate) fn unshare(flags: CloneFlags) -> Result<()> {
    nix::sched::unshare(flags).map_err(|source| Error::Unshare {
        flags,
        source,
        details: "Failed to unshare namespaces".to_string(),
    })?;
    Ok(())
}

/// Sets the hostname for the container.
pub(crate) fn set_container_hostname(hostname: &str) -> Result<()> {
    sethostname(hostname).map_err(|source| Error::SetHostname {
        hostname: hostname.to_string(),
        source,
        details: "Failed to set hostname".to_string(),
    })?;
    Ok(())
}

/// Sets up bind mounts for the container filesystem.
pub(crate) fn bind_mounts(host_container_root: &Path, mounts: &[Mount]) -> Result<()> {
    // Rebind `/` to make it private
    mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )
    .map_err(|e| Error::Mount {
        src: "None".to_string(),
        target: "/".to_string(),
        fstype: None,
        flags: MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        err: e,
        details: "Failed to mount /".to_string(),
    })?;

    for m in mounts.iter() {
        if !Path::new(&m.source).exists() {
            continue;
        }

        let target = format!(
            "{}/{target}",
            host_container_root.display(),
            target = m.target.strip_prefix("/").unwrap_or(&m.target).to_owned()
        );

        let flags = m
            .flags
            .iter()
            .fold(MsFlags::empty(), |acc, flag| acc | *flag);

        create_dir_all(&target).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&target),
            source,
            details: "Failed to create target directory".to_string(),
        })?;

        mount(
            Some(m.source.as_str()),
            target.as_str(),
            None::<&str>,
            flags,
            m.data.as_deref(),
        )
        .map_err(|e| Error::Mount {
            src: m.source.clone(),
            target: target.clone(),
            fstype: None,
            flags,
            err: e,
            details: "Failed to mount bind mount".to_string(),
        })?;
    }
    Ok(())
}

/// Creates the `/proc` filesystem (non-self).
pub(crate) fn create_proc() -> Result<()> {
    let proc_path = "/proc";
    let proc_fstype = "proc";
    let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

    create_dir_all(proc_path).map_err(|source| Error::CreateDir {
        path: PathBuf::from(proc_path),
        source,
        details: "Failed to create proc directory".to_string(),
    })?;

    mount(
        None::<&str>,
        proc_path,
        Some(proc_fstype),
        proc_flags,
        None::<&str>,
    )
    .map_err(|e| Error::Mount {
        src: "None".to_string(),
        target: proc_path.to_string(),
        fstype: Some(proc_fstype.to_string()),
        flags: proc_flags,
        err: e,
        details: "Failed to mount proc filesystem".to_string(),
    })?;

    Ok(())
}

/// Creates the `/sys` filesystem (non-self).
pub(crate) fn create_sys() -> Result<()> {
    let sys_target = "/sys";
    let sys_fstype = "sysfs";
    let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

    create_dir_all(sys_target).map_err(|source| Error::CreateDir {
        path: PathBuf::from(sys_target),
        source,
        details: "Failed to create sys directory".to_string(),
    })?;

    mount(
        None::<&str>,
        sys_target,
        Some(sys_fstype),
        sys_flags,
        None::<&str>,
    )
    .map_err(|e| Error::Mount {
        src: "None".to_string(),
        target: sys_target.to_string(),
        fstype: Some(sys_fstype.to_string()),
        flags: sys_flags,
        err: e,
        details: "Failed to mount sys filesystem".to_string(),
    })?;

    Ok(())
}

pub(crate) fn create_cgroup() -> Result<()> {
    let cgroup_path = std::path::PathBuf::from("/sys/fs/cgroup");
    let cgroup_path_str = cgroup_path.to_str().unwrap();
    let cgroup_fstype = "cgroup2";
    let cgroup_flags = MsFlags::MS_RELATIME;
    mount(
        None::<&str>,
        cgroup_path_str,
        Some(cgroup_fstype),
        cgroup_flags,
        None::<&str>,
    )
    .map_err(|e| Error::Mount {
        src: "None".to_string(),
        target: cgroup_path_str.to_string(),
        fstype: Some(cgroup_fstype.to_string()),
        flags: cgroup_flags,
        err: e,
        details: "Failed to mount cgroup2 filesystem".to_string(),
    })?;

    Ok(())
}

pub(crate) fn write_files(path: &Path, files: &HashMap<String, String>) -> Result<()> {
    create_dir_all(path).map_err(|source| Error::CreateDir {
        path: path.to_path_buf(),
        source,
        details: "Failed to create workdir".to_string(),
    })?;

    for (rel_path, contents) in files {
        let target = path.join(rel_path);
        if let Some(parent) = target.parent() {
            if !parent.exists() {
                create_dir_all(parent).map_err(|source| Error::CreateDir {
                    path: parent.to_path_buf(),
                    source,
                    details:
                        "Failed to create parent directory for file when writing files for task"
                            .to_string(),
                })?;
            }
        }

        write(&target, contents).map_err(|source| Error::WriteFile {
            path: target.clone(),
            bytes: contents.len(),
            source,
            details: "Failed to write file to workdir".to_string(),
        })?;
    }
    Ok(())
}

pub(crate) fn mask_mounts(mounts: &[&str]) -> Result<()> {
    let scratch = PathBuf::from("/.masked");
    create_dir_all(&scratch).map_err(|source| Error::CreateDir {
        path: scratch.clone(),
        source,
        details: "Failed to create mask scratch directory".to_string(),
    })?;
    let scratch_str = scratch.to_str().unwrap();

    for path in mounts {
        let target = PathBuf::from(path);
        let target_str = target.to_str().unwrap();

        umount2(target_str, MntFlags::MNT_DETACH | MntFlags::MNT_FORCE).map_err(|e| {
            Error::Umount {
                target: target_str.to_string(),
                flags: MntFlags::MNT_DETACH | MntFlags::MNT_FORCE,
                err: e,
                details: "Failed to unmount path".to_string(),
            }
        })?;

        mount(
            Some(scratch_str),
            target_str,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: scratch_str.to_string(),
            target: target_str.to_string(),
            fstype: None,
            flags: MsFlags::MS_BIND,
            err: e,
            details: "Failed to bind mount mask over target".to_string(),
        })?;

        mount(
            None::<&str>,
            target_str,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: "None".to_string(),
            target: target_str.to_string(),
            fstype: None,
            flags: MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
            err: e,
            details: "Failed to remount masked path readonly".to_string(),
        })?;
    }

    Ok(())
}

/// Creates and mounts a temporary filesystem at the specified target directory.
pub(crate) fn create_tmp_dir(target: &Path, tmp_size: &str) -> Result<()> {
    create_dir_all(target).map_err(|source| Error::CreateDir {
        path: target.to_path_buf(),
        source,
        details: "Failed to create tmp directory".to_string(),
    })?;

    let mount_options = format!("size={tmp_size},mode=1777");
    let target_str = target.to_str().ok_or_else(|| Error::Configuration {
        component: "tmp dir path".to_string(),
        details: "Target tmp path contains invalid UTF-8 characters".to_string(),
    })?;

    mount(
        Some("tmpfs"),
        target_str,
        Some("tmpfs"),
        MsFlags::empty(),
        Some(mount_options.as_str()),
    )
    .map_err(|e| Error::Mount {
        src: "tmpfs".to_string(),
        target: target_str.to_string(),
        fstype: Some("tmpfs".to_string()),
        flags: MsFlags::empty(),
        err: e,
        details: "Failed to mount tmp filesystem".to_string(),
    })?;

    Ok(())
}

/// Creates the working directory for user files within the container.
pub(crate) fn create_work_dir(work_dir: &Path, workdir_size: &str) -> Result<()> {
    create_dir_all(work_dir).map_err(|source| Error::CreateDir {
        path: work_dir.to_path_buf(),
        source,
        details: "Failed to create work directory".to_string(),
    })?;

    let mount_options = format!("size={workdir_size},mode=755");
    mount(
        Some("tmpfs"),
        work_dir.to_str().unwrap(),
        Some("tmpfs"),
        MsFlags::empty(),
        Some(mount_options.as_str()),
    )
    .map_err(|e| Error::Mount {
        src: "tmpfs".to_string(),
        target: work_dir.to_str().unwrap().to_string(),
        fstype: Some("tmpfs".to_string()),
        flags: MsFlags::empty(),
        err: e,
        details: "Failed to mount workdir filesystem".to_string(),
    })?;

    set_current_dir(work_dir).map_err(|source| Error::Chdir {
        path: work_dir.to_str().unwrap().to_string(),
        source,
        details: "Failed to change directory to workdir".to_string(),
    })?;

    Ok(())
}

/// Performs a pivot root operation to change the filesystem root.
pub(crate) fn pivot_root_to(host_container_root: &Path) -> Result<()> {
    let new_root = host_container_root.to_path_buf();
    let old_root = host_container_root.join("oldroot");

    create_dir_all(&old_root).map_err(|source| Error::CreateDir {
        path: old_root.clone(),
        source,
        details: "Failed to create old root".to_string(),
    })?;

    let new_root_str = new_root.to_str().ok_or_else(|| Error::Configuration {
        component: "container root path".to_string(),
        details: "Container root path contains invalid UTF-8 characters".to_string(),
    })?;

    mount(
        Some(new_root_str),
        new_root_str,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| Error::Mount {
        src: new_root_str.to_string(),
        target: new_root_str.to_string(),
        fstype: None,
        flags: MsFlags::MS_BIND | MsFlags::MS_REC,
        err: e,
        details: "Failed to mount new root".to_string(),
    })?;

    pivot_root(new_root_str, old_root.to_str().unwrap()).map_err(|source| Error::PivotRoot {
        new_root: new_root.clone(),
        old_root: old_root.clone(),
        source,
        details: "Failed to pivot root".to_string(),
    })?;

    set_current_dir("/").map_err(|source| Error::Chdir {
        path: "/".to_string(),
        source,
        details: "Failed to set current directory to root".to_string(),
    })?;

    umount2("/oldroot", MntFlags::MNT_DETACH).map_err(|e| Error::Umount {
        target: "/oldroot".to_string(),
        flags: MntFlags::MNT_DETACH | MntFlags::MNT_FORCE,
        err: e,
        details: "Failed to unmount old root".to_string(),
    })?;

    remove_dir_all("/oldroot").map_err(|source| Error::RemoveDir {
        path: PathBuf::from("/oldroot"),
        source,
        details: "Failed to remove old root".to_string(),
    })?;

    Ok(())
}

/// Creates essential device nodes for the container.
pub(crate) fn create_dev_devices() -> Result<()> {
    let flags = SFlag::S_IFCHR;
    let mode = Mode::S_IRUSR
        | Mode::S_IWUSR
        | Mode::S_IRGRP
        | Mode::S_IWGRP
        | Mode::S_IROTH
        | Mode::S_IWOTH;

    let dev_path = "/dev";
    create_dir_all(dev_path).map_err(|source| Error::CreateDir {
        path: PathBuf::from(dev_path),
        source,
        details: "Failed to create dev directory".to_string(),
    })?;

    // Create null device
    let device_path = format!("{dev_path}/null");
    let device_id = makedev(1, 3);
    mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
        Error::CreateDeviceNode {
            path: PathBuf::from(device_path.clone()),
            source,
            details: "Failed to create null device".to_string(),
        }
    })?;

    // Create zero device
    let device_path = format!("{dev_path}/zero");
    let device_id = makedev(1, 5);
    mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
        Error::CreateDeviceNode {
            path: PathBuf::from(device_path.clone()),
            source,
            details: "Failed to create zero device".to_string(),
        }
    })?;

    // Create full device
    let device_path = format!("{dev_path}/full");
    let device_id = makedev(1, 7);
    mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
        Error::CreateDeviceNode {
            path: PathBuf::from(device_path.clone()),
            source,
            details: "Failed to create full device".to_string(),
        }
    })?;

    // Create random device
    let device_path = format!("{dev_path}/random");
    let device_id = makedev(1, 8);
    mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
        Error::CreateDeviceNode {
            path: PathBuf::from(device_path.clone()),
            source,
            details: "Failed to create random device".to_string(),
        }
    })?;

    // Create urandom device
    let device_path = format!("{dev_path}/urandom");
    let device_id = makedev(1, 9);
    mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
        Error::CreateDeviceNode {
            path: PathBuf::from(device_path.clone()),
            source,
            details: "Failed to create urandom device".to_string(),
        }
    })?;

    Ok(())
}

/// Drops privileges to the nobody user (uid=65534, gid=65534).
/// This function should be called after setting up the container environment
/// but before executing user tasks to ensure security.
pub(crate) fn drop_privileges_to_nobody() -> Result<()> {
    // Set group ID first (setgid must be called before setuid)
    setgid(Gid::from_raw(65534)).map_err(|source| Error::SetGid {
        gid: 65534,
        source,
        details: "Failed to set group ID to nobody".to_string(),
    })?;

    // Set user ID
    setuid(Uid::from_raw(65534)).map_err(|source| Error::SetUid {
        uid: 65534,
        source,
        details: "Failed to set user ID to nobody".to_string(),
    })?;

    Ok(())
}
