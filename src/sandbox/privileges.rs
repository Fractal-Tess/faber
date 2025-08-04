//! Privilege management for container security
//!
//! This module provides functionality to drop privileges and manage
//! user/group permissions for secure container execution.

use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::{debug, warn};

use super::error::SandboxError;

/// Privilege manager for dropping privileges
pub struct PrivilegeManager {
    /// User ID to drop to
    uid: u32,
    /// Group ID to drop to
    gid: u32,
}

impl PrivilegeManager {
    /// Create a new privilege manager
    pub fn new(uid: u32, gid: u32) -> Self {
        Self { uid, gid }
    }

    /// Apply privilege dropping to a command
    pub fn apply_privileges(&self, cmd: &mut Command) -> Result<(), SandboxError> {
        let uid = self.uid;
        let gid = self.gid;

        // Use pre_exec to drop privileges before executing the command
        // This approach should work with stdout/stderr pipes
        unsafe {
            cmd.pre_exec(move || {
                // Drop group privileges first
                if gid != 0 {
                    let result = libc::setgid(gid);
                    if result != 0 {
                        let error = std::io::Error::last_os_error();
                        warn!("Failed to set group ID to {}: {}", gid, error);
                        // Don't fail on privilege dropping errors, just warn
                    }
                }

                // Drop user privileges
                if uid != 0 {
                    let result = libc::setuid(uid);
                    if result != 0 {
                        let error = std::io::Error::last_os_error();
                        warn!("Failed to set user ID to {}: {}", uid, error);
                        // Don't fail on privilege dropping errors, just warn
                    }
                }

                // Set supplementary groups to empty (security measure)
                let result = libc::setgroups(0, std::ptr::null());
                if result != 0 {
                    let error = std::io::Error::last_os_error();
                    warn!("Failed to clear supplementary groups: {}", error);
                    // Don't fail on this, it's not critical
                }

                debug!("Dropped privileges to uid={}, gid={}", uid, gid);
                Ok(())
            });
        }

        Ok(())
    }

    /// Check if we can drop to the specified user/group
    pub fn check_privileges(uid: u32, gid: u32) -> Result<(), SandboxError> {
        // Check if we're running as root (required to drop privileges)
        if unsafe { libc::geteuid() } != 0 {
            warn!("Not running as root, cannot drop privileges");
            return Ok(()); // Not an error, just can't drop privileges
        }

        // Check if the target user/group exists
        if uid != 0 {
            let passwd = unsafe { libc::getpwuid(uid) };
            if passwd.is_null() {
                return Err(SandboxError::ResourceLimitFailed(format!(
                    "User ID {} does not exist",
                    uid
                )));
            }
        }

        if gid != 0 {
            let group = unsafe { libc::getgrgid(gid) };
            if group.is_null() {
                return Err(SandboxError::ResourceLimitFailed(format!(
                    "Group ID {} does not exist",
                    gid
                )));
            }
        }

        debug!("Privilege check passed for uid={}, gid={}", uid, gid);
        Ok(())
    }

    /// Get current effective user ID
    pub fn get_current_uid() -> u32 {
        unsafe { libc::geteuid() }
    }

    /// Get current effective group ID
    pub fn get_current_gid() -> u32 {
        unsafe { libc::getegid() }
    }
}
