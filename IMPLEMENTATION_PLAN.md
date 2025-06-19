# Demon CLI Implementation Plan

## Project Overview
A CLI tool named `demon` for spawning and managing background processes with stdout/stderr redirection.

## Requirements Summary
- **Files created**: `<id>.pid`, `<id>.stdout`, `<id>.stderr` in working directory
- **Platform**: Linux only (maybe macOS later)
- **File location**: Working directory (add to .gitignore)
- **Signal handling**: SIGTERM then SIGKILL with configurable timeout
- **Logging**: Tool logs to stderr, process output to stdout
- **Concurrency**: Single process tail for now

## CLI Structure
```
demon run --id <identifier> <command...>
demon stop --id <id> [--timeout <seconds>]
demon tail [--stdout] [--stderr] --id <id>
demon cat [--stdout] [--stderr] --id <id>
demon list
demon status --id <id>
demon clean
```

## Implementation Progress

### ✅ Phase 1: Project Setup
- [x] Add dependencies: clap, tracing, tracing-subscriber, notify
- [x] Create CLI structure with Commands enum and Args structs

### 🔄 Phase 2: Core Process Management
- [ ] **CURRENT**: Implement `demon run`
  - Check if process already running via PID file
  - Spawn process with stdout/stderr redirection to files
  - Write PID to `.pid` file
  - Truncate log files when starting new process
  - Detach process so parent can exit
- [ ] Implement `demon stop`
  - Read PID from file
  - Send SIGTERM first
  - Wait for timeout, then send SIGKILL
  - Clean up PID file
  - Handle already-dead processes gracefully

### 📋 Phase 3: File Operations
- [ ] Implement `demon cat`
  - Read and display `.stdout` and/or `.stderr` files
  - Handle file selection flags properly
  - Error handling for missing files
- [ ] Implement `demon tail`
  - Use `notify` crate for file watching
  - Support both stdout and stderr simultaneously
  - Handle file rotation/truncation
  - Clean shutdown on Ctrl+C

### 📋 Phase 4: Additional Commands
- [ ] Implement `demon list`
  - Scan working directory for `.pid` files
  - Check which processes are actually running
  - Display process info
- [ ] Implement `demon status`
  - Check if specific process is running
  - Display process info
- [ ] Implement `demon clean`
  - Find orphaned files (PID exists but process dead)
  - Remove orphaned `.pid`, `.stdout`, `.stderr` files

### 📋 Phase 5: Error Handling & Polish
- [ ] Robust error handling throughout
- [ ] Proper cleanup on failures
- [ ] Input validation
- [ ] Help text and documentation

## Technical Implementation Details

### Process Spawning (demon run)
```rust
// 1. Check if <id>.pid exists and process is running
// 2. Truncate/create <id>.stdout and <id>.stderr files
// 3. Spawn process with:
//    - stdout redirected to <id>.stdout
//    - stderr redirected to <id>.stderr
//    - stdin redirected to /dev/null
// 4. Write PID to <id>.pid file
// 5. Don't call .wait() - let process run detached
```

### Process Stopping (demon stop)
```rust
// 1. Read PID from <id>.pid file
// 2. Send SIGTERM to process
// 3. Wait for timeout (default 10s)
// 4. If still running, send SIGKILL
// 5. Remove <id>.pid file
// 6. Handle process already dead gracefully
```

### File Tailing (demon tail)
```rust
// 1. Use notify crate to watch file changes
// 2. When files change, read new content and print
// 3. Handle both stdout and stderr based on flags
// 4. Default: show both if neither flag specified
// 5. Graceful shutdown on Ctrl+C
```

### File Listing (demon list)
```rust
// 1. Glob for *.pid files in current directory
// 2. For each PID file, check if process is running
// 3. Display: ID, PID, Status, Command (if available)
```

## Dependencies Used
- `clap` (derive feature) - CLI argument parsing
- `tracing` + `tracing-subscriber` - Structured logging
- `notify` - File system notifications for tail
- Standard library for process management

## File Naming Convention
- PID file: `<id>.pid`
- Stdout log: `<id>.stdout`  
- Stderr log: `<id>.stderr`

## Error Handling Strategy
- Use `Result<(), Box<dyn std::error::Error>>` for main functions
- Log errors using `tracing::error!`
- Exit with code 1 on errors
- Provide descriptive error messages

## Testing Strategy
- Manual testing with simple commands (sleep, echo, etc.)
- Test edge cases: process crashes, missing files, etc.
- Test signal handling and cleanup

## Current Status
- ✅ All core functionality implemented and tested
- ✅ CLI structure with proper subcommands and arguments
- ✅ Process spawning and management working correctly
- ✅ File watching and real-time tailing functional
- ✅ Error handling and edge cases covered
- ✅ Clean up functionality for orphaned files

## Implementation Complete!

All planned features have been successfully implemented:

1. **`demon run`** - ✅ Spawns background processes with file redirection
2. **`demon stop`** - ✅ Graceful termination with SIGTERM/SIGKILL timeout
3. **`demon tail`** - ✅ Real-time file watching with notify crate
4. **`demon cat`** - ✅ Display log file contents
5. **`demon list`** - ✅ Show all managed processes with status
6. **`demon status`** - ✅ Detailed status of specific process
7. **`demon clean`** - ✅ Remove orphaned files from dead processes

## Testing Summary

All commands have been tested and work correctly:
- Process spawning and detachment
- Signal handling (SIGTERM → SIGKILL)
- File redirection (stdout/stderr)
- Duplicate process detection
- File watching and real-time updates
- Orphan cleanup
- Error handling for edge cases

## Final Architecture

The implementation follows the planned modular structure:
- **CLI Interface**: Uses clap with enum-based subcommands ✅
- **Process Manager**: Handles spawning, tracking, and termination ✅
- **File Operations**: Manages PID files and log redirection ✅
- **Output Display**: Implements both cat and tail functionality ✅

The tool is ready for production use!