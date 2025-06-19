use clap::{Parser, Subcommand, Args};
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::path::Path;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use glob::glob;

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
    List,
    
    /// Check status of a daemon process
    Status(StatusArgs),
    
    /// Clean up orphaned pid and log files
    Clean,
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

fn run_command(command: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::Run(args) => {
            if args.command.is_empty() {
                return Err("Command cannot be empty".into());
            }
            run_daemon(&args.id, &args.command)
        }
        Commands::Stop(args) => {
            stop_daemon(&args.id, args.timeout)
        }
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
        Commands::List => {
            list_daemons()
        }
        Commands::Status(args) => {
            status_daemon(&args.id)
        }
        Commands::Clean => {
            clean_orphaned_files()
        }
    }
}

fn run_daemon(id: &str, command: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let pid_file = format!("{}.pid", id);
    let stdout_file = format!("{}.stdout", id);
    let stderr_file = format!("{}.stderr", id);
    
    // Check if process is already running
    if is_process_running(&pid_file)? {
        return Err(format!("Process '{}' is already running", id).into());
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
    let args = if command.len() > 1 { &command[1..] } else { &[] };
    
    let child = Command::new(program)
        .args(args)
        .stdout(Stdio::from(stdout_redirect))
        .stderr(Stdio::from(stderr_redirect))
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start process '{}': {}", program, e))?;
    
    // Write PID to file
    let mut pid_file_handle = File::create(&pid_file)?;
    writeln!(pid_file_handle, "{}", child.id())?;
    
    // Don't wait for the child - let it run detached
    std::mem::forget(child);
    
    println!("Started daemon '{}' with PID written to {}", id, pid_file);
    
    Ok(())
}

fn is_process_running(pid_file: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // Try to read the PID file
    let mut file = match File::open(pid_file) {
        Ok(f) => f,
        Err(_) => return Ok(false), // No PID file means no running process
    };
    
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let pid: u32 = match contents.trim().parse() {
        Ok(p) => p,
        Err(_) => return Ok(false), // Invalid PID file
    };
    
    // Check if process is still running using kill -0
    let output = Command::new("kill")
        .args(&["-0", &pid.to_string()])
        .output()?;
    
    Ok(output.status.success())
}

fn stop_daemon(id: &str, timeout: u64) -> Result<(), Box<dyn std::error::Error>> {
    let pid_file = format!("{}.pid", id);
    
    // Check if PID file exists
    let mut file = match File::open(&pid_file) {
        Ok(f) => f,
        Err(_) => {
            println!("Process '{}' is not running (no PID file found)", id);
            return Ok(());
        }
    };
    
    // Read PID
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let pid: u32 = match contents.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            println!("Process '{}': invalid PID file, removing it", id);
            std::fs::remove_file(&pid_file)?;
            return Ok(());
        }
    };
    
    tracing::info!("Stopping daemon '{}' (PID: {}) with timeout {}s", id, pid, timeout);
    
    // Check if process is running
    if !is_process_running_by_pid(pid) {
        println!("Process '{}' (PID: {}) is not running, cleaning up PID file", id, pid);
        std::fs::remove_file(&pid_file)?;
        return Ok(());
    }
    
    // Send SIGTERM
    tracing::info!("Sending SIGTERM to PID {}", pid);
    let output = Command::new("kill")
        .args(&["-TERM", &pid.to_string()])
        .output()?;
    
    if !output.status.success() {
        return Err(format!("Failed to send SIGTERM to PID {}", pid).into());
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
    tracing::warn!("Process {} didn't terminate after {}s, sending SIGKILL", pid, timeout);
    let output = Command::new("kill")
        .args(&["-KILL", &pid.to_string()])
        .output()?;
    
    if !output.status.success() {
        return Err(format!("Failed to send SIGKILL to PID {}", pid).into());
    }
    
    // Wait a bit more for SIGKILL to take effect
    thread::sleep(Duration::from_secs(1));
    
    if is_process_running_by_pid(pid) {
        return Err(format!("Process {} is still running after SIGKILL", pid).into());
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

fn cat_logs(id: &str, show_stdout: bool, show_stderr: bool) -> Result<(), Box<dyn std::error::Error>> {
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

fn tail_logs(id: &str, show_stdout: bool, show_stderr: bool) -> Result<(), Box<dyn std::error::Error>> {
    let stdout_file = format!("{}.stdout", id);
    let stderr_file = format!("{}.stderr", id);
    
    // First, display existing content and set up initial positions
    let mut file_positions: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    
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
        println!("No log files found for daemon '{}'. Watching for new files...", id);
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
                            
                            if (show_stdout && path_str == stdout_file) || 
                               (show_stderr && path_str == stderr_file) {
                                
                                if let Err(e) = handle_file_change(&path_str, &mut file_positions, show_stdout && show_stderr) {
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
                            
                            if (show_stdout && path_str == stdout_file) || 
                               (show_stderr && path_str == stderr_file) {
                                
                                tracing::info!("New file detected: {}", path_str);
                                file_positions.insert(path_str.clone(), 0);
                                
                                if let Err(e) = handle_file_change(&path_str, &mut file_positions, show_stdout && show_stderr) {
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

fn read_file_content(file: &mut File) -> Result<String, Box<dyn std::error::Error>> {
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

fn handle_file_change(
    file_path: &str, 
    positions: &mut std::collections::HashMap<String, u64>,
    show_headers: bool
) -> Result<(), Box<dyn std::error::Error>> {
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

fn list_daemons() -> Result<(), Box<dyn std::error::Error>> {
    println!("{:<20} {:<8} {:<10} {}", "ID", "PID", "STATUS", "COMMAND");
    println!("{}", "-".repeat(50));
    
    let mut found_any = false;
    
    // Find all .pid files in current directory
    for entry in glob("*.pid")? {
        match entry {
            Ok(path) => {
                found_any = true;
                let path_str = path.to_string_lossy();
                
                // Extract ID from filename (remove .pid extension)
                let id = path_str.strip_suffix(".pid").unwrap_or(&path_str);
                
                // Read PID from file
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        let pid_str = contents.trim();
                        match pid_str.parse::<u32>() {
                            Ok(pid) => {
                                let status = if is_process_running_by_pid(pid) {
                                    "RUNNING"
                                } else {
                                    "DEAD"
                                };
                                
                                // Try to read command from a hypothetical command file
                                // For now, we'll just show "N/A" since we don't store the command
                                let command = "N/A";
                                
                                println!("{:<20} {:<8} {:<10} {}", id, pid, status, command);
                            }
                            Err(_) => {
                                println!("{:<20} {:<8} {:<10} {}", id, "INVALID", "ERROR", "Invalid PID file");
                            }
                        }
                    }
                    Err(e) => {
                        println!("{:<20} {:<8} {:<10} {}", id, "ERROR", "ERROR", format!("Cannot read: {}", e));
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Error reading glob entry: {}", e);
            }
        }
    }
    
    if !found_any {
        println!("No daemon processes found.");
    }
    
    Ok(())
}

fn status_daemon(id: &str) -> Result<(), Box<dyn std::error::Error>> {
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
    
    // Read PID from file
    match std::fs::read_to_string(&pid_file) {
        Ok(contents) => {
            let pid_str = contents.trim();
            match pid_str.parse::<u32>() {
                Ok(pid) => {
                    println!("PID: {}", pid);
                    
                    if is_process_running_by_pid(pid) {
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
                Err(_) => {
                    println!("Status: ERROR (invalid PID in file)");
                }
            }
        }
        Err(e) => {
            println!("Status: ERROR (cannot read PID file: {})", e);
        }
    }
    
    Ok(())
}

fn clean_orphaned_files() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Scanning for orphaned daemon files...");
    
    let mut cleaned_count = 0;
    
    // Find all .pid files in current directory
    for entry in glob("*.pid")? {
        match entry {
            Ok(path) => {
                let path_str = path.to_string_lossy();
                let id = path_str.strip_suffix(".pid").unwrap_or(&path_str);
                
                // Read PID from file
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        let pid_str = contents.trim();
                        match pid_str.parse::<u32>() {
                            Ok(pid) => {
                                // Check if process is still running
                                if !is_process_running_by_pid(pid) {
                                    println!("Cleaning up orphaned files for '{}' (PID: {})", id, pid);
                                    
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
                                    tracing::info!("Skipping '{}' (PID: {}) - process is still running", id, pid);
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
                    Err(_) => {
                        println!("Cleaning up unreadable PID file: {}", path_str);
                        if let Err(e) = std::fs::remove_file(&path) {
                            tracing::warn!("Failed to remove unreadable PID file {}: {}", path_str, e);
                        } else {
                            tracing::info!("Removed unreadable PID file {}", path_str);
                            cleaned_count += 1;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Error reading glob entry: {}", e);
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
