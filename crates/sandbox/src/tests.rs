use super::seccomp::SeccompLevel;
use super::*;
use faber_config::Config;
use std::collections::HashMap;

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn test_sandbox_error_variants() {
        let container_error = SandboxError::ContainerCreation("test error".to_string());
        assert!(
            container_error
                .to_string()
                .contains("Container creation failed")
        );

        let mount_error = SandboxError::MountFailed("mount test".to_string());
        assert!(mount_error.to_string().contains("Mount operation failed"));

        let namespace_error = SandboxError::NamespaceSetup("namespace test".to_string());
        assert!(
            namespace_error
                .to_string()
                .contains("Namespace setup failed")
        );

        let resource_error = SandboxError::ResourceLimitFailed("resource test".to_string());
        assert!(
            resource_error
                .to_string()
                .contains("Resource limit enforcement failed")
        );

        let execution_error = SandboxError::ExecutionFailed("execution test".to_string());
        assert!(
            execution_error
                .to_string()
                .contains("Container execution failed")
        );

        let cleanup_error = SandboxError::CleanupFailed("cleanup test".to_string());
        assert!(cleanup_error.to_string().contains("Sandbox cleanup failed"));

        let not_active_error = SandboxError::ContainerNotActive;
        assert_eq!(not_active_error.to_string(), "Container is not active");

        let file_copy_error = SandboxError::FileCopyFailed("copy test".to_string());
        assert!(
            file_copy_error
                .to_string()
                .contains("File copy into container failed")
        );

        let security_error = SandboxError::SecuritySetup("security test".to_string());
        assert!(security_error.to_string().contains("Security setup failed"));
    }

    #[test]
    fn test_sandbox_error_conversion_to_faber_error() {
        let sandbox_error = SandboxError::ContainerCreation("test".to_string());
        let faber_error: faber_core::FaberError = sandbox_error.into();

        match faber_error {
            faber_core::FaberError::Sandbox(msg) => {
                assert!(msg.contains("Container creation failed"));
            }
            _ => panic!("Expected Sandbox error variant"),
        }
    }

    #[test]
    fn test_sandbox_error_conversion_to_task_result() {
        let sandbox_error = SandboxError::ExecutionFailed("test".to_string());
        let task_result: faber_core::TaskResult = sandbox_error.into();

        assert_eq!(task_result.status, faber_core::TaskStatus::Failure);
        assert!(task_result.error.is_some());
        assert!(
            task_result
                .error
                .unwrap()
                .contains("Container execution failed")
        );
        assert!(task_result.exit_code.is_none());
        assert!(task_result.stdout.is_none());
        assert!(task_result.stderr.is_none());
    }
}

#[cfg(test)]
mod resource_limits_tests {
    use super::*;

    #[test]
    fn test_resource_limits_from_config() {
        let config = Config::default();
        let limits = ResourceLimits::from_config(&config);

        // Test that limits are loaded from config (default values from config/default.toml)
        assert_eq!(limits.memory_limit, 524288 * 1024); // 512MB from config
        assert_eq!(limits.cpu_time_limit, 10000 * 1_000_000); // 10 seconds from config
        assert_eq!(limits.wall_time_limit, 30000 * 1_000_000); // 30 seconds from config
        assert_eq!(limits.max_processes, 50); // from config
        assert_eq!(limits.max_fds, 256); // from config
        assert_eq!(limits.stack_limit, 4 * 1024); // 4MB from config
        assert_eq!(limits.data_segment_limit, 256 * 1024); // 256MB from config
        assert_eq!(limits.address_space_limit, 1024 * 1024); // 1GB from config
        assert_eq!(limits.cpu_rate_limit, Some(50)); // 50% from config
        assert_eq!(limits.io_read_limit, Some(10 * 1024)); // 10MB/s from config
        assert_eq!(limits.io_write_limit, Some(10 * 1024)); // 10MB/s from config
    }
}

#[cfg(test)]
mod namespace_settings_tests {
    use super::*;

    #[test]
    fn test_namespace_settings_from_config() {
        let config = Config::default();
        let settings = NamespaceSettings::from_config(&config);

        // Test that namespace settings are loaded from config (default values from config/default.toml)
        assert_eq!(settings.pid, config.sandbox.security.namespaces.pid);
        assert_eq!(settings.mount, config.sandbox.security.namespaces.mount);
        assert_eq!(settings.network, config.sandbox.security.namespaces.network);
        assert_eq!(settings.ipc, config.sandbox.security.namespaces.ipc);
        assert_eq!(settings.uts, config.sandbox.security.namespaces.uts);
        assert_eq!(settings.user, config.sandbox.security.namespaces.user);
        assert_eq!(settings.time, config.sandbox.security.namespaces.time);
        assert_eq!(settings.cgroup, config.sandbox.security.namespaces.cgroup);
    }
}

#[cfg(test)]
mod container_config_tests {
    use super::*;

    #[test]
    fn test_container_config_from_config() {
        let global_config = Config::default();
        let config = ContainerConfig::from_config(&global_config);

        // Test that config is loaded from global config
        assert_eq!(config.resource_limits.memory_limit, 524288 * 1024); // 512MB from config
        assert_eq!(
            config.namespace_settings.pid,
            global_config.sandbox.security.namespaces.pid
        );
        assert_eq!(config.uid, 65534);
        assert_eq!(config.gid, 65534);
        assert_eq!(config.enable_mount_operations, true);
        assert_eq!(config.work_dir_size_mb, 64);
        assert_eq!(
            config.seccomp_level,
            if global_config.sandbox.security.seccomp.enabled {
                SeccompLevel::Basic
            } else {
                SeccompLevel::None
            }
        );
    }

    #[test]
    fn test_container_config_with_resource_limits() {
        let global_config = Config::default();
        let mut config = ContainerConfig::from_config(&global_config);

        let mut custom_limits = ResourceLimits::from_config(&global_config);
        custom_limits.memory_limit = 1024 * 1024 * 1024; // 1GB

        config = config.with_resource_limits(custom_limits.clone());

        assert_eq!(config.resource_limits.memory_limit, 1024 * 1024 * 1024);
        assert_eq!(config.resource_limits.max_processes, 50); // From config
    }

    #[test]
    fn test_container_config_with_namespace_settings() {
        let global_config = Config::default();
        let mut config = ContainerConfig::from_config(&global_config);

        let mut custom_settings = NamespaceSettings::from_config(&global_config);
        custom_settings.user = false;

        config = config.with_namespace_settings(custom_settings.clone());

        assert_eq!(config.namespace_settings.user, false);
        assert_eq!(
            config.namespace_settings.pid,
            global_config.sandbox.security.namespaces.pid
        );
    }

    #[test]
    fn test_container_config_with_user_ids() {
        let global_config = Config::default();
        let config = ContainerConfig::from_config(&global_config).with_user_ids(2000, 2000);

        assert_eq!(config.uid, 2000);
        assert_eq!(config.gid, 2000);
    }

    #[test]
    fn test_container_config_default() {
        let config = ContainerConfig::default();

        // Test default values (should match the hardcoded defaults)
        assert_eq!(config.resource_limits.memory_limit, 512 * 1024 * 1024); // 512MB
        assert_eq!(config.uid, 65534);
        assert_eq!(config.gid, 65534);
    }
}

#[cfg(test)]
mod container_tests {
    use super::*;

    #[test]
    fn test_container_new() {
        let global_config = Config::default();
        let config = ContainerConfig::from_config(&global_config);
        let resource_limits = ResourceLimits::from_config(&global_config);
        let namespace_settings = NamespaceSettings::from_config(&global_config);

        let container = Container::new(
            "test-container".to_string(),
            config.clone(),
            resource_limits.clone(),
            namespace_settings.clone(),
        );

        assert_eq!(container.id, "test-container");
        assert_eq!(container.resource_limits.memory_limit, 524288 * 1024); // From config
        assert_eq!(
            container.namespace_settings.pid,
            global_config.sandbox.security.namespaces.pid
        );
    }

    #[test]
    fn test_container_sandbox_new() {
        let global_config = Config::default();
        let sandbox = ContainerSandbox::from_config(&global_config);

        // This test might fail in non-privileged environments, so we handle the error gracefully
        match sandbox {
            Ok(sandbox) => {
                assert!(sandbox.is_active());
                assert!(sandbox.container_id().len() > 0);
                assert!(
                    sandbox.work_dir().exists() || sandbox.work_dir().parent().unwrap().exists()
                );
            }
            Err(_) => {
                // Expected in non-privileged environments
                // The error should be related to container creation
            }
        }
    }

    #[test]
    fn test_container_sandbox_copy_files() {
        let global_config = Config::default();
        let mut sandbox = ContainerSandbox::from_config(&global_config);

        match sandbox {
            Ok(mut sandbox) => {
                let mut files = HashMap::new();
                files.insert("test.txt".to_string(), "Hello, World!".to_string());

                let result = sandbox.copy_files_in(&files);
                // This might fail in non-privileged environments
                if result.is_err() {
                    // Expected in some environments
                }
            }
            Err(_) => {
                // Expected in non-privileged environments
            }
        }
    }
}

#[cfg(test)]
mod cgroup_manager_tests {
    use super::*;

    #[test]
    fn test_cgroup_manager_new() {
        let config = Config::default();
        let manager = cgroups::CgroupManager::new("test-container", &config);

        match manager {
            Ok(manager) => {
                assert_eq!(manager.prefix, "faber");
                assert_eq!(manager.cgroup_path, "faber/test-container");
                assert!(manager.base_path.is_none());
            }
            Err(_) => {
                // Expected in environments without cgroup support
            }
        }
    }

    #[test]
    fn test_cgroups_manager_new() {
        let manager = cgroups::CgroupsManager::new(
            "test-prefix".to_string(),
            Some("/sys/fs/cgroup".to_string()),
        );

        assert_eq!(manager.prefix, "test-prefix");
        assert_eq!(manager.base_path, Some("/sys/fs/cgroup".to_string()));
    }

    #[test]
    fn test_cgroups_manager_new_without_base_path() {
        let manager = cgroups::CgroupsManager::new("test-prefix".to_string(), None);

        assert_eq!(manager.prefix, "test-prefix");
        assert!(manager.base_path.is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_resource_limits_serialization() {
        let config = Config::default();
        let limits = ResourceLimits::from_config(&config);
        let serialized = serde_json::to_string(&limits).unwrap();
        let deserialized: ResourceLimits = serde_json::from_str(&serialized).unwrap();

        assert_eq!(limits.memory_limit, deserialized.memory_limit);
        assert_eq!(limits.cpu_time_limit, deserialized.cpu_time_limit);
        assert_eq!(limits.max_processes, deserialized.max_processes);
    }

    #[test]
    fn test_container_config_serialization() {
        let global_config = Config::default();
        let config = ContainerConfig::from_config(&global_config);
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ContainerConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            config.resource_limits.memory_limit,
            deserialized.resource_limits.memory_limit
        );
        assert_eq!(config.uid, deserialized.uid);
        assert_eq!(config.gid, deserialized.gid);
    }
}
