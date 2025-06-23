use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_help_output() {
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("daemon process management"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("tail"))
        .stdout(predicate::str::contains("cat"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("clean"))
        .stdout(predicate::str::contains("wait"));
}

#[test]
fn test_version_output() {
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("demon 0.1.0"));
}

#[test]
fn test_run_missing_command() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        temp_dir.path().to_str().unwrap(),
        "test",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("Command cannot be empty"));
}

#[test]
fn test_run_creates_files() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        temp_dir.path().to_str().unwrap(),
        "test",
        "echo",
        "hello",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Started daemon 'test'"));

    // Verify files were created
    assert!(temp_dir.path().join("test.pid").exists());
    assert!(temp_dir.path().join("test.stdout").exists());
    assert!(temp_dir.path().join("test.stderr").exists());

    // Give the process a moment to complete
    std::thread::sleep(Duration::from_millis(100));

    // Check that stdout contains our output
    let stdout_content = fs::read_to_string(temp_dir.path().join("test.stdout")).unwrap();
    assert_eq!(stdout_content.trim(), "hello");
}

#[test]
fn test_run_duplicate_process() {
    let temp_dir = TempDir::new().unwrap();

    // Start a long-running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        temp_dir.path().to_str().unwrap(),
        "long",
        "sleep",
        "30",
    ])
    .assert()
    .success();

    // Try to start another with the same ID
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "long", "sleep", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already running"));

    // Clean up the running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["stop", "long"])
        .assert()
        .success();
}

#[test]
fn test_list_empty() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["list", "--root-dir", temp_dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("PID"))
        .stdout(predicate::str::contains("STATUS"))
        .stdout(predicate::str::contains("No daemon processes found"));
}

#[test]
fn test_list_with_processes() {
    let temp_dir = TempDir::new().unwrap();

    // Start a process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "test", "echo", "done"])
        .assert()
        .success();

    // List processes
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test"))
        .stdout(predicate::str::contains("DEAD")); // Process should be finished by now
}

#[test]
fn test_cat_output() {
    let temp_dir = TempDir::new().unwrap();

    // Create a process with output
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&[
            "run",
            "test",
            "--",
            "sh",
            "-c",
            "echo 'stdout line'; echo 'stderr line' >&2",
        ])
        .assert()
        .success();

    // Cat the output
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["cat", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout line"))
        .stdout(predicate::str::contains("stderr line"));
}

#[test]
fn test_cat_stdout_only() {
    let temp_dir = TempDir::new().unwrap();

    // Create a process with output
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&[
            "run",
            "test",
            "--",
            "sh",
            "-c",
            "echo 'stdout line'; echo 'stderr line' >&2",
        ])
        .assert()
        .success();

    // Cat only stdout
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["cat", "test", "--stdout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout line"))
        .stdout(predicate::str::contains("stderr line").not());
}

#[test]
fn test_status_nonexistent() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["status", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("NOT FOUND"));
}

#[test]
fn test_status_dead_process() {
    let temp_dir = TempDir::new().unwrap();

    // Create a short-lived process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "dead", "echo", "hello"])
        .assert()
        .success();

    // Check its status (should be dead)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["status", "dead"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DEAD"));
}

#[test]
fn test_stop_nonexistent() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["stop", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

#[test]
fn test_stop_process() {
    let temp_dir = TempDir::new().unwrap();

    // Start a long-running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "long", "sleep", "10"])
        .assert()
        .success();

    // Stop it
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["stop", "long"])
        .assert()
        .success()
        .stdout(predicate::str::contains("terminated gracefully"));

    // Verify PID file is gone
    assert!(!temp_dir.path().join("long.pid").exists());
}

#[test]
fn test_clean_no_orphans() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["clean"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No orphaned files found"));
}

#[test]
fn test_clean_with_orphans() {
    let temp_dir = TempDir::new().unwrap();

    // Create a dead process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        temp_dir.path().to_str().unwrap(),
        "dead",
        "echo",
        "hello",
    ])
    .assert()
    .success();

    // Wait for process to complete
    std::thread::sleep(Duration::from_millis(100));

    // Verify files exist before clean
    assert!(temp_dir.path().join("dead.pid").exists());
    assert!(temp_dir.path().join("dead.stdout").exists());
    assert!(temp_dir.path().join("dead.stderr").exists());

    // Clean up orphaned files
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["clean", "--root-dir", temp_dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleaned up"))
        .stdout(predicate::str::contains("orphaned"));

    // Verify files are gone
    assert!(!temp_dir.path().join("dead.pid").exists());
    assert!(!temp_dir.path().join("dead.stdout").exists());
    assert!(!temp_dir.path().join("dead.stderr").exists());
}

#[test]
fn test_clean_removes_stdout_stderr_files() {
    let temp_dir = TempDir::new().unwrap();

    // Create a process that outputs to both stdout and stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        temp_dir.path().to_str().unwrap(),
        "test_output",
        "--",
        "sh",
        "-c",
        "echo 'stdout content'; echo 'stderr content' >&2",
    ])
    .assert()
    .success();

    // Wait for process to complete
    std::thread::sleep(Duration::from_millis(100));

    // Verify all files exist and have content
    assert!(temp_dir.path().join("test_output.pid").exists());
    assert!(temp_dir.path().join("test_output.stdout").exists());
    assert!(temp_dir.path().join("test_output.stderr").exists());

    let stdout_content = fs::read_to_string(temp_dir.path().join("test_output.stdout")).unwrap();
    let stderr_content = fs::read_to_string(temp_dir.path().join("test_output.stderr")).unwrap();
    assert!(stdout_content.contains("stdout content"));
    assert!(stderr_content.contains("stderr content"));

    // Clean up orphaned files
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["clean", "--root-dir", temp_dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleaned up"))
        .stdout(predicate::str::contains("orphaned"));

    // Verify ALL files are gone, not just the PID file
    assert!(!temp_dir.path().join("test_output.pid").exists());
    assert!(!temp_dir.path().join("test_output.stdout").exists());
    assert!(!temp_dir.path().join("test_output.stderr").exists());
}

#[test]
fn test_default_demon_directory_creation() {
    // This test verifies that when no --root-dir is specified,
    // the system creates and uses a .demon subdirectory in the git root

    // Create a temporary git repo
    let temp_dir = TempDir::new().unwrap();
    let git_dir = temp_dir.path().join(".git");
    std::fs::create_dir(&git_dir).unwrap();

    // Change to the temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Restore directory when done
    struct DirGuard(PathBuf);
    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }
    let _guard = DirGuard(original_dir);

    // Run a command without --root-dir to test default behavior
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["run", "default_test", "echo", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Started daemon 'default_test'"));

    // Wait for process to complete
    std::thread::sleep(Duration::from_millis(100));

    // Verify that .demon directory was created and files are in it
    let demon_dir = temp_dir.path().join(".demon");
    assert!(demon_dir.exists());
    assert!(demon_dir.is_dir());
    assert!(demon_dir.join("default_test.pid").exists());
    assert!(demon_dir.join("default_test.stdout").exists());
    assert!(demon_dir.join("default_test.stderr").exists());

    // Verify the stdout content
    let stdout_content = fs::read_to_string(demon_dir.join("default_test.stdout")).unwrap();
    assert_eq!(stdout_content.trim(), "hello");
}

#[test]
fn test_run_with_complex_command() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&[
            "run",
            "complex",
            "--",
            "sh",
            "-c",
            "for i in 1 2 3; do echo \"line $i\"; done",
        ])
        .assert()
        .success();

    // Give the process a moment to complete
    std::thread::sleep(Duration::from_millis(100));

    // Check the output contains all lines
    let stdout_content = fs::read_to_string(temp_dir.path().join("complex.stdout")).unwrap();
    assert!(stdout_content.contains("line 1"));
    assert!(stdout_content.contains("line 2"));
    assert!(stdout_content.contains("line 3"));
}

#[test]
fn test_timeout_configuration() {
    let temp_dir = TempDir::new().unwrap();

    // Start a process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "timeout-test", "sleep", "5"])
        .assert()
        .success();

    // Stop with custom timeout (should work normally since sleep responds to SIGTERM)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["stop", "timeout-test", "--timeout", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains("terminated gracefully"));
}

#[test]
fn test_invalid_process_id() {
    let temp_dir = TempDir::new().unwrap();

    // Create an invalid PID file
    fs::write(temp_dir.path().join("invalid.pid"), "not-a-number").unwrap();

    // Status should handle it gracefully
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["status", "invalid"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ERROR"));

    // Clean should remove it
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["clean"])
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid PID file"));
}

#[test]
fn test_list_quiet_mode() {
    let temp_dir = TempDir::new().unwrap();

    // Test quiet mode with no processes
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["list", "--quiet"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Create a process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "quiet-test", "echo", "done"])
        .assert()
        .success();

    // Test quiet mode with process - should output colon-separated format
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["list", "-q"])
        .assert()
        .success()
        .stdout(predicate::str::contains("quiet-test:"))
        .stdout(predicate::str::contains(":DEAD"))
        // Should not contain headers
        .stdout(predicate::str::contains("ID").not())
        .stdout(predicate::str::contains("PID").not())
        .stdout(predicate::str::contains("STATUS").not());
}

#[test]
fn test_llm_command() {
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["llm"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "# Demon - Daemon Process Management CLI",
        ))
        .stdout(predicate::str::contains("## Available Commands"))
        .stdout(predicate::str::contains("demon run"))
        .stdout(predicate::str::contains("demon stop"))
        .stdout(predicate::str::contains("demon list"))
        .stdout(predicate::str::contains("demon tail"))
        .stdout(predicate::str::contains("demon cat"))
        .stdout(predicate::str::contains("demon status"))
        .stdout(predicate::str::contains("demon clean"))
        .stdout(predicate::str::contains("demon wait"))
        .stdout(predicate::str::contains("Common Workflows"))
        .stdout(predicate::str::contains("Best Practices"))
        .stdout(predicate::str::contains("Integration Tips"));
}

#[test]
fn test_wait_nonexistent_process() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["wait", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_wait_already_dead_process() {
    let temp_dir = TempDir::new().unwrap();

    // Create a short-lived process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "dead", "echo", "hello"])
        .assert()
        .success();

    // Give it time to finish
    std::thread::sleep(Duration::from_millis(100));

    // Try to wait for it (should fail since it's already dead)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["wait", "dead"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not running"));
}

#[test]
fn test_wait_process_terminates() {
    let temp_dir = TempDir::new().unwrap();

    // Start a process that will run for 2 seconds
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "short", "sleep", "2"])
        .assert()
        .success();

    // Wait for it with a 5-second timeout (should succeed)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["wait", "short", "--timeout", "5"])
        .assert()
        .success();
}

#[test]
fn test_wait_timeout() {
    let temp_dir = TempDir::new().unwrap();

    // Start a long-running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "long", "sleep", "10"])
        .assert()
        .success();

    // Wait with a very short timeout (should fail)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["wait", "long", "--timeout", "2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Timeout reached"));

    // Clean up the still-running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["stop", "long"])
        .assert()
        .success();
}

#[test]
fn test_wait_infinite_timeout() {
    let temp_dir = TempDir::new().unwrap();

    // Start a short process that will finish quickly
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "quick", "sleep", "1"])
        .assert()
        .success();

    // Wait with infinite timeout (should succeed quickly)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["wait", "quick", "--timeout", "0"])
        .assert()
        .success();
}

#[test]
fn test_wait_custom_interval() {
    let temp_dir = TempDir::new().unwrap();

    // Start a short process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["run", "interval-test", "sleep", "2"])
        .assert()
        .success();

    // Wait with custom interval (should still succeed)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(["--root-dir", temp_dir.path().to_str().unwrap()])
        .args(&["wait", "interval-test", "--timeout", "5", "--interval", "2"])
        .assert()
        .success();
}
