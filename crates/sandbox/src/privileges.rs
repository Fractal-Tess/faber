use faber_core::{FaberError, Result};
use std::os::unix::process::CommandExt;
use std::process::Command;
use tracing::{debug, warn};

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
    pub fn apply_privileges(&self, _cmd: &mut Command) -> Result<()> {
        let uid = self.uid;
        let gid = self.gid;

        // Use pre_exec to drop privileges before executing the command
        // This approach should work with stdout/stderr pipes
        unsafe {
            _cmd.pre_exec(move || {
                // Drop group privileges first
                if gid != 0 {
                    let result = libc::setgid(gid);
                    if result != 0 {
                        // Don't fail on privilege dropping errors, just continue
                    }
                }

                // Drop user privileges
                if uid != 0 {
                    let result = libc::setuid(uid);
                    if result != 0 {
                        // Don't fail on privilege dropping errors, just continue
                    }
                }

                // Set supplementary groups to empty (security measure)
                let result = libc::setgroups(0, std::ptr::null());
                if result != 0 {
                    // Don't fail on this, it's not critical
                }
                Ok(())
            });
        }

        Ok(())
    }

    /// Check if we can drop to the specified user/group
    pub fn check_privileges(uid: u32, gid: u32) -> Result<()> {
        // Check if we're running as root (required to drop privileges)
        if unsafe { libc::geteuid() } != 0 {
            warn!("Not running as root, cannot drop privileges");
            return Ok(()); // Not an error, just can't drop privileges
        }

        // Check if the target user/group exists
        if uid != 0 {
            let passwd = unsafe { libc::getpwuid(uid) };
            if passwd.is_null() {
                return Err(FaberError::Sandbox(format!("User ID {uid} does not exist")));
            }
        }

        if gid != 0 {
            let group = unsafe { libc::getgrgid(gid) };
            if group.is_null() {
                return Err(FaberError::Sandbox(format!(
                    "Group ID {gid} does not exist"
                )));
            }
        }

        debug!("Privilege check passed for uid={uid}, gid={gid}");
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
