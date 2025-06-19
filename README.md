# Demon - Background Process Manager

A lightweight, intuitive CLI tool for spawning, managing, and monitoring background processes on Linux systems. Perfect for development servers, long-running tasks, and automation workflows.

## ✨ Features

- **Simple Process Management**: Start, stop, and monitor background processes with ease
- **Persistent Logging**: Automatic stdout/stderr capture to files  
- **Real-time Monitoring**: Tail logs in real-time with file watching
- **Process Lifecycle**: Graceful termination with SIGTERM/SIGKILL fallback
- **Machine-readable Output**: Perfect for scripting and LLM agent integration
- **Zero Configuration**: Works out of the box, no setup required

## 🚀 Installation

### From Source
```bash
git clone https://github.com/yourusername/demon
cd demon
cargo install --path .
```

### From Crates.io (Coming Soon)
```bash
cargo install demon
```

## 🎯 Quick Start

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

## 📋 Command Reference

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
demon tail web-server

# Follow only stdout
demon tail web-server --stdout

# Follow only stderr  
demon tail web-server --stderr
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

### `demon llm`
Output comprehensive usage guide optimized for LLM consumption.

```bash
demon llm
```

## 🎮 Use Cases

### Development Workflows
Perfect for managing development servers and build processes:

```bash
# Start multiple development services
demon run api-server npm run dev
demon run frontend yarn start  
demon run db-server docker run -p 5432:5432 postgres

# Monitor everything
demon list
demon tail api-server --stderr  # Watch for errors

# Wait for a specific service to finish
demon wait api-server
```

### LLM Agent Integration
Designed for seamless automation and LLM agent workflows:

```bash
# Agents can start long-running processes
demon run data-processor python process_large_dataset.py

# Wait for the process to complete
demon wait data-processor --timeout 3600  # 1 hour timeout

# Check status programmatically
if demon status data-processor | grep -q "RUNNING"; then
    echo "Processing is still running"
fi

# Get machine-readable process list
demon list --quiet | while IFS=: read id pid status; do
    echo "Process $id ($pid) is $status"
done
```

### Background Tasks & Scripts
Ideal for CI/CD, backups, and system maintenance:

```bash
# Database backup
demon run nightly-backup -- pg_dump mydb > backup.sql

# Log file processing
demon run log-analyzer tail -f /var/log/app.log | grep ERROR

# System monitoring
demon run monitor -- iostat -x 1
```

### DevOps & System Administration
Manage services without complex init systems:

```bash
# Application deployment
demon run app-server ./deploy.sh production

# Health monitoring
demon run health-check -- while true; do curl -f http://localhost:8080/health || exit 1; sleep 30; done

# Resource monitoring
demon run resource-monitor -- top -b -n1 | head -20
```

## 🏗️ How It Works

When you run `demon run web-server python -m http.server 8080`:

1. **Process Creation**: Spawns the process detached from your terminal
2. **File Management**: Creates three files:
   - `web-server.pid` - Contains the process ID and command
   - `web-server.stdout` - Captures standard output
   - `web-server.stderr` - Captures error output
3. **Process Monitoring**: Tracks process lifecycle independently
4. **Log Management**: Files persist after process termination for inspection

## 🤖 LLM Agent Integration

Demon is specifically designed to work seamlessly with LLM agents and automation tools:

### Machine-Readable Output
```bash
# Get process status in parseable format
demon list --quiet
# Output: web-server:12345:RUNNING
#         backup-job:12346:DEAD
```

### Scripting Examples
```bash
# Start service if not running
demon list --quiet | grep -q "web-server:" || demon run web-server python -m http.server

# Wait for process to finish
demon wait backup-job --timeout 0  # Wait indefinitely

# Get all running processes
demon list --quiet | grep ":RUNNING" | cut -d: -f1
```

### Error Handling
```bash
# Check if process started successfully
if demon run api-server ./start-api.sh; then
    echo "API server started successfully"
    demon tail api-server &  # Monitor in background
else
    echo "Failed to start API server"
    exit 1
fi
```

## 📁 File Management

### File Locations
All files are created in the current working directory:
- `<id>.pid` - Process ID and command information
- `<id>.stdout` - Standard output log
- `<id>.stderr` - Standard error log

### Cleanup
- Files persist after process termination for inspection
- Use `demon clean` to remove files from dead processes
- Consider adding `*.pid`, `*.stdout`, `*.stderr` to `.gitignore`

### Log Rotation
- Demon doesn't handle log rotation internally
- For long-running processes, implement rotation in your application
- Or use external tools like `logrotate`

## 🔧 Advanced Usage

### Process Management
```bash
# Graceful shutdown with custom timeout
demon stop long-running-task --timeout 60

# Force kill if process is stuck
demon stop stuck-process --timeout 1
```

### Monitoring & Debugging
```bash
# Monitor multiple processes
for id in web-server api-server worker; do
    demon tail $id --stderr &
done

# Check resource usage
demon run monitor -- ps aux | grep my-app
demon cat monitor
```

### Integration with System Tools
```bash
# Use with systemd
demon run my-service systemctl --user start my-app

# Use with Docker
demon run container -- docker run -d --name myapp nginx

# Use with tmux/screen for complex setups
demon run dev-env -- tmux new-session -d 'npm run dev'
```

## ⚠️ System Requirements

- **Operating System**: Linux (uses `kill` command for process management)
- **Rust**: 1.70+ (for building from source)
- **Permissions**: Standard user permissions (no root required)

## 🔒 Security Considerations

- Demon runs processes with the same permissions as the calling user
- PID files contain process information - protect accordingly
- Log files may contain sensitive information from your applications
- No network access or elevated privileges required

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes**
4. **Run tests**: `cargo test`
5. **Format code**: `cargo fmt`
6. **Submit a pull request**

### Development Setup
```bash
git clone https://github.com/yourusername/demon
cd demon
cargo build
cargo test
```

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🐛 Bug Reports & Feature Requests

Found a bug or have a feature idea? Please open an issue on [GitHub Issues](https://github.com/yourusername/demon/issues).

## 📚 Similar Tools

- **pm2** - Process manager for Node.js applications
- **supervisor** - Process control system for Unix  
- **systemd** - System and service manager for Linux
- **screen/tmux** - Terminal multiplexers

Demon focuses on simplicity, LLM integration, and developer experience over complex process management features.

---

**Built with ❤️ in Rust**