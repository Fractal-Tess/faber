use crate::prelude::*;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
    sync::Once,
};

static CGLOBAL_INIT: Once = Once::new();

#[derive(Debug)]
pub struct CgroupManager {
    cgroup_path: PathBuf,
}

impl CgroupManager {
    /// Check the current state of the root cgroup system for debugging
    pub fn debug_root_cgroup_state() -> Result<()> {
        println!("=== Root Cgroup System State ===");

        let root_cgroup_path = PathBuf::from("/sys/fs/cgroup");
        println!("Root path: {:?}", root_cgroup_path);
        println!("Root exists: {}", root_cgroup_path.exists());

        if !root_cgroup_path.exists() {
            println!("ERROR: Root cgroup path does not exist!");
            return Ok(());
        }

        // Check cgroup type
        let cgroup_type_path = root_cgroup_path.join("cgroup.type");
        if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
            println!("Root cgroup type: '{}'", cgroup_type.trim());
        } else {
            println!("ERROR: Could not read root cgroup type");
        }

        // Check controllers
        let cgroup_controllers_path = root_cgroup_path.join("cgroup.controllers");
        if let Ok(controllers) = std::fs::read_to_string(&cgroup_controllers_path) {
            println!("Root cgroup controllers: '{}'", controllers.trim());
        } else {
            println!("ERROR: Could not read root cgroup controllers");
        }

        // Check subtree control
        let subtree_control_path = root_cgroup_path.join("cgroup.subtree_control");
        if let Ok(subtree_control) = std::fs::read_to_string(&subtree_control_path) {
            println!("Root subtree control: '{}'", subtree_control.trim());
        } else {
            println!("ERROR: Could not read root subtree control");
        }

        // Check if faber cgroup exists
        let faber_cgroup_path = root_cgroup_path.join("faber");
        println!("Faber cgroup exists: {}", faber_cgroup_path.exists());

        if faber_cgroup_path.exists() {
            let faber_type_path = faber_cgroup_path.join("cgroup.type");
            if let Ok(faber_type) = std::fs::read_to_string(&faber_type_path) {
                println!("Faber cgroup type: '{}'", faber_type.trim());
            }

            let faber_controllers_path = faber_cgroup_path.join("cgroup.controllers");
            if let Ok(faber_controllers) = std::fs::read_to_string(&faber_controllers_path) {
                println!("Faber cgroup controllers: '{}'", faber_controllers.trim());
            }
        }

        println!("=================================");
        Ok(())
    }

    /// Initialize the global cgroup subsystem once per application lifecycle
    fn initialize_cgroup_subsystem() -> Result<()> {
        // Use Once to ensure this runs only once per process
        let mut initialized = false;
        CGLOBAL_INIT.call_once(|| {
            initialized = Self::do_initialize_cgroup_subsystem().is_ok();
        });

        if initialized {
            Ok(())
        } else {
            Err(Error::Generic(
                "Failed to initialize cgroup subsystem".to_string(),
            ))
        }
    }

    /// Actually perform the cgroup subsystem initialization
    fn do_initialize_cgroup_subsystem() -> Result<()> {
        println!("=== Starting cgroup subsystem initialization ===");

        // First, check the current state
        Self::debug_root_cgroup_state()?;

        let root_cgroup_path = PathBuf::from("/sys/fs/cgroup");
        println!("Root cgroup path: {:?}", root_cgroup_path);
        println!("Root cgroup exists: {}", root_cgroup_path.exists());

        // Check if controllers are already enabled to avoid unnecessary operations
        let cgroup_subtree_control = root_cgroup_path.join("cgroup.subtree_control");
        println!("Subtree control path: {:?}", cgroup_subtree_control);
        println!(
            "Subtree control exists: {}",
            cgroup_subtree_control.exists()
        );

        let current_controllers =
            std::fs::read_to_string(&cgroup_subtree_control).unwrap_or_default();
        println!(
            "Current controllers in root: '{}'",
            current_controllers.trim()
        );

        // Only enable controllers if they're not already enabled
        if !current_controllers.contains("cpu") || !current_controllers.contains("pids") {
            println!("Enabling cpu and pids controllers...");
            let mut file = OpenOptions::new()
                .write(true)
                .open(&cgroup_subtree_control)?;

            // Enable both cpu and pids controllers
            let controller_string = b"+cpu +pids";
            println!(
                "Writing '{}' to subtree_control",
                String::from_utf8_lossy(controller_string)
            );
            file.write_all(controller_string).map_err(|e| {
                Error::Generic(format!("Failed to write to cgroup.subtree_control: {e}"))
            })?;
            file.sync_all().map_err(|e| {
                Error::Generic(format!("Failed to sync cgroup.subtree_control: {e}"))
            })?;

            // Read back to confirm
            let updated_controllers =
                std::fs::read_to_string(&cgroup_subtree_control).unwrap_or_default();
            println!(
                "Controllers after enabling: '{}'",
                updated_controllers.trim()
            );
        } else {
            println!("Controllers already enabled, skipping...");
        }

        // Create the faber parent cgroup if it doesn't exist
        let faber_cgroup_path = PathBuf::from("/sys/fs/cgroup/faber");
        println!("Faber cgroup path: {:?}", faber_cgroup_path);
        println!("Faber cgroup exists: {}", faber_cgroup_path.exists());

        if !faber_cgroup_path.exists() {
            println!("Creating faber parent cgroup...");
            std::fs::create_dir_all(&faber_cgroup_path)
                .map_err(|e| Error::Generic(format!("Failed to create faber cgroup: {e}")))?;
            println!("✓ Faber parent cgroup created");

            // Debug: Show what files are available in the faber cgroup
            println!("=== Faber cgroup contents after creation ===");
            if let Ok(entries) = std::fs::read_dir(&faber_cgroup_path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        println!("  {:?}", entry.path());
                    }
                }
            }
            println!("===========================================");

            // Enable controllers in the faber cgroup itself
            let faber_subtree_control = faber_cgroup_path.join("cgroup.subtree_control");
            println!("Enabling controllers in faber cgroup...");
            println!("Faber subtree control path: {:?}", faber_subtree_control);
            println!(
                "Faber subtree control exists: {}",
                faber_subtree_control.exists()
            );

            // Check file permissions and attributes
            if faber_subtree_control.exists() {
                if let Ok(metadata) = std::fs::metadata(&faber_subtree_control) {
                    println!("Faber subtree_control metadata: {:?}", metadata);
                    println!(
                        "Faber subtree_control permissions: {:?}",
                        metadata.permissions()
                    );
                    println!(
                        "Faber subtree_control readonly: {}",
                        metadata.permissions().readonly()
                    );
                }

                // Try to read the file to see if it's accessible
                if let Ok(current_faber_controllers) =
                    std::fs::read_to_string(&faber_subtree_control)
                {
                    println!(
                        "Current faber subtree control: '{}'",
                        current_faber_controllers.trim()
                    );
                } else {
                    println!("Could not read current faber subtree control");
                }
            } else {
                println!("ERROR: faber subtree_control file does not exist!");
                // List what files are actually in the faber cgroup
                println!("Available files in faber cgroup:");
                if let Ok(entries) = std::fs::read_dir(&faber_cgroup_path) {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            println!("  {:?}", entry.path());
                        }
                    }
                }
            }

            let mut faber_file = OpenOptions::new()
                .write(true)
                .open(&faber_subtree_control)
                .map_err(|e| {
                    println!("ERROR: Failed to open faber subtree_control: {:?}", e);
                    Error::Generic(format!("Failed to open faber subtree_control: {e}"))
                })?;

            let controller_string = b"+cpu +pids";
            println!(
                "Writing '{}' to faber subtree_control",
                String::from_utf8_lossy(controller_string)
            );
            faber_file.write_all(controller_string).map_err(|e| {
                println!("ERROR: Failed to write to faber subtree_control: {:?}", e);
                Error::Generic(format!("Failed to write to faber subtree_control: {e}"))
            })?;
            faber_file.sync_all().map_err(|e| {
                println!("ERROR: Failed to sync faber subtree_control: {:?}", e);
                Error::Generic(format!("Failed to sync faber subtree_control: {e}"))
            })?;

            // Read back to confirm
            let faber_controllers =
                std::fs::read_to_string(&faber_subtree_control).unwrap_or_default();
            println!(
                "Faber subtree control after enabling: '{}'",
                faber_controllers.trim()
            );

            // Wait a moment for the cgroup to stabilize
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Check what the system assigned
            let cgroup_type_path = faber_cgroup_path.join("cgroup.type");
            if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                println!("Faber cgroup type after creation: '{}'", cgroup_type.trim());
            }

            let controllers_path = faber_cgroup_path.join("cgroup.controllers");
            if let Ok(controllers) = std::fs::read_to_string(&controllers_path) {
                println!("Faber cgroup controllers: '{}'", controllers.trim());
            }

            // If still invalid, wait a bit more
            if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                if cgroup_type.trim() == "domain invalid" {
                    println!("⚠️  Faber cgroup still invalid, waiting longer...");
                    std::thread::sleep(std::time::Duration::from_millis(500));

                    if let Ok(retry_cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                        println!(
                            "Faber cgroup type after retry: '{}'",
                            retry_cgroup_type.trim()
                        );
                    }
                }
            }
        } else {
            println!("Faber parent cgroup already exists");
            // Check if existing faber cgroup is in valid state
            let cgroup_type_path = faber_cgroup_path.join("cgroup.type");
            if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                println!("Existing faber cgroup type: '{}'", cgroup_type.trim());
                if cgroup_type.trim() == "domain invalid" {
                    println!(
                        "⚠️  Existing faber cgroup is invalid; will not attempt repair. Container cgroups will fall back to root-level (e.g. '/sys/fs/cgroup/faber-<id>')."
                    );
                    // Intentionally do not attempt removal here, as it can fail when tasks/children exist
                    // and isn't necessary for operation since we have a fallback path during cgroup creation.
                }
            }
        }

        println!("=== Cgroup subsystem initialization complete ===");
        Ok(())
    }

    pub fn new(id: impl Into<String>) -> Result<Self> {
        let id_string = id.into();
        println!("=== Creating CgroupManager for ID: {} ===", id_string);

        // Ensure global cgroup subsystem is initialized (only happens once)
        println!("Initializing cgroup subsystem...");
        Self::initialize_cgroup_subsystem()?;
        println!("✓ Cgroup subsystem initialized");

        // Try to create container cgroup under faber first
        let faber_cgroup_path = PathBuf::from("/sys/fs/cgroup/faber");
        let cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/faber/{}", id_string));

        // Check if faber cgroup is working
        let faber_working = faber_cgroup_path.exists() && {
            let cgroup_type_path = faber_cgroup_path.join("cgroup.type");
            if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                cgroup_type.trim() != "domain invalid"
            } else {
                false
            }
        };

        if faber_working {
            println!("Faber cgroup is working, creating container cgroup under it");
            return Self::create_container_cgroup(&cgroup_path, id_string);
        } else {
            println!("⚠️  Faber cgroup not working, falling back to root-level container cgroup");
            // Fallback: create container cgroup directly under root
            let fallback_cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/faber-{}", id_string));
            return Self::create_container_cgroup(&fallback_cgroup_path, id_string);
        }
    }

    fn create_container_cgroup(cgroup_path: &PathBuf, id_string: String) -> Result<Self> {
        println!("Container cgroup path: {:?}", cgroup_path);
        println!("Container cgroup exists: {}", cgroup_path.exists());

        // Check if cgroup already exists and is in valid state
        if cgroup_path.exists() {
            println!("Container cgroup already exists, checking state...");
            let cgroup_type_path = cgroup_path.join("cgroup.type");
            if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                println!("Existing container cgroup type: '{}'", cgroup_type.trim());
                if cgroup_type.trim() == "domain invalid" {
                    println!("⚠️  Existing container cgroup is invalid, removing...");
                    // Remove invalid cgroup and recreate
                    std::fs::remove_dir_all(cgroup_path).map_err(|e| {
                        Error::Generic(format!("Failed to remove invalid cgroup: {e}"))
                    })?;
                    println!("✓ Invalid container cgroup removed");
                } else {
                    println!("✓ Existing container cgroup is valid, using it");
                    // Cgroup exists and is valid, use it
                    return Ok(CgroupManager {
                        cgroup_path: cgroup_path.clone(),
                    });
                }
            }
        }

        // Create new cgroup
        println!("Creating new container cgroup...");
        std::fs::create_dir_all(cgroup_path)
            .map_err(|e| Error::Generic(format!("Failed to create container cgroup: {e}")))?;
        println!("✓ Container cgroup directory created");

        // Wait a moment for the cgroup to stabilize, then check its state
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Check what the system assigned
        let cgroup_type_path = cgroup_path.join("cgroup.type");
        if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
            println!("New container cgroup type: '{}'", cgroup_type.trim());

            // If still invalid, try to wait a bit more and check again
            if cgroup_type.trim() == "domain invalid" {
                println!("⚠️  Container cgroup still invalid, waiting longer...");
                std::thread::sleep(std::time::Duration::from_millis(500));

                if let Ok(retry_cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
                    println!(
                        "Container cgroup type after retry: '{}'",
                        retry_cgroup_type.trim()
                    );
                }
            }
        }

        let controllers_path = cgroup_path.join("cgroup.controllers");
        if let Ok(controllers) = std::fs::read_to_string(&controllers_path) {
            println!("New container cgroup controllers: '{}'", controllers.trim());
        }

        // Don't try to set cgroup type - let the system decide
        // The cgroup type will be automatically determined based on the controllers

        println!("=== CgroupManager creation complete ===");
        Ok(CgroupManager {
            cgroup_path: cgroup_path.clone(),
        })
    }

    pub fn debug_cgroup_contents(&self) -> Result<()> {
        let contents = std::fs::read_dir(&self.cgroup_path)?;
        for entry in contents {
            println!("Cgroup entry: {:?}", entry.unwrap().path());
        }
        Ok(())
    }

    /// Print detailed information about the cgroup state for debugging
    pub fn debug_cgroup_state(&self) -> Result<()> {
        println!("=== Cgroup Debug Info ===");
        println!("Path: {:?}", self.cgroup_path);

        // Check if cgroup exists
        if !self.cgroup_path.exists() {
            println!("ERROR: Cgroup directory does not exist!");
            return Ok(());
        }

        // Check cgroup type
        let cgroup_type_path = self.cgroup_path.join("cgroup.type");
        if let Ok(cgroup_type) = std::fs::read_to_string(&cgroup_type_path) {
            println!("Type: {}", cgroup_type.trim());
        } else {
            println!("ERROR: Could not read cgroup type");
        }

        // Check controllers
        let cgroup_controllers_path = self.cgroup_path.join("cgroup.controllers");
        if let Ok(controllers) = std::fs::read_to_string(&cgroup_controllers_path) {
            println!("Controllers: {}", controllers.trim());
        } else {
            println!("ERROR: Could not read cgroup controllers");
        }

        // Check if key files exist
        let key_files = ["cgroup.procs", "pids.max", "cgroup.events"];
        for file in key_files {
            let file_path = self.cgroup_path.join(file);
            if file_path.exists() {
                println!("✓ {} exists", file);
            } else {
                println!("✗ {} missing", file);
            }
        }

        println!("========================");
        Ok(())
    }

    /// Validate that the cgroup is in a proper state for operations
    pub fn validate_cgroup_state(&self) -> Result<()> {
        // Check cgroup type
        let cgroup_type_path = self.cgroup_path.join("cgroup.type");
        let cgroup_type = std::fs::read_to_string(&cgroup_type_path)
            .map_err(|e| Error::Generic(format!("Failed to read cgroup type: {e}")))?;

        println!("Cgroup type: {}", cgroup_type.trim());

        if cgroup_type.trim() == "domain invalid" {
            return Err(Error::Generic("Cgroup is in invalid state".to_string()));
        }

        // Accept any valid cgroup type (domain, threaded, etc.)
        // The system will assign the appropriate type based on the controllers

        // Check if pids controller is available
        let pids_max_path = self.cgroup_path.join("pids.max");
        if !pids_max_path.exists() {
            return Err(Error::Generic(format!(
                "pids.max not found. Cgroup type: {}, Path: {:?}",
                cgroup_type.trim(),
                self.cgroup_path
            )));
        }

        // Check if cgroup.procs is available
        let cgroup_procs_path = self.cgroup_path.join("cgroup.procs");
        if !cgroup_procs_path.exists() {
            return Err(Error::Generic(format!(
                "cgroup.procs not found. Cgroup type: {}, Path: {:?}",
                cgroup_type.trim(),
                self.cgroup_path
            )));
        }

        println!("Cgroup validation passed - type: {}", cgroup_type.trim());
        Ok(())
    }

    pub fn add_proc(&self, pid: u32) -> Result<()> {
        // Check if process exists first
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return Err(Error::Generic(format!("Process {pid} does not exist")));
        }

        // Check if cgroup is in a valid state
        let cgroup_type_path = self.cgroup_path.join("cgroup.type");
        let cgroup_type = std::fs::read_to_string(&cgroup_type_path)
            .map_err(|e| Error::Generic(format!("Failed to read cgroup type: {e}")))?;

        if cgroup_type.trim() == "domain invalid" {
            return Err(Error::Generic(
                "Cgroup is in invalid state. Try recreating the cgroup.".to_string(),
            ));
        }

        let cgroup_procs = self.cgroup_path.join("cgroup.procs");

        let mut file = OpenOptions::new()
            .append(true)
            .truncate(false)
            .open(&cgroup_procs)?;

        // Write the PID as bytes with newline
        file.write_all(format!("{pid}\n").as_bytes())
            .map_err(|e| Error::Generic(format!("Failed to write to cgroup.procs: {e}")))?;

        // Ensure immediate flush
        file.sync_all()
            .map_err(|e| Error::Generic(format!("Failed to sync cgroup.procs: {e}")))?;

        Ok(())
    }
    pub fn print_group_procs(&self) -> Result<()> {
        let procs_path = self.cgroup_path.join("cgroup.procs");

        let mut file = OpenOptions::new().read(true).open(procs_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        println!("Cgroup procs: {}", contents);

        Ok(())
    }

    pub fn set_max_procs(&self, limit: u64) -> Result<()> {
        // First check if the cgroup is in a valid state
        let cgroup_type_path = self.cgroup_path.join("cgroup.type");
        let cgroup_type = std::fs::read_to_string(&cgroup_type_path)
            .map_err(|e| Error::Generic(format!("Failed to read cgroup type: {e}")))?;

        if cgroup_type.trim() == "domain invalid" {
            return Err(Error::Generic(
                "Cannot set max procs on invalid cgroup".to_string(),
            ));
        }

        let cgroup_pids = self.cgroup_path.join("pids.max");

        // Check if the pids.max file exists
        if !cgroup_pids.exists() {
            return Err(Error::Generic(format!(
                "pids.max file not found in cgroup. Cgroup type: {}, Path: {:?}",
                cgroup_type.trim(),
                self.cgroup_path
            )));
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(false)
            .open(&cgroup_pids)
            .map_err(|e| Error::Generic(format!("Failed to open pids.max: {e}")))?;

        file.write_all(format!("{limit}\n").as_bytes())
            .map_err(|e| Error::Generic(format!("Failed to write to pids.max: {e}")))?;

        file.sync_all()
            .map_err(|e| Error::Generic(format!("Failed to sync pids.max: {e}")))?;

        Ok(())
    }
    pub fn print_max_procs(&self) -> Result<()> {
        let pids_path = self.cgroup_path.join("pids.max");

        let mut file = OpenOptions::new().read(true).open(pids_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        println!("Cgroup pids: {contents}");

        Ok(())
    }
}
