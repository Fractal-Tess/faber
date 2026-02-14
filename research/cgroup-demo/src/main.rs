//! Cgroup v2 + Namespace Isolation Demo
//!
//! Demonstrates Linux container primitives:
//! - Cgroups v2 for resource limiting (memory, CPU, PIDs)
//! - Namespaces for isolation (PID, mount, UTS, IPC, network)

use nix::mount::{mount, umount2, MntFlags, MsFlags};
use nix::sched::{clone, CloneFlags};
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{getpid, sethostname, Pid};
use std::fs;
use std::io;
use std::path::Path;
use std::process;
use std::thread;
use std::time::Duration;

const CGROUP_ROOT: &str = "/sys/fs/cgroup";

fn main() {
    println!("=== Cgroup + Namespace Isolation Demo ===\n");

    // Must run as root
    if !nix::unistd::geteuid().is_root() {
        eprintln!("Error: This program must run as root.");
        process::exit(1);
    }

    // Detect cgroup setup
    println!("[Info] Detecting cgroup setup...");
    detect_cgroup_setup();

    // Print Docker-set limits
    println!("\n[Cgroup] Current resource limits:");
    print_cgroup_limits(CGROUP_ROOT);

    // Define namespaces to create
    let namespaces = CloneFlags::CLONE_NEWPID    // New PID namespace (child sees PID 1)
        | CloneFlags::CLONE_NEWNS               // New mount namespace
        | CloneFlags::CLONE_NEWUTS              // New UTS namespace (hostname)
        | CloneFlags::CLONE_NEWIPC              // New IPC namespace
        | CloneFlags::CLONE_NEWNET;             // New network namespace

    println!("\n[Namespaces] Creating isolated child with:");
    println!("  - CLONE_NEWPID  (isolated PID namespace)");
    println!("  - CLONE_NEWNS   (isolated mount namespace)");
    println!("  - CLONE_NEWUTS  (isolated hostname)");
    println!("  - CLONE_NEWIPC  (isolated IPC)");
    println!("  - CLONE_NEWNET  (isolated network)");

    // Allocate stack for clone (required by clone syscall)
    const STACK_SIZE: usize = 1024 * 1024; // 1MB stack
    let mut stack = vec![0u8; STACK_SIZE];

    // Clone with namespaces
    println!("\n[Parent] Spawning isolated child process...");

    let child_pid = match unsafe {
        clone(
            Box::new(|| child_main()),
            &mut stack,
            namespaces,
            Some(Signal::SIGCHLD as i32),
        )
    } {
        Ok(pid) => {
            println!("[Parent] Child created with PID {} (in parent namespace)", pid);
            pid
        }
        Err(e) => {
            eprintln!("[Parent] Clone failed: {}", e);
            process::exit(1);
        }
    };

    // Monitor the child via cgroups
    println!("\n[Parent] Monitoring child via cgroup stats...");
    monitor_cgroup(CGROUP_ROOT, child_pid);

    // Wait for child
    match waitpid(child_pid, None) {
        Ok(WaitStatus::Exited(_, code)) => {
            println!("\n[Parent] Child exited with code: {}", code);
        }
        Ok(WaitStatus::Signaled(_, sig, _)) => {
            println!("\n[Parent] Child killed by signal: {:?}", sig);
        }
        Ok(status) => {
            println!("\n[Parent] Child status: {:?}", status);
        }
        Err(e) => {
            eprintln!("\n[Parent] Wait error: {}", e);
        }
    }

    // Final stats
    println!("\n[Parent] Final cgroup statistics:");
    print_cgroup_stats(CGROUP_ROOT);
}

/// Child process entry point - runs in isolated namespaces
fn child_main() -> isize {
    println!("\n[Child] === Entering isolated environment ===");

    // Show our PID (should be 1 in new PID namespace)
    let my_pid = getpid();
    println!("[Child] PID in namespace: {} (should be 1)", my_pid);

    // Set up namespace isolation
    if let Err(e) = setup_namespace_isolation() {
        eprintln!("[Child] Namespace setup failed: {}", e);
        return 1;
    }

    // Show isolation proof
    println!("\n[Child] === Isolation Verification ===");
    verify_isolation();

    // Run memory-intensive workload
    println!("\n[Child] === Running Workload ===");
    run_workload();

    println!("\n[Child] === Exiting isolated environment ===");
    0
}

/// Set up isolation within the new namespaces
fn setup_namespace_isolation() -> io::Result<()> {
    // Set a custom hostname (UTS namespace)
    println!("[Child] Setting hostname to 'isolated-container'");
    if let Err(e) = sethostname("isolated-container") {
        println!("[Child] Warning: Could not set hostname: {}", e);
    }

    // Make mount namespace private (prevent mount propagation)
    println!("[Child] Making mounts private");
    if let Err(e) = mount::<str, str, str, str>(
        None,
        "/",
        None,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None,
    ) {
        println!("[Child] Warning: Could not make mounts private: {}", e);
    }

    // Mount a new /proc for our PID namespace
    println!("[Child] Mounting new /proc");
    if Path::new("/proc").exists() {
        // Unmount old /proc first
        let _ = umount2("/proc", MntFlags::MNT_DETACH);

        // Mount new /proc
        if let Err(e) = mount::<str, str, str, str>(
            Some("proc"),
            "/proc",
            Some("proc"),
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
            None,
        ) {
            println!("[Child] Warning: Could not mount /proc: {}", e);
        }
    }

    // Mount a minimal /sys (read-only)
    println!("[Child] Remounting /sys read-only");
    if Path::new("/sys").exists() {
        let _ = mount::<str, str, str, str>(
            None,
            "/sys",
            None,
            MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY | MsFlags::MS_BIND,
            None,
        );
    }

    // Make /tmp private
    if Path::new("/tmp").exists() {
        println!("[Child] Mounting private /tmp");
        let _ = mount::<str, str, str, str>(
            Some("tmpfs"),
            "/tmp",
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("size=10M"),
        );
    }

    Ok(())
}

/// Verify isolation is working
fn verify_isolation() {
    // Check hostname
    println!("[Child] Hostname verification:");
    if let Ok(hostname) = fs::read_to_string("/proc/sys/kernel/hostname") {
        println!("  Hostname: {}", hostname.trim());
    } else {
        // Try alternative method
        if let Ok(hostname) = nix::unistd::gethostname() {
            println!("  Hostname: {}", hostname.to_string_lossy());
        }
    }

    // Check PID 1 is us (new PID namespace)
    let my_pid = getpid();
    println!("\n[Child] PID namespace verification:");
    println!("  Our PID: {} {}", my_pid,
        if my_pid.as_raw() == 1 { "(we are init!)" } else { "" });

    // List all processes visible in our namespace
    println!("\n[Child] Process visibility:");
    if let Ok(entries) = fs::read_dir("/proc") {
        let mut pids: Vec<u32> = Vec::new();
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if let Ok(pid) = name.parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
        pids.sort();
        if pids.is_empty() {
            println!("  (no /proc entries - PID namespace is fully isolated)");
        } else {
            for pid in &pids {
                if let Ok(cmdline) = fs::read_to_string(format!("/proc/{}/comm", pid)) {
                    println!("  PID {}: {}", pid, cmdline.trim());
                }
            }
        }
        println!("  Total visible processes: {}", pids.len());
    }

    // Check network isolation
    println!("\n[Child] Network namespace verification:");
    if let Ok(entries) = fs::read_dir("/sys/class/net") {
        let interfaces: Vec<_> = entries.flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        if interfaces.is_empty() || (interfaces.len() == 1 && interfaces[0] == "lo") {
            println!("  Network is isolated (only loopback or empty)");
        } else {
            println!("  Interfaces: {}", interfaces.join(", "));
        }
    } else {
        println!("  (network isolated - no /sys/class/net access)");
    }

    // Check namespace IDs to prove isolation
    println!("\n[Child] Namespace IDs (proof of isolation):");
    for ns in &["pid", "mnt", "uts", "ipc", "net"] {
        if let Ok(link) = fs::read_link(format!("/proc/self/ns/{}", ns)) {
            println!("  {}: {}", ns, link.display());
        }
    }

    // IPC isolation test - check shared memory
    println!("\n[Child] IPC isolation verification:");
    if let Ok(shm) = fs::read_dir("/dev/shm") {
        let count = shm.count();
        println!("  /dev/shm entries: {} (isolated namespace has 0)", count);
    }
}

/// Run memory and CPU intensive workload
fn run_workload() {
    println!("[Child] Starting memory allocation workload...");
    println!("[Child] Will allocate 4MB chunks until limits hit");

    let mut allocations: Vec<Vec<u8>> = Vec::new();

    for i in 1..=15 {
        // Allocate 4MB and touch all bytes
        let mut chunk: Vec<u8> = Vec::with_capacity(4 * 1024 * 1024);
        for _ in 0..(4 * 1024 * 1024) {
            chunk.push((i % 256) as u8);
        }
        allocations.push(chunk);

        let total_mb = i * 4;
        println!("[Child] Chunk {} allocated (total: ~{}MB)", i, total_mb);

        // CPU work
        let mut x: u64 = 0;
        for j in 0..500_000u64 {
            x = x.wrapping_add(j * j);
        }
        let _ = x;

        thread::sleep(Duration::from_millis(200));

        if total_mb >= 32 {
            println!("[Child] ** High memory usage: {}MB **", total_mb);
        }
    }

    drop(allocations);
    println!("[Child] Workload completed");
}

fn detect_cgroup_setup() {
    let controllers_path = format!("{}/cgroup.controllers", CGROUP_ROOT);
    if Path::new(&controllers_path).exists() {
        println!("[Info] cgroups v2 detected at {}", CGROUP_ROOT);
        if let Ok(content) = fs::read_to_string(&controllers_path) {
            println!("[Info] Available controllers: {}", content.trim());
        }
    }
}

fn print_cgroup_limits(cgroup_path: &str) {
    // Memory
    if let Ok(content) = fs::read_to_string(format!("{}/memory.max", cgroup_path)) {
        let val = content.trim();
        if val == "max" {
            println!("  memory.max:  unlimited");
        } else if let Ok(bytes) = val.parse::<u64>() {
            println!("  memory.max:  {} MB", bytes / (1024 * 1024));
        }
    }

    // CPU
    if let Ok(content) = fs::read_to_string(format!("{}/cpu.max", cgroup_path)) {
        let parts: Vec<&str> = content.trim().split_whitespace().collect();
        if parts.len() == 2 {
            if parts[0] == "max" {
                println!("  cpu.max:     unlimited");
            } else if let (Ok(quota), Ok(period)) =
                (parts[0].parse::<u64>(), parts[1].parse::<u64>())
            {
                let percent = (quota as f64 / period as f64) * 100.0;
                println!("  cpu.max:     {:.0}%", percent);
            }
        }
    }

    // PIDs
    if let Ok(content) = fs::read_to_string(format!("{}/pids.max", cgroup_path)) {
        println!("  pids.max:    {}", content.trim());
    }
}

fn monitor_cgroup(cgroup_path: &str, child_pid: Pid) {
    for i in 1..=10 {
        thread::sleep(Duration::from_millis(500));

        // Check if child still running
        let proc_path = format!("/proc/{}/status", child_pid);
        let child_alive = Path::new(&proc_path).exists();

        let status = if child_alive { "" } else { "(child exited)" };
        println!("\n--- Snapshot {} {} ---", i, status);
        print_cgroup_stats(cgroup_path);

        if !child_alive {
            break;
        }
    }
}

fn print_cgroup_stats(cgroup_path: &str) {
    // Memory
    if let Ok(content) = fs::read_to_string(format!("{}/memory.current", cgroup_path)) {
        if let Ok(bytes) = content.trim().parse::<u64>() {
            let mb = bytes as f64 / (1024.0 * 1024.0);
            println!("  Memory current: {:.2} MB", mb);
        }
    }

    if let Ok(content) = fs::read_to_string(format!("{}/memory.peak", cgroup_path)) {
        if let Ok(bytes) = content.trim().parse::<u64>() {
            let mb = bytes as f64 / (1024.0 * 1024.0);
            println!("  Memory peak:    {:.2} MB", mb);
        }
    }

    // PIDs
    if let Ok(content) = fs::read_to_string(format!("{}/pids.current", cgroup_path)) {
        print!("  PIDs current:   {}", content.trim());
        if let Ok(max) = fs::read_to_string(format!("{}/pids.max", cgroup_path)) {
            print!(" / {}", max.trim());
        }
        println!();
    }

    // CPU
    if let Ok(content) = fs::read_to_string(format!("{}/cpu.stat", cgroup_path)) {
        for line in content.lines().take(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(usec) = parts[1].parse::<u64>() {
                    let ms = usec as f64 / 1000.0;
                    println!("  CPU {}:  {:.1} ms", parts[0], ms);
                }
            }
        }
    }

    // Memory events
    if let Ok(content) = fs::read_to_string(format!("{}/memory.events", cgroup_path)) {
        let mut events = Vec::new();
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(count) = parts[1].parse::<u64>() {
                    if count > 0 {
                        match parts[0] {
                            "high" => events.push(format!("throttled={}", count)),
                            "oom" => events.push(format!("oom={}", count)),
                            "oom_kill" => events.push(format!("oom_kill={}", count)),
                            _ => {}
                        }
                    }
                }
            }
        }
        if !events.is_empty() {
            println!("  Events:         {}", events.join(", "));
        }
    }
}
