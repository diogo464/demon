use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

/// Error types for reading PID files
#[derive(Debug)]
pub enum PidFileReadError {
    /// The PID file does not exist
    FileNotFound,
    /// The PID file exists but has invalid content
    FileInvalid(String),
    /// IO error occurred while reading
    IoError(std::io::Error),
}

impl std::fmt::Display for PidFileReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PidFileReadError::FileNotFound => write!(f, "PID file not found"),
            PidFileReadError::FileInvalid(reason) => write!(f, "PID file invalid: {}", reason),
            PidFileReadError::IoError(err) => write!(f, "IO error reading PID file: {}", err),
        }
    }
}

impl std::error::Error for PidFileReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PidFileReadError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

/// Represents the contents of a PID file
#[derive(Debug, Clone)]
struct PidFile {
    /// Process ID
    pid: u32,
    /// Command that was executed (program + arguments)
    command: Vec<String>,
}

impl PidFile {
    /// Create a new PidFile instance
    fn new(pid: u32, command: Vec<String>) -> Self {
        Self { pid, command }
    }

    /// Write PID file to a file
    fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        writeln!(file, "{}", self.pid)?;
        for arg in &self.command {
            writeln!(file, "{}", arg)?;
        }
        Ok(())
    }

    /// Read PID file from a file
    fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self, PidFileReadError> {
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                return if err.kind() == std::io::ErrorKind::NotFound {
                    Err(PidFileReadError::FileNotFound)
                } else {
                    Err(PidFileReadError::IoError(err))
                };
            }
        };

        let lines: Vec<&str> = contents.lines().collect();

        if lines.is_empty() {
            return Err(PidFileReadError::FileInvalid(
                "PID file is empty".to_string(),
            ));
        }

        let pid = lines[0]
            .trim()
            .parse::<u32>()
            .map_err(|_| PidFileReadError::FileInvalid("Invalid PID on first line".to_string()))?;

        let command: Vec<String> = lines[1..].iter().map(|line| line.to_string()).collect();

        if command.is_empty() {
            return Err(PidFileReadError::FileInvalid(
                "No command found in PID file".to_string(),
            ));
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

#[derive(Args)]
struct Global {
    /// Root directory for daemon files (pid, logs). If not specified, searches for git root.
    #[arg(long, global = true, env = "DEMON_ROOT_DIR")]
    root_dir: Option<PathBuf>,
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
    Clean(CleanArgs),

    /// Output comprehensive usage guide for LLMs
    Llm,

    /// Wait for a daemon process to terminate
    Wait(WaitArgs),
}

#[derive(Args)]
struct RunArgs {
    #[clap(flatten)]
    global: Global,

    /// Process identifier
    id: String,

    /// Command and arguments to execute
    command: Vec<String>,
}

#[derive(Args)]
struct StopArgs {
    #[clap(flatten)]
    global: Global,

    /// Process identifier
    id: String,

    /// Timeout in seconds before sending SIGKILL after SIGTERM
    #[arg(long, default_value = "10")]
    timeout: u64,
}

#[derive(Args)]
struct TailArgs {
    #[clap(flatten)]
    global: Global,

    /// Process identifier
    id: String,

    /// Only tail stdout
    #[arg(long)]
    stdout: bool,

    /// Only tail stderr
    #[arg(long)]
    stderr: bool,

    /// Follow mode - continuously watch for new content (like tail -f)
    #[arg(short = 'f', long)]
    follow: bool,

    /// Number of lines to display from the end (default: 50)
    #[arg(short = 'n', long, default_value = "50")]
    lines: usize,
}

#[derive(Args)]
struct CatArgs {
    #[clap(flatten)]
    global: Global,

    /// Process identifier
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
    #[clap(flatten)]
    global: Global,

    /// Quiet mode - output only process data without headers
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Args)]
struct StatusArgs {
    #[clap(flatten)]
    global: Global,

    /// Process identifier
    id: String,
}

#[derive(Args)]
struct CleanArgs {
    #[clap(flatten)]
    global: Global,
}

#[derive(Args)]
struct WaitArgs {
    #[clap(flatten)]
    global: Global,

    /// Process identifier
    id: String,

    /// Timeout in seconds (0 = infinite)
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Polling interval in seconds
    #[arg(long, default_value = "1")]
    interval: u64,
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
            let root_dir = resolve_root_dir(&args.global)?;
            run_daemon(&args.id, &args.command, &root_dir)
        }
        Commands::Stop(args) => {
            let root_dir = resolve_root_dir(&args.global)?;
            stop_daemon(&args.id, args.timeout, &root_dir)
        }
        Commands::Tail(args) => {
            let show_stdout = !args.stderr || args.stdout;
            let show_stderr = !args.stdout || args.stderr;
            let root_dir = resolve_root_dir(&args.global)?;
            tail_logs(&args.id, show_stdout, show_stderr, args.follow, args.lines, &root_dir)
        }
        Commands::Cat(args) => {
            let show_stdout = !args.stderr || args.stdout;
            let show_stderr = !args.stdout || args.stderr;
            let root_dir = resolve_root_dir(&args.global)?;
            cat_logs(&args.id, show_stdout, show_stderr, &root_dir)
        }
        Commands::List(args) => {
            let root_dir = resolve_root_dir(&args.global)?;
            list_daemons(args.quiet, &root_dir)
        }
        Commands::Status(args) => {
            let root_dir = resolve_root_dir(&args.global)?;
            status_daemon(&args.id, &root_dir)
        }
        Commands::Clean(args) => {
            let root_dir = resolve_root_dir(&args.global)?;
            clean_orphaned_files(&root_dir)
        }
        Commands::Llm => {
            print_llm_guide();
            Ok(())
        }
        Commands::Wait(args) => {
            let root_dir = resolve_root_dir(&args.global)?;
            wait_daemon(&args.id, args.timeout, args.interval, &root_dir)
        }
    }
}

fn find_git_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;
    
    loop {
        let git_path = current.join(".git");
        if git_path.exists() {
            return Ok(current);
        }
        
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return Err(anyhow::anyhow!(
                "No git repository found. Please specify --root-dir or run from within a git repository"
            )),
        }
    }
}

fn resolve_root_dir(global: &Global) -> Result<PathBuf> {
    match &global.root_dir {
        Some(dir) => {
            if !dir.exists() {
                return Err(anyhow::anyhow!("Specified root directory does not exist: {}", dir.display()));
            }
            if !dir.is_dir() {
                return Err(anyhow::anyhow!("Specified root path is not a directory: {}", dir.display()));
            }
            Ok(dir.clone())
        },
        None => find_git_root(),
    }
}

fn build_file_path(root_dir: &Path, id: &str, extension: &str) -> PathBuf {
    root_dir.join(format!("{}.{}", id, extension))
}

fn run_daemon(id: &str, command: &[String], root_dir: &Path) -> Result<()> {
    let pid_file = build_file_path(root_dir, id, "pid");
    let stdout_file = build_file_path(root_dir, id, "stdout");
    let stderr_file = build_file_path(root_dir, id, "stderr");

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
    let pid_file_data = PidFile::new(child.id(), command.to_vec());
    pid_file_data.write_to_file(&pid_file)?;

    // Don't wait for the child - let it run detached
    std::mem::forget(child);

    println!("Started daemon '{}' with PID written to {}", id, pid_file.display());

    Ok(())
}

fn is_process_running<P: AsRef<Path>>(pid_file: P) -> Result<bool> {
    let pid_file_data = match PidFile::read_from_file(pid_file) {
        Ok(data) => data,
        Err(PidFileReadError::FileNotFound) => return Ok(false), // No PID file means no running process
        Err(PidFileReadError::FileInvalid(_)) => return Ok(false), // Invalid PID file means no running process
        Err(PidFileReadError::IoError(err)) => return Err(err.into()), // Propagate IO errors
    };

    // Check if process is still running using kill -0
    let output = Command::new("kill")
        .args(&["-0", &pid_file_data.pid.to_string()])
        .output()?;

    Ok(output.status.success())
}

fn stop_daemon(id: &str, timeout: u64, root_dir: &Path) -> Result<()> {
    let pid_file = build_file_path(root_dir, id, "pid");

    // Check if PID file exists and read PID data
    let pid_file_data = match PidFile::read_from_file(&pid_file) {
        Ok(data) => data,
        Err(PidFileReadError::FileNotFound) => {
            println!("Process '{}' is not running (no PID file found)", id);
            return Ok(());
        }
        Err(PidFileReadError::FileInvalid(_)) => {
            println!("Process '{}': invalid PID file, removing it", id);
            std::fs::remove_file(&pid_file)?;
            return Ok(());
        }
        Err(PidFileReadError::IoError(err)) => {
            return Err(anyhow::anyhow!("Failed to read PID file: {}", err));
        }
    };

    let pid = pid_file_data.pid;

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

fn cat_logs(id: &str, show_stdout: bool, show_stderr: bool, root_dir: &Path) -> Result<()> {
    let stdout_file = build_file_path(root_dir, id, "stdout");
    let stderr_file = build_file_path(root_dir, id, "stderr");

    let mut files_found = false;

    if show_stdout {
        if let Ok(contents) = std::fs::read_to_string(&stdout_file) {
            if !contents.is_empty() {
                files_found = true;
                if show_stderr {
                    println!("==> {} <==", stdout_file.display());
                }
                print!("{}", contents);
            }
        } else {
            tracing::warn!("Could not read {}", stdout_file.display());
        }
    }

    if show_stderr {
        if let Ok(contents) = std::fs::read_to_string(&stderr_file) {
            if !contents.is_empty() {
                files_found = true;
                if show_stdout {
                    println!("==> {} <==", stderr_file.display());
                }
                print!("{}", contents);
            }
        } else {
            tracing::warn!("Could not read {}", stderr_file.display());
        }
    }

    if !files_found {
        println!("No log files found for daemon '{}'", id);
    }

    Ok(())
}

fn tail_logs(
    id: &str,
    show_stdout: bool,
    show_stderr: bool,
    follow: bool,
    lines: usize,
    root_dir: &Path,
) -> Result<()> {
    let stdout_file = build_file_path(root_dir, id, "stdout");
    let stderr_file = build_file_path(root_dir, id, "stderr");

    if !follow {
        // Non-follow mode: just show the last n lines and exit
        let mut files_found = false;

        if show_stdout && stdout_file.exists() {
            let content = read_last_n_lines(&stdout_file, lines)?;
            if !content.is_empty() {
                files_found = true;
                if show_stderr {
                    println!("==> {} <==", stdout_file.display());
                }
                print!("{}", content);
            }
        }

        if show_stderr && stderr_file.exists() {
            let content = read_last_n_lines(&stderr_file, lines)?;
            if !content.is_empty() {
                files_found = true;
                if show_stdout {
                    println!("==> {} <==", stderr_file.display());
                }
                print!("{}", content);
            }
        }

        if !files_found {
            println!("No log files found for daemon '{}'", id);
        }

        return Ok(());
    }

    // Follow mode: original real-time monitoring behavior
    let mut file_positions: std::collections::HashMap<PathBuf, u64> =
        std::collections::HashMap::new();

    if show_stdout && stdout_file.exists() {
        let mut file = File::open(&stdout_file)?;
        let initial_content = read_file_content(&mut file)?;
        if !initial_content.is_empty() {
            if show_stderr {
                println!("==> {} <==", stdout_file.display());
            }
            print!("{}", initial_content);
        }
        let position = file.seek(SeekFrom::Current(0))?;
        file_positions.insert(stdout_file.clone(), position);
    }

    if show_stderr && stderr_file.exists() {
        let mut file = File::open(&stderr_file)?;
        let initial_content = read_file_content(&mut file)?;
        if !initial_content.is_empty() {
            if show_stdout && file_positions.len() > 0 {
                println!("\n==> {} <==", stderr_file.display());
            } else if show_stdout {
                println!("==> {} <==", stderr_file.display());
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

    // Watch the root directory for new files and changes
    watcher.watch(root_dir, RecursiveMode::NonRecursive)?;

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
                            if (show_stdout && path == stdout_file)
                                || (show_stderr && path == stderr_file)
                            {
                                if let Err(e) = handle_file_change(
                                    &path,
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
                            if (show_stdout && path == stdout_file)
                                || (show_stderr && path == stderr_file)
                            {
                                tracing::info!("New file detected: {}", path.display());
                                file_positions.insert(path.clone(), 0);

                                if let Err(e) = handle_file_change(
                                    &path,
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

fn read_last_n_lines<P: AsRef<Path>>(file_path: P, n: usize) -> Result<String> {
    let content = std::fs::read_to_string(file_path)?;
    if content.is_empty() {
        return Ok(String::new());
    }

    let lines: Vec<&str> = content.lines().collect();
    let start_index = if lines.len() > n { lines.len() - n } else { 0 };

    let last_lines: Vec<&str> = lines[start_index..].to_vec();
    Ok(last_lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" })
}

fn handle_file_change(
    file_path: &Path,
    positions: &mut std::collections::HashMap<PathBuf, u64>,
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
            println!("==> {} <==", file_path.display());
        }
        print!("{}", new_content);
        std::io::Write::flush(&mut std::io::stdout())?;

        // Update position
        let new_pos = file.seek(SeekFrom::Current(0))?;
        positions.insert(file_path.to_path_buf(), new_pos);
    }

    Ok(())
}

fn list_daemons(quiet: bool, root_dir: &Path) -> Result<()> {
    if !quiet {
        println!("{:<20} {:<8} {:<10} {}", "ID", "PID", "STATUS", "COMMAND");
        println!("{}", "-".repeat(50));
    }

    let mut found_any = false;

    // Find all .pid files in root directory
    for entry in find_pid_files(root_dir)? {
        found_any = true;
        let path = entry.path();
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        // Extract ID from filename (remove .pid extension)
        let id = filename.strip_suffix(".pid").unwrap_or(filename);

        // Read PID data from file
        match PidFile::read_from_file(&path) {
            Ok(pid_file_data) => {
                let status = if is_process_running_by_pid(pid_file_data.pid) {
                    "RUNNING"
                } else {
                    "DEAD"
                };

                if quiet {
                    println!("{}:{}:{}", id, pid_file_data.pid, status);
                } else {
                    let command = pid_file_data.command_string();
                    println!(
                        "{:<20} {:<8} {:<10} {}",
                        id, pid_file_data.pid, status, command
                    );
                }
            }
            Err(PidFileReadError::FileNotFound) => {
                // This shouldn't happen since we found the file, but handle gracefully
                if quiet {
                    println!("{}:NOTFOUND:ERROR", id);
                } else {
                    println!(
                        "{:<20} {:<8} {:<10} {}",
                        id, "NOTFOUND", "ERROR", "PID file disappeared"
                    );
                }
            }
            Err(PidFileReadError::FileInvalid(reason)) => {
                if quiet {
                    println!("{}:INVALID:ERROR", id);
                } else {
                    println!("{:<20} {:<8} {:<10} {}", id, "INVALID", "ERROR", reason);
                }
            }
            Err(PidFileReadError::IoError(_)) => {
                if quiet {
                    println!("{}:ERROR:ERROR", id);
                } else {
                    println!(
                        "{:<20} {:<8} {:<10} {}",
                        id, "ERROR", "ERROR", "Cannot read PID file"
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

fn status_daemon(id: &str, root_dir: &Path) -> Result<()> {
    let pid_file = build_file_path(root_dir, id, "pid");
    let stdout_file = build_file_path(root_dir, id, "stdout");
    let stderr_file = build_file_path(root_dir, id, "stderr");

    println!("Daemon: {}", id);
    println!("PID file: {}", pid_file.display());

    // Read PID data from file
    match PidFile::read_from_file(&pid_file) {
        Ok(pid_file_data) => {
            println!("PID: {}", pid_file_data.pid);
            println!("Command: {}", pid_file_data.command_string());

            if is_process_running_by_pid(pid_file_data.pid) {
                println!("Status: RUNNING");

                // Show file information
                if stdout_file.exists() {
                    let metadata = std::fs::metadata(&stdout_file)?;
                    println!("Stdout file: {} ({} bytes)", stdout_file.display(), metadata.len());
                } else {
                    println!("Stdout file: {} (not found)", stdout_file.display());
                }

                if stderr_file.exists() {
                    let metadata = std::fs::metadata(&stderr_file)?;
                    println!("Stderr file: {} ({} bytes)", stderr_file.display(), metadata.len());
                } else {
                    println!("Stderr file: {} (not found)", stderr_file.display());
                }
            } else {
                println!("Status: DEAD (process not running)");
                println!("Note: Use 'demon clean' to remove orphaned files");
            }
        }
        Err(PidFileReadError::FileNotFound) => {
            println!("Status: NOT FOUND (no PID file)");
        }
        Err(PidFileReadError::FileInvalid(reason)) => {
            println!("Status: ERROR (invalid PID file: {})", reason);
        }
        Err(PidFileReadError::IoError(err)) => {
            println!("Status: ERROR (cannot read PID file: {})", err);
        }
    }

    Ok(())
}

fn clean_orphaned_files(root_dir: &Path) -> Result<()> {
    tracing::info!("Scanning for orphaned daemon files...");

    let mut cleaned_count = 0;

    // Find all .pid files in root directory
    for entry in find_pid_files(root_dir)? {
        let path = entry.path();
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let id = filename.strip_suffix(".pid").unwrap_or(filename);

        // Read PID data from file
        match PidFile::read_from_file(&path) {
            Ok(pid_file_data) => {
                // Check if process is still running
                if !is_process_running_by_pid(pid_file_data.pid) {
                    println!(
                        "Cleaning up orphaned files for '{}' (PID: {})",
                        id, pid_file_data.pid
                    );

                    // Remove PID file
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::warn!("Failed to remove {}: {}", path.display(), e);
                    } else {
                        tracing::info!("Removed {}", path.display());
                    }

                    // Remove stdout file if it exists
                    let stdout_file = build_file_path(root_dir, id, "stdout");
                    if stdout_file.exists() {
                        if let Err(e) = std::fs::remove_file(&stdout_file) {
                            tracing::warn!("Failed to remove {}: {}", stdout_file.display(), e);
                        } else {
                            tracing::info!("Removed {}", stdout_file.display());
                        }
                    }

                    // Remove stderr file if it exists
                    let stderr_file = build_file_path(root_dir, id, "stderr");
                    if stderr_file.exists() {
                        if let Err(e) = std::fs::remove_file(&stderr_file) {
                            tracing::warn!("Failed to remove {}: {}", stderr_file.display(), e);
                        } else {
                            tracing::info!("Removed {}", stderr_file.display());
                        }
                    }

                    cleaned_count += 1;
                } else {
                    tracing::info!(
                        "Skipping '{}' (PID: {}) - process is still running",
                        id,
                        pid_file_data.pid
                    );
                }
            }
            Err(PidFileReadError::FileNotFound) => {
                // This shouldn't happen since we found the file, but handle gracefully
                tracing::warn!("PID file {} disappeared during processing", path.display());
            }
            Err(PidFileReadError::FileInvalid(_)) | Err(PidFileReadError::IoError(_)) => {
                println!("Cleaning up invalid PID file: {}", path.display());
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Failed to remove invalid PID file {}: {}", path.display(), e);
                } else {
                    tracing::info!("Removed invalid PID file {}", path.display());
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

### demon run <identifier> <command...>
Spawns a background process with the given identifier.

**Syntax**: `demon run <id> [--] <command> [args...]`

**Behavior**:
- Creates `<id>.pid`, `<id>.stdout`, `<id>.stderr` files
- Truncates log files if they already exist
- Fails if a process with the same ID is already running
- Parent process exits immediately, child continues in background
- Use `--` to separate flags from command when command has flags

**Examples**:
```bash
demon run web-server python -m http.server 8080
demon run backup-job -- rsync -av /data/ /backup/
demon run log-monitor tail -f /var/log/app.log
```

### demon stop <id> [--timeout <seconds>]
Stops a running daemon process gracefully.

**Syntax**: `demon stop <id> [--timeout <seconds>]`

**Behavior**:
- Sends SIGTERM to the process first
- Waits for specified timeout (default: 10 seconds)
- Sends SIGKILL if process doesn't terminate
- Removes PID file after successful termination
- Handles already-dead processes gracefully

**Examples**:
```bash
demon stop web-server
demon stop backup-job --timeout 30
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

### demon status <id>
Shows detailed status information for a specific daemon.

**Syntax**: `demon status <id>`

**Output includes**:
- Daemon ID and PID file location
- Process ID (if available)
- Current status (RUNNING/DEAD/NOT FOUND/ERROR)
- Log file locations and sizes
- Suggestions for cleanup if needed

**Example**:
```bash
demon status web-server
```

### demon cat <id> [--stdout] [--stderr]
Displays the contents of daemon log files.

**Syntax**: `demon cat <id> [--stdout] [--stderr]`

**Behavior**:
- Shows both stdout and stderr by default
- Use flags to show only specific streams
- Displays file headers when showing multiple files
- Handles missing files gracefully

**Examples**:
```bash
demon cat web-server           # Show both logs
demon cat web-server --stdout  # Show only stdout
demon cat web-server --stderr  # Show only stderr
```

### demon tail <id> [--stdout] [--stderr]
Follows daemon log files in real-time (like `tail -f`).

**Syntax**: `demon tail <id> [--stdout] [--stderr]`

**Behavior**:
- Shows existing content first, then follows new content
- Shows both stdout and stderr by default
- Uses file system notifications for efficient monitoring
- Press Ctrl+C to stop tailing
- Handles file creation, rotation, and truncation

**Examples**:
```bash
demon tail web-server           # Follow both logs
demon tail web-server --stdout  # Follow only stdout
```

### demon wait <id> [--timeout <seconds>] [--interval <seconds>]
Blocks until a daemon process terminates.

**Syntax**: `demon wait <id> [--timeout <seconds>] [--interval <seconds>]`

**Behavior**:
- Checks if PID file exists and process is running
- Polls the process every `interval` seconds (default: 1 second)
- Waits for up to `timeout` seconds (default: 30 seconds)
- Use `--timeout 0` for infinite wait
- Exits successfully when process terminates
- Fails with error if process doesn't exist or timeout is reached
- Does not clean up PID files (use `demon clean` for that)

**Examples**:
```bash
demon wait web-server                      # Wait 30s for termination
demon wait backup-job --timeout 0          # Wait indefinitely
demon wait data-processor --timeout 3600   # Wait up to 1 hour
demon wait short-task --interval 2         # Poll every 2 seconds
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
demon run my-web-server python -m http.server 8080
demon status my-web-server  # Check if it started
demon tail my-web-server    # Monitor logs
```

### Waiting for Process Completion
```bash
demon run batch-job python process_data.py
demon wait batch-job --timeout 600  # Wait up to 10 minutes
demon cat batch-job                  # Check output after completion
```

### Running a Backup Job
```bash
demon run nightly-backup -- rsync -av /data/ /backup/
demon cat nightly-backup   # Check output when done
demon clean                     # Clean up after completion
```

### Managing Multiple Services
```bash
demon run api-server ./api --port 3000
demon run worker-queue ./worker --config prod.conf
demon list                      # See all running services
demon stop api-server      # Stop specific service
```

### Monitoring and Debugging
```bash
demon list --quiet | grep RUNNING  # Machine-readable active processes
demon tail problematic-app --stderr  # Monitor just errors
demon status failing-service         # Get detailed status
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
if demon status my-service | grep -q "RUNNING"; then
    echo "Service is running"
fi

# Start service if not running
demon list --quiet | grep -q "my-service:" || demon run my-service ./my-app

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

fn wait_daemon(id: &str, timeout: u64, interval: u64, root_dir: &Path) -> Result<()> {
    let pid_file = build_file_path(root_dir, id, "pid");

    // Check if PID file exists and read PID data
    let pid_file_data = match PidFile::read_from_file(&pid_file) {
        Ok(data) => data,
        Err(PidFileReadError::FileNotFound) => {
            return Err(anyhow::anyhow!("Process '{}' not found (no PID file)", id));
        }
        Err(PidFileReadError::FileInvalid(reason)) => {
            return Err(anyhow::anyhow!(
                "Process '{}' has invalid PID file: {}",
                id,
                reason
            ));
        }
        Err(PidFileReadError::IoError(err)) => {
            return Err(anyhow::anyhow!(
                "Failed to read PID file for '{}': {}",
                id,
                err
            ));
        }
    };

    let pid = pid_file_data.pid;

    // Check if process is currently running
    if !is_process_running_by_pid(pid) {
        return Err(anyhow::anyhow!("Process '{}' is not running", id));
    }

    tracing::info!("Waiting for process '{}' (PID: {}) to terminate", id, pid);

    // Handle infinite timeout case
    if timeout == 0 {
        loop {
            if !is_process_running_by_pid(pid) {
                tracing::info!("Process '{}' (PID: {}) has terminated", id, pid);
                return Ok(());
            }
            thread::sleep(Duration::from_secs(interval));
        }
    }

    // Handle timeout case
    let mut elapsed = 0;
    while elapsed < timeout {
        if !is_process_running_by_pid(pid) {
            tracing::info!("Process '{}' (PID: {}) has terminated", id, pid);
            return Ok(());
        }

        thread::sleep(Duration::from_secs(interval));
        elapsed += interval;
    }

    // Timeout reached
    Err(anyhow::anyhow!(
        "Timeout reached waiting for process '{}' to terminate",
        id
    ))
}

fn find_pid_files(root_dir: &Path) -> Result<Vec<std::fs::DirEntry>> {
    let entries = std::fs::read_dir(root_dir)?
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
