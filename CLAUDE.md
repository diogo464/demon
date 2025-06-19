## project development guidelines
remember to use a ./IMPLEMENTATION_PLAN.md file to keep track of your work and maintain it updated when you complete work or requirements changes. you should add as much detail as you think is necessary to this file.

## rust guidelines
do not add dependencies manually, instead, use the following tools:
+ `cargo info` to obtain information about a crate such as its version, features, licence, ...
+ `cargo add` to add new dependencies, you can use the `--features` to specifiy comma separated list of features
+ for logging, prefer the `tracing` crate with `tracing-subscriber` and fully qualify the log macros (ex: `tracing::info!`)
+ for cli use the `clap` crate. when implementing subcommands use an `enum` and separate structs for each subcommand's arguments
+ use the `anyhow` crate for error handling

## testing guidelines
for testing cli applications, use the `assert_cmd` crate for integration testing

## assert_cmd crate reference

### Overview
`assert_cmd` is a Rust testing library designed to simplify integration testing of command-line applications. It provides easy command initialization, simplified configuration, and robust assertion mechanisms.

### Key Features
- Easy command initialization and execution
- Cargo binary testing support
- Flexible output validation with predicates
- Environment variable and stdin management
- Comprehensive assertion mechanisms

### Basic Usage Patterns

#### 1. Basic Command Testing
```rust
use assert_cmd::Command;

// Run a Cargo binary
let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.assert().success(); // Basic success assertion
```

#### 2. Command with Arguments
```rust
let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.args(&["run", "--id", "test", "sleep", "5"])
   .assert()
   .success();
```

#### 3. Output Validation
```rust
use predicates::prelude::*;

let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.args(&["list"])
   .assert()
   .success()
   .stdout(predicate::str::contains("ID"))
   .stderr(predicate::str::is_empty());
```

#### 4. Testing Failures
```rust
let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.args(&["run", "--id", "test"]) // Missing command
   .assert()
   .failure()
   .stderr(predicate::str::contains("Command cannot be empty"));
```

### Key Methods

#### Command Configuration
- `Command::cargo_bin("binary_name")`: Find and initialize a Cargo project binary
- `arg(arg)` / `args(&[args])`: Add command arguments
- `env(key, value)` / `envs(vars)`: Set environment variables
- `current_dir(path)`: Set working directory
- `write_stdin(input)`: Provide stdin input

#### Assertions
- `assert()`: Start assertion chain
- `success()`: Check for successful execution (exit code 0)
- `failure()`: Check for command failure (exit code != 0)
- `code(expected)`: Validate specific exit code
- `stdout(predicate)`: Validate stdout content
- `stderr(predicate)`: Validate stderr content

### Predicates for Output Validation
```rust
use predicates::prelude::*;

// Exact match
.stdout("exact output")

// Contains text
.stdout(predicate::str::contains("partial"))

// Regex match
.stdout(predicate::str::is_match(r"PID: \d+").unwrap())

// Empty output
.stderr(predicate::str::is_empty())

// Multiple conditions
.stdout(predicate::str::contains("SUCCESS").and(predicate::str::contains("ID")))
```

### Testing File I/O
For testing CLI tools that create/modify files, combine with `tempfile` and `assert_fs`:

```rust
use tempfile::TempDir;
use std::fs;

#[test]
fn test_file_creation() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.current_dir(temp_dir.path())
       .args(&["run", "--id", "test", "echo", "hello"])
       .assert()
       .success();
    
    // Verify files were created
    assert!(temp_dir.path().join("test.pid").exists());
    assert!(temp_dir.path().join("test.stdout").exists());
}
```

### Best Practices

1. **Use `cargo_bin()`**: Automatically locate project binaries
2. **Chain configuration**: Configure all arguments/env before calling `assert()`
3. **Test various scenarios**: Success, failure, edge cases
4. **Use predicates**: More flexible than exact string matching
5. **Isolate tests**: Use temporary directories for file-based tests
6. **Test error conditions**: Verify proper error handling and messages

### Common Patterns for CLI Testing

#### Testing Help Output
```rust
let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.args(&["--help"])
   .assert()
   .success()
   .stdout(predicate::str::contains("daemon process management"));
```

#### Testing Subcommands
```rust
let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.args(&["list"])
   .assert()
   .success()
   .stdout(predicate::str::contains("ID"));
```

#### Testing with Timeouts
```rust
use std::time::Duration;

let mut cmd = Command::cargo_bin("demon").unwrap();
cmd.timeout(Duration::from_secs(30)) // Prevent hanging tests
   .args(&["run", "--id", "long", "sleep", "10"])
   .assert()
   .success();
```

### Integration with Other Test Crates
- **`assert_fs`**: Filesystem testing utilities
- **`predicates`**: Advanced output validation
- **`tempfile`**: Temporary file/directory management
- **`serial_test`**: Serialize tests that can't run concurrently
