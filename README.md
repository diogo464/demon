# demon - Background Process Manager

This tool helps AI agents control long-running processes and view their logs easily. For example, when running `npm run dev`, Claude often runs problematic commands like `npm run dev &` which makes it unable to see the logs or properly kill the process afterward. When it tries to run `npm run dev` again, the new instance binds to a different port and it ends up getting kind of lost.

Using demon with a justfile like this:
```Justfile
start: stop
    demon run server -- npm run dev

stop:
    demon stop server

logs:
    demon cat server
```
allows Claude to check errors, manage the running server without getting stuck, and be more autonomous overall.

## Installation

```bash
cargo install --git https://github.com/diogo464/demon
```

## Quick Start

```bash
# Start a development server
demon run web-server python -m http.server 8080

# Monitor the logs in real-time  
demon tail web-server

# Check what's running
demon list

# Stop the server
demon stop web-server

# Clean up finished processes
demon clean
```

## Command Reference

### `demon run <id> [command...]`
Spawn a background process with the given identifier.

```bash
# Basic usage
demon run my-app ./my-application

# Development server
demon run dev-server npm run dev

# Complex commands (use -- to separate)
demon run backup-job -- rsync -av /data/ /backup/

# Long-running computation
demon run ml-training python train_model.py --epochs 100
```

### `demon list [--quiet]`
List all managed processes and their status.

```bash
# Human-readable format
demon list

# Machine-readable format (for scripts/agents)
demon list --quiet
```

### `demon status <id>`
Show detailed status information for a specific process.

```bash
demon status web-server
```

### `demon stop <id> [--timeout <seconds>]`
Stop a running process gracefully (SIGTERM, then SIGKILL if needed).

```bash
# Default 10-second timeout
demon stop web-server

# Custom timeout
demon stop slow-service --timeout 30
```

### `demon tail <id> [--stdout] [--stderr]`
Follow log files in real-time (like `tail -f`).

```bash
# Follow both stdout and stderr
demon tail -f web-server

# Follow only stdout
demon tail -f web-server --stdout

# Follow only stderr  
demon tail =f web-server --stderr
```

### `demon cat <id> [--stdout] [--stderr]`
Display the complete contents of log files.

```bash
# Show both logs
demon cat web-server

# Show only stdout
demon cat web-server --stdout
```

### `demon wait <id> [--timeout <seconds>] [--interval <seconds>]`
Wait for a daemon process to terminate.

```bash
# Wait with default 30-second timeout
demon wait web-server

# Wait indefinitely 
demon wait web-server --timeout 0

# Wait with custom timeout and polling interval
demon wait web-server --timeout 60 --interval 2
```

### `demon clean`
Remove orphaned files from processes that are no longer running.

```bash
demon clean
```

## How It Works

When you run `demon run web-server python -m http.server 8080`:

1. **Root Directory Discovery**: Finds the git root directory and creates a `.demon` subdirectory for all daemon files (or uses `--root-dir` if specified, or `DEMON_ROOT_DIR` environment variable)
2. **Process Creation**: Spawns the process detached from your terminal
3. **File Management**: Creates three files in the root directory:
   - `web-server.pid` - Contains the process ID and command
   - `web-server.stdout` - Captures standard output
   - `web-server.stderr` - Captures error output
4. **Process Monitoring**: Tracks process lifecycle independently
5. **Log Management**: Files persist after process termination for inspection
