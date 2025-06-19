use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

/// Represents the contents of a PID file
#[derive(Debug, Clone)]
struct PidFileData {
    /// Process ID
    pid: u32,
    /// Command that was executed (program + arguments)
    command: Vec<String>,
}

impl PidFileData {
    /// Create a new PidFileData instance
    fn new(pid: u32, command: Vec<String>) -> Self {
        Self { pid, command }
    }

    /// Write PID file data to a file
    fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        writeln!(file, "{}", self.pid)?;
        for arg in &self.command {
            writeln!(file, "{}", arg)?;
        }
        Ok(())
    }

    /// Read PID file data from a file
    fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let lines: Vec<&str> = contents.lines().collect();

        if lines.is_empty() {
            return Err(anyhow::anyhow!("PID file is empty"));
        }

        let pid = lines[0]
            .trim()
            .parse::<u32>()
            .context("Failed to parse PID from first line")?;

        let command: Vec<String> = lines[1..].iter().map(|line| line.to_string()).collect();

        if command.is_empty() {
            return Err(anyhow::anyhow!("No command found in PID file"));
        }

        Ok(Self { pid, command })
    }

    /// Get the command as a formatted string for display
    fn command_string(&self) -> String {
        self.command.join(" ")
    }
}

#[derive(Parser)]
#[command(name = "demon")]
#[command(about = "A daemon process management CLI", long_about = None)]
#[command(version)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Spawn a background process and redirect stdout/stderr to files
    Run(RunArgs),

    /// Stop a running daemon process
    Stop(StopArgs),

    /// Tail daemon logs in real-time
    Tail(TailArgs),

    /// Display daemon log contents
    Cat(CatArgs),

    /// List all running daemon processes
    List(ListArgs),

    /// Check status of a daemon process
    Status(StatusArgs),

    /// Clean up orphaned pid and log files
    Clean,

    /// Output comprehensive usage guide for LLMs
    Llm,
}

#[derive(Args)]
struct RunArgs {
    /// Process identifier
    #[arg(long)]
    id: String,

    /// Command and arguments to execute
    command: Vec<String>,
}

#[derive(Args)]
struct StopArgs {
    /// Process identifier
    #[arg(long)]
    id: String,

    /// Timeout in seconds before sending SIGKILL after SIGTERM
    #[arg(long, default_value = "10")]
    timeout: u64,
}

#[derive(Args)]
struct TailArgs {
    /// Process identifier
    #[arg(long)]
    id: String,

    /// Only tail stdout
    #[arg(long)]
    stdout: bool,

    /// Only tail stderr
    #[arg(long)]
    stderr: bool,
}

#[derive(Args)]
struct CatArgs {
    /// Process identifier
    #[arg(long)]
    id: String,

    /// Only show stdout
    #[arg(long)]
    stdout: bool,

    /// Only show stderr
    #[arg(long)]
    stderr: bool,
}

#[derive(Args)]
struct ListArgs {
    /// Quiet mode - output only process data without headers
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Args)]
struct StatusArgs {
    /// Process identifier
    #[arg(long)]
    id: String,
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    if let Err(e) = run_command(cli.command) {
        tracing::error!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Run(args) => {
            if args.command.is_empty() {
                return Err(anyhow::anyhow!("Command cannot be empty"));
            }
            run_daemon(&args.id, &args.command)
        }
        Commands::Stop(args) => stop_daemon(&args.id, args.timeout),
        Commands::Tail(args) => {
            let show_stdout = !args.stderr || args.stdout;
            let show_stderr = !args.stdout || args.stderr;
            tail_logs(&args.id, show_stdout, show_stderr)
        }
        Commands::Cat(args) => {
            let show_stdout = !args.stderr || args.stdout;
            let show_stderr = !args.stdout || args.stderr;
            cat_logs(&args.id, show_stdout, show_stderr)
        }
        Commands::List(args) => list_daemons(args.quiet),
        Commands::Status(args) => status_daemon(&args.id),
        Commands::Clean => clean_orphaned_files(),
        Commands::Llm => {
            print_llm_guide();
            Ok(())
        }
    }
}

fn run_daemon(id: &str, command: &[String]) -> Result<()> {
    let pid_file = format!("{}.pid", id);
    let stdout_file = format!("{}.stdout", id);
    let stderr_file = format!("{}.stderr", id);

    // Check if process is already running
    if is_process_running(&pid_file)? {
        return Err(anyhow::anyhow!("Process '{}' is already running", id));
    }

    tracing::info!("Starting daemon '{}' with command: {:?}", id, command);

    // Truncate/create output files
    File::create(&stdout_file)?;
    File::create(&stderr_file)?;

    // Open files for redirection
    let stdout_redirect = File::create(&stdout_file)?;
    let stderr_redirect = File::create(&stderr_file)?;

    // Spawn the process
    let program = &command[0];
    let args = if command.len() > 1 {
        &command[1..]
    } else {
        &[]
    };

    let child = Command::new(program)
        .args(args)
        .stdout(Stdio::from(stdout_redirect))
        .stderr(Stdio::from(stderr_redirect))
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to start process '{}' with args {:?}", program, args))?;

    // Write PID and command to file
    let pid_data = PidFileData::new(child.id(), command.to_vec());
    pid_data.write_to_file(&pid_file)?;

    // Don't wait for the child - let it run detached
    std::mem::forget(child);

    println!("Started daemon '{}' with PID written to {}", id, pid_file);

    Ok(())
}

fn is_process_running(pid_file: &str) -> Result<bool> {
    // Try to read the PID file
    if !Path::new(pid_file).exists() {
        return Ok(false); // No PID file means no running process
    }

    let pid_data = match PidFileData::read_from_file(pid_file) {
        Ok(data) => data,
        Err(_) => return Ok(false), // Invalid PID file
    };

    // Check if process is still running using kill -0
    let output = Command::new("kill")
        .args(&["-0", &pid_data.pid.to_string()])
        .output()?;

    Ok(output.status.success())
}

fn stop_daemon(id: &str, timeout: u64) -> Result<()> {
    let pid_file = format!("{}.pid", id);

    // Check if PID file exists and read PID data
    let pid_data = match PidFileData::read_from_file(&pid_file) {
        Ok(data) => data,
        Err(_) => {
            if Path::new(&pid_file).exists() {
                println!("Process '{}': invalid PID file, removing it", id);
                std::fs::remove_file(&pid_file)?;
            } else {
                println!("Process '{}' is not running (no PID file found)", id);
            }
            return Ok(());
        }
    };

    let pid = pid_data.pid;

    tracing::info!(
        "Stopping daemon '{}' (PID: {}) with timeout {}s",
        id,
        pid,
        timeout
    );

    // Check if process is running
    if !is_process_running_by_pid(pid) {
        println!(
            "Process '{}' (PID: {}) is not running, cleaning up PID file",
            id, pid
        );
        std::fs::remove_file(&pid_file)?;
        return Ok(());
    }

    // Send SIGTERM
    tracing::info!("Sending SIGTERM to PID {}", pid);
    let output = Command::new("kill")
        .args(&["-TERM", &pid.to_string()])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to send SIGTERM to PID {}", pid));
    }

    // Wait for the process to terminate
    for i in 0..timeout {
        if !is_process_running_by_pid(pid) {
            println!("Process '{}' (PID: {}) terminated gracefully", id, pid);
            std::fs::remove_file(&pid_file)?;
            return Ok(());
        }

        if i == 0 {
            tracing::info!("Waiting for process to terminate gracefully...");
        }

        thread::sleep(Duration::from_secs(1));
    }

    // Process didn't terminate, send SIGKILL
    tracing::warn!(
        "Process {} didn't terminate after {}s, sending SIGKILL",
        pid,
        timeout
    );
    let output = Command::new("kill")
        .args(&["-KILL", &pid.to_string()])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to send SIGKILL to PID {}", pid));
    }

    // Wait a bit more for SIGKILL to take effect
    thread::sleep(Duration::from_secs(1));

    if is_process_running_by_pid(pid) {
        return Err(anyhow::anyhow!(
            "Process {} is still running after SIGKILL",
            pid
        ));
    }

    println!("Process '{}' (PID: {}) terminated forcefully", id, pid);
    std::fs::remove_file(&pid_file)?;

    Ok(())
}

fn is_process_running_by_pid(pid: u32) -> bool {
    let output = Command::new("kill")
        .args(&["-0", &pid.to_string()])
        .output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn cat_logs(id: &str, show_stdout: bool, show_stderr: bool) -> Result<()> {
    let stdout_file = format!("{}.stdout", id);
    let stderr_file = format!("{}.stderr", id);

    let mut files_found = false;

    if show_stdout {
        if let Ok(contents) = std::fs::read_to_string(&stdout_file) {
            if !contents.is_empty() {
                files_found = true;
                if show_stderr {
                    println!("==> {} <==", stdout_file);
                }
                print!("{}", contents);
            }
        } else {
            tracing::warn!("Could not read {}", stdout_file);
        }
    }

    if show_stderr {
        if let Ok(contents) = std::fs::read_to_string(&stderr_file) {
            if !contents.is_empty() {
                files_found = true;
                if show_stdout {
                    println!("==> {} <==", stderr_file);
                }
                print!("{}", contents);
            }
        } else {
            tracing::warn!("Could not read {}", stderr_file);
        }
    }

    if !files_found {
        println!("No log files found for daemon '{}'", id);
    }

    Ok(())
}

fn tail_logs(id: &str, show_stdout: bool, show_stderr: bool) -> Result<()> {
    let stdout_file = format!("{}.stdout", id);
    let stderr_file = format!("{}.stderr", id);

    // First, display existing content and set up initial positions
    let mut file_positions: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();

    if show_stdout && Path::new(&stdout_file).exists() {
        let mut file = File::open(&stdout_file)?;
        let initial_content = read_file_content(&mut file)?;
        if !initial_content.is_empty() {
            if show_stderr {
                println!("==> {} <==", stdout_file);
            }
            print!("{}", initial_content);
        }
        let position = file.seek(SeekFrom::Current(0))?;
        file_positions.insert(stdout_file.clone(), position);
    }

    if show_stderr && Path::new(&stderr_file).exists() {
        let mut file = File::open(&stderr_file)?;
        let initial_content = read_file_content(&mut file)?;
        if !initial_content.is_empty() {
            if show_stdout && file_positions.len() > 0 {
                println!("\n==> {} <==", stderr_file);
            } else if show_stdout {
                println!("==> {} <==", stderr_file);
            }
            print!("{}", initial_content);
        }
        let position = file.seek(SeekFrom::Current(0))?;
        file_positions.insert(stderr_file.clone(), position);
    }

    if file_positions.is_empty() {
        println!(
            "No log files found for daemon '{}'. Watching for new files...",
            id
        );
    }

    tracing::info!("Watching for changes to log files... Press Ctrl+C to stop.");

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    // Watch the current directory for new files and changes
    watcher.watch(Path::new("."), RecursiveMode::NonRecursive)?;

    // Handle Ctrl+C gracefully
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })?;

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(res) => {
                match res {
                    Ok(Event {
                        kind: EventKind::Modify(_),
                        paths,
                        ..
                    }) => {
                        for path in paths {
                            let path_str = path.to_string_lossy().to_string();

                            if (show_stdout && path_str == stdout_file)
                                || (show_stderr && path_str == stderr_file)
                            {
                                if let Err(e) = handle_file_change(
                                    &path_str,
                                    &mut file_positions,
                                    show_stdout && show_stderr,
                                ) {
                                    tracing::error!("Error handling file change: {}", e);
                                }
                            }
                        }
                    }
                    Ok(Event {
                        kind: EventKind::Create(_),
                        paths,
                        ..
                    }) => {
                        // Handle file creation
                        for path in paths {
                            let path_str = path.to_string_lossy().to_string();

                            if (show_stdout && path_str == stdout_file)
                                || (show_stderr && path_str == stderr_file)
                            {
                                tracing::info!("New file detected: {}", path_str);
                                file_positions.insert(path_str.clone(), 0);

                                if let Err(e) = handle_file_change(
                                    &path_str,
                                    &mut file_positions,
                                    show_stdout && show_stderr,
                                ) {
                                    tracing::error!("Error handling new file: {}", e);
                                }
                            }
                        }
                    }
                    Ok(_) => {} // Ignore other events
                    Err(e) => tracing::error!("Watch error: {:?}", e),
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Timeout is normal, just continue
            }
            Err(e) => {
                tracing::error!("Receive error: {}", e);
                break;
            }
        }
    }

    println!("\nTailing stopped.");
    Ok(())
}

fn read_file_content(file: &mut File) -> Result<String> {
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn handle_file_change(
    file_path: &str,
    positions: &mut std::collections::HashMap<String, u64>,
    show_headers: bool,
) -> Result<()> {
    let mut file = File::open(file_path)?;
    let current_pos = positions.get(file_path).copied().unwrap_or(0);

    // Seek to the last read position
    file.seek(SeekFrom::Start(current_pos))?;

    // Read new content
    let mut new_content = String::new();
    file.read_to_string(&mut new_content)?;

    if !new_content.is_empty() {
        if show_headers {
            println!("==> {} <==", file_path);
        }
        print!("{}", new_content);
        std::io::Write::flush(&mut std::io::stdout())?;

        // Update position
        let new_pos = file.seek(SeekFrom::Current(0))?;
        positions.insert(file_path.to_string(), new_pos);
    }

    Ok(())
}

fn list_daemons(quiet: bool) -> Result<()> {
    if !quiet {
        println!("{:<20} {:<8} {:<10} {}", "ID", "PID", "STATUS", "COMMAND");
        println!("{}", "-".repeat(50));
    }

    let mut found_any = false;

    // Find all .pid files in current directory
    for entry in find_pid_files()? {
        found_any = true;
        let path = entry.path();
        let path_str = path.to_string_lossy();

        // Extract ID from filename (remove .pid extension)
        let id = path_str.strip_suffix(".pid").unwrap_or(&path_str);

        // Read PID data from file
        match PidFileData::read_from_file(&path) {
            Ok(pid_data) => {
                let status = if is_process_running_by_pid(pid_data.pid) {
                    "RUNNING"
                } else {
                    "DEAD"
                };

                if quiet {
                    println!("{}:{}:{}", id, pid_data.pid, status);
                } else {
                    let command = pid_data.command_string();
                    println!("{:<20} {:<8} {:<10} {}", id, pid_data.pid, status, command);
                }
            }
            Err(_) => {
                if quiet {
                    println!("{}:INVALID:ERROR", id);
                } else {
                    println!(
                        "{:<20} {:<8} {:<10} {}",
                        id, "INVALID", "ERROR", "Invalid PID file"
                    );
                }
            }
        }
    }

    if !found_any && !quiet {
        println!("No daemon processes found.");
    }

    Ok(())
}

fn status_daemon(id: &str) -> Result<()> {
    let pid_file = format!("{}.pid", id);
    let stdout_file = format!("{}.stdout", id);
    let stderr_file = format!("{}.stderr", id);

    println!("Daemon: {}", id);
    println!("PID file: {}", pid_file);

    // Check if PID file exists
    if !Path::new(&pid_file).exists() {
        println!("Status: NOT FOUND (no PID file)");
        return Ok(());
    }

    // Read PID data from file
    match PidFileData::read_from_file(&pid_file) {
        Ok(pid_data) => {
            println!("PID: {}", pid_data.pid);
            println!("Command: {}", pid_data.command_string());

            if is_process_running_by_pid(pid_data.pid) {
                println!("Status: RUNNING");

                // Show file information
                if Path::new(&stdout_file).exists() {
                    let metadata = std::fs::metadata(&stdout_file)?;
                    println!("Stdout file: {} ({} bytes)", stdout_file, metadata.len());
                } else {
                    println!("Stdout file: {} (not found)", stdout_file);
                }

                if Path::new(&stderr_file).exists() {
                    let metadata = std::fs::metadata(&stderr_file)?;
                    println!("Stderr file: {} ({} bytes)", stderr_file, metadata.len());
                } else {
                    println!("Stderr file: {} (not found)", stderr_file);
                }
            } else {
                println!("Status: DEAD (process not running)");
                println!("Note: Use 'demon clean' to remove orphaned files");
            }
        }
        Err(e) => {
            println!("Status: ERROR (cannot read PID file: {})", e);
        }
    }

    Ok(())
}

fn clean_orphaned_files() -> Result<()> {
    tracing::info!("Scanning for orphaned daemon files...");

    let mut cleaned_count = 0;

    // Find all .pid files in current directory
    for entry in find_pid_files()? {
        let path = entry.path();
        let path_str = path.to_string_lossy();
        let id = path_str.strip_suffix(".pid").unwrap_or(&path_str);

        // Read PID data from file
        match PidFileData::read_from_file(&path) {
            Ok(pid_data) => {
                // Check if process is still running
                if !is_process_running_by_pid(pid_data.pid) {
                    println!(
                        "Cleaning up orphaned files for '{}' (PID: {})",
                        id, pid_data.pid
                    );

                    // Remove PID file
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::warn!("Failed to remove {}: {}", path_str, e);
                    } else {
                        tracing::info!("Removed {}", path_str);
                    }

                    // Remove stdout file if it exists
                    let stdout_file = format!("{}.stdout", id);
                    if Path::new(&stdout_file).exists() {
                        if let Err(e) = std::fs::remove_file(&stdout_file) {
                            tracing::warn!("Failed to remove {}: {}", stdout_file, e);
                        } else {
                            tracing::info!("Removed {}", stdout_file);
                        }
                    }

                    // Remove stderr file if it exists
                    let stderr_file = format!("{}.stderr", id);
                    if Path::new(&stderr_file).exists() {
                        if let Err(e) = std::fs::remove_file(&stderr_file) {
                            tracing::warn!("Failed to remove {}: {}", stderr_file, e);
                        } else {
                            tracing::info!("Removed {}", stderr_file);
                        }
                    }

                    cleaned_count += 1;
                } else {
                    tracing::info!(
                        "Skipping '{}' (PID: {}) - process is still running",
                        id,
                        pid_data.pid
                    );
                }
            }
            Err(_) => {
                println!("Cleaning up invalid PID file: {}", path_str);
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Failed to remove invalid PID file {}: {}", path_str, e);
                } else {
                    tracing::info!("Removed invalid PID file {}", path_str);
                    cleaned_count += 1;
                }
            }
        }
    }

    if cleaned_count == 0 {
        println!("No orphaned files found.");
    } else {
        println!("Cleaned up {} orphaned daemon(s).", cleaned_count);
    }

    Ok(())
}

fn print_llm_guide() {
    println!(
        r#"# Demon - Daemon Process Management CLI

## Overview
Demon is a command-line tool for spawning, managing, and monitoring background processes (daemons) on Linux systems. It redirects process stdout/stderr to files and provides commands to control and observe these processes.

## Core Concept
- Each daemon is identified by a unique string ID
- Three files are created per daemon: `<id>.pid`, `<id>.stdout`, `<id>.stderr`
- Files are created in the current working directory
- Processes run detached from the parent shell

## Available Commands

### demon run --id <identifier> <command...>
Spawns a background process with the given identifier.

**Syntax**: `demon run --id <id> [--] <command> [args...]`

**Behavior**:
- Creates `<id>.pid`, `<id>.stdout`, `<id>.stderr` files
- Truncates log files if they already exist
- Fails if a process with the same ID is already running
- Parent process exits immediately, child continues in background
- Use `--` to separate flags from command when command has flags

**Examples**:
```bash
demon run --id web-server python -m http.server 8080
demon run --id backup-job -- rsync -av /data/ /backup/
demon run --id log-monitor tail -f /var/log/app.log
```

### demon stop --id <id> [--timeout <seconds>]
Stops a running daemon process gracefully.

**Syntax**: `demon stop --id <id> [--timeout <seconds>]`

**Behavior**:
- Sends SIGTERM to the process first
- Waits for specified timeout (default: 10 seconds)
- Sends SIGKILL if process doesn't terminate
- Removes PID file after successful termination
- Handles already-dead processes gracefully

**Examples**:
```bash
demon stop --id web-server
demon stop --id backup-job --timeout 30
```

### demon list [--quiet]
Lists all managed daemon processes and their status.

**Syntax**: `demon list [-q|--quiet]`

**Normal Output Format**:
```
ID                   PID      STATUS     COMMAND
--------------------------------------------------
web-server           12345    RUNNING    N/A
backup-job           12346    DEAD       N/A
```

**Quiet Output Format** (machine-readable):
```
web-server:12345:RUNNING
backup-job:12346:DEAD
```

**Status Values**:
- `RUNNING`: Process is actively running
- `DEAD`: Process has terminated, files still exist

### demon status --id <id>
Shows detailed status information for a specific daemon.

**Syntax**: `demon status --id <id>`

**Output includes**:
- Daemon ID and PID file location
- Process ID (if available)
- Current status (RUNNING/DEAD/NOT FOUND/ERROR)
- Log file locations and sizes
- Suggestions for cleanup if needed

**Example**:
```bash
demon status --id web-server
```

### demon cat --id <id> [--stdout] [--stderr]
Displays the contents of daemon log files.

**Syntax**: `demon cat --id <id> [--stdout] [--stderr]`

**Behavior**:
- Shows both stdout and stderr by default
- Use flags to show only specific streams
- Displays file headers when showing multiple files
- Handles missing files gracefully

**Examples**:
```bash
demon cat --id web-server           # Show both logs
demon cat --id web-server --stdout  # Show only stdout
demon cat --id web-server --stderr  # Show only stderr
```

### demon tail --id <id> [--stdout] [--stderr]
Follows daemon log files in real-time (like `tail -f`).

**Syntax**: `demon tail --id <id> [--stdout] [--stderr]`

**Behavior**:
- Shows existing content first, then follows new content
- Shows both stdout and stderr by default
- Uses file system notifications for efficient monitoring
- Press Ctrl+C to stop tailing
- Handles file creation, rotation, and truncation

**Examples**:
```bash
demon tail --id web-server           # Follow both logs
demon tail --id web-server --stdout  # Follow only stdout
```

### demon clean
Removes orphaned files from processes that are no longer running.

**Syntax**: `demon clean`

**Behavior**:
- Scans for `.pid` files in current directory
- Checks if corresponding processes are still running
- Removes `.pid`, `.stdout`, `.stderr` files for dead processes
- Handles invalid PID files gracefully
- Reports what was cleaned up

**Example**:
```bash
demon clean
```

## File Management

### Created Files
For each daemon with ID "example":
- `example.pid`: Contains the process ID
- `example.stdout`: Contains standard output from the process
- `example.stderr`: Contains standard error from the process

### File Locations
All files are created in the current working directory where `demon run` is executed.

### Cleanup
- Files persist after process termination for inspection
- Use `demon clean` to remove files from dead processes
- Consider adding `*.pid`, `*.stdout`, `*.stderr` to `.gitignore`

## Common Workflows

### Starting a Web Server
```bash
demon run --id my-web-server python -m http.server 8080
demon status --id my-web-server  # Check if it started
demon tail --id my-web-server    # Monitor logs
```

### Running a Backup Job
```bash
demon run --id nightly-backup -- rsync -av /data/ /backup/
demon cat --id nightly-backup   # Check output when done
demon clean                     # Clean up after completion
```

### Managing Multiple Services
```bash
demon run --id api-server ./api --port 3000
demon run --id worker-queue ./worker --config prod.conf
demon list                      # See all running services
demon stop --id api-server      # Stop specific service
```

### Monitoring and Debugging
```bash
demon list --quiet | grep RUNNING  # Machine-readable active processes
demon tail --id problematic-app --stderr  # Monitor just errors
demon status --id failing-service         # Get detailed status
```

## Error Handling

### Common Error Scenarios
- **"Process already running"**: Another process with the same ID exists
- **"Command cannot be empty"**: No command specified after `--id`
- **"Process not found"**: No PID file exists for the given ID
- **"Failed to start process"**: Command not found or permission denied

### Best Practices
1. Use descriptive, unique IDs for each daemon
2. Check status before starting to avoid conflicts
3. Use `demon clean` periodically to remove old files
4. Monitor logs with `demon tail` for debugging
5. Use `--timeout` with stop for processes that may take time to shutdown

## Integration Tips

### Scripting
```bash
# Check if service is running
if demon status --id my-service | grep -q "RUNNING"; then
    echo "Service is running"
fi

# Start service if not running
demon list --quiet | grep -q "my-service:" || demon run --id my-service ./my-app

# Get machine-readable process list
demon list --quiet > process_status.txt
```

### Process Management
- Demon handles process detachment automatically
- Processes continue running even if demon exits
- Use standard Unix signals for process control
- Log rotation should be handled by the application itself

This tool is designed for Linux environments and provides a simple interface for managing background processes with persistent logging."#
    );
}

fn find_pid_files() -> Result<Vec<std::fs::DirEntry>> {
    let entries = std::fs::read_dir(".")?
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .filter(|ext| *ext == "pid")
                    .map(|_| e)
            })
        })
        .collect();
    Ok(entries)
}
