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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Command cannot be empty"));
}

#[test]
fn test_run_creates_files() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "test", "echo", "hello"])
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "long", "sleep", "30"])
        .assert()
        .success();

    // Try to start another with the same ID
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "long", "sleep", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already running"));

    // Clean up the running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["stop", "long"])
        .assert()
        .success();
}

#[test]
fn test_list_empty() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["list"])
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "test", "echo", "done"])
        .assert()
        .success();

    // List processes
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "dead", "echo", "hello"])
        .assert()
        .success();

    // Check its status (should be dead)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["status", "dead"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DEAD"));
}

#[test]
fn test_stop_nonexistent() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "long", "sleep", "10"])
        .assert()
        .success();

    // Stop it
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "dead", "echo", "hello"])
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["clean"])
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&[
            "run",
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["clean"])
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "timeout-test", "sleep", "5"])
        .assert()
        .success();

    // Stop with custom timeout (should work normally since sleep responds to SIGTERM)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["status", "invalid"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ERROR"));

    // Clean should remove it
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["list", "--quiet"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Create a process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "quiet-test", "echo", "done"])
        .assert()
        .success();

    // Test quiet mode with process - should output colon-separated format
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "dead", "echo", "hello"])
        .assert()
        .success();

    // Give it time to finish
    std::thread::sleep(Duration::from_millis(100));

    // Try to wait for it (should fail since it's already dead)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
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
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "short", "sleep", "2"])
        .assert()
        .success();

    // Wait for it with a 5-second timeout (should succeed)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["wait", "short", "--timeout", "5"])
        .assert()
        .success();
}

#[test]
fn test_wait_timeout() {
    let temp_dir = TempDir::new().unwrap();

    // Start a long-running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "long", "sleep", "10"])
        .assert()
        .success();

    // Wait with a very short timeout (should fail)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["wait", "long", "--timeout", "2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Timeout reached"));

    // Clean up the still-running process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["stop", "long"])
        .assert()
        .success();
}

#[test]
fn test_wait_infinite_timeout() {
    let temp_dir = TempDir::new().unwrap();

    // Start a short process that will finish quickly
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "quick", "sleep", "1"])
        .assert()
        .success();

    // Wait with infinite timeout (should succeed quickly)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["wait", "quick", "--timeout", "0"])
        .assert()
        .success();
}

#[test]
fn test_wait_custom_interval() {
    let temp_dir = TempDir::new().unwrap();

    // Start a short process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "interval-test", "sleep", "2"])
        .assert()
        .success();

    // Wait with custom interval (should still succeed)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["wait", "interval-test", "--timeout", "5", "--interval", "2"])
        .assert()
        .success();
}

// Root directory validation edge case tests

#[test]
fn test_root_dir_is_file_not_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Create a file instead of a directory
    let file_path = temp_dir.path().join("not_a_directory");
    fs::write(&file_path, "this is a file").unwrap();

    // Try to use the file as root directory - should fail
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        file_path.to_str().unwrap(),
        "test",
        "echo",
        "hello",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("not a directory"));
}

#[test]
fn test_root_dir_does_not_exist() {
    let temp_dir = TempDir::new().unwrap();

    // Use a non-existent path
    let nonexistent_path = temp_dir.path().join("does_not_exist");

    // Try to use non-existent path as root directory - should fail
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "--root-dir",
        nonexistent_path.to_str().unwrap(),
        "run",
        "test",
        "echo",
        "hello",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_git_root_demon_dir_exists_as_file() {
    // Create a temporary git repo
    let temp_dir = TempDir::new().unwrap();
    let git_dir = temp_dir.path().join(".git");
    std::fs::create_dir(&git_dir).unwrap();

    // Create .demon as a FILE instead of directory
    let demon_file = temp_dir.path().join(".demon");
    fs::write(&demon_file, "this should be a directory").unwrap();

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

    // Run command without --root-dir (should use git root and fail)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["run", "test", "echo", "hello"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exists but is not a directory"));
}

#[test]
fn test_git_root_demon_dir_permission_denied() {
    // This test is tricky to implement portably since it requires creating
    // a directory with restricted permissions. We'll create a more comprehensive
    // test that simulates the condition by creating a read-only parent directory.

    let temp_dir = TempDir::new().unwrap();
    let git_dir = temp_dir.path().join(".git");
    std::fs::create_dir(&git_dir).unwrap();

    // Create a subdirectory and make it read-only
    let subdir = temp_dir.path().join("subdir");
    std::fs::create_dir(&subdir).unwrap();
    let subdir_git = subdir.join(".git");
    std::fs::create_dir(&subdir_git).unwrap();

    // Make the subdirectory read-only (this should prevent .demon creation)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&subdir).unwrap().permissions();
        perms.set_mode(0o444); // Read-only
        std::fs::set_permissions(&subdir, perms).unwrap();
    }

    // Change to the subdirectory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&subdir).unwrap();

    // Restore directory and permissions when done
    struct TestGuard {
        original_dir: PathBuf,
        #[cfg(unix)]
        restore_path: PathBuf,
    }
    impl Drop for TestGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original_dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mut perms) =
                    std::fs::metadata(&self.restore_path).map(|m| m.permissions())
                {
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(&self.restore_path, perms);
                }
            }
        }
    }
    let _guard = TestGuard {
        original_dir,
        #[cfg(unix)]
        restore_path: subdir.clone(),
    };

    // Run command without --root-dir - should fail due to permission denied
    #[cfg(unix)]
    {
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&["run", "test", "echo", "hello"])
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "Failed to create daemon directory",
            ));
    }
}

#[test]
fn test_no_git_root_and_no_root_dir() {
    // Create a temporary directory that's NOT a git repository
    let temp_dir = TempDir::new().unwrap();

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

    // Run command without --root-dir and outside git repo - should fail
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&["run", "test", "echo", "hello"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No git repository found"));
}

#[test]
fn test_invalid_utf8_path_handling() {
    // This test checks handling of paths with invalid UTF-8 characters
    // This is primarily relevant on Unix systems where paths can contain arbitrary bytes

    let temp_dir = TempDir::new().unwrap();

    // Try to use a path with null bytes (should be invalid on most systems)
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "--root-dir",
        "path\0with\0nulls",
        "run",
        "test",
        "echo",
        "hello",
    ])
    .assert()
    .failure();
    // Note: exact error message may vary by OS, so we don't check specific text
}

#[test]
fn test_deeply_nested_nonexistent_path() {
    let temp_dir = TempDir::new().unwrap();

    // Create a path with many levels that don't exist
    let deep_path = temp_dir
        .path()
        .join("does")
        .join("not")
        .join("exist")
        .join("at")
        .join("all")
        .join("very")
        .join("deep")
        .join("path");

    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "--root-dir",
        deep_path.to_str().unwrap(),
        "run",
        "test",
        "echo",
        "hello",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn test_root_dir_is_symlink_to_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Create a real directory
    let real_dir = temp_dir.path().join("real_directory");
    std::fs::create_dir(&real_dir).unwrap();

    // Create a symlink to it (on systems that support it)
    let symlink_path = temp_dir.path().join("symlink_to_dir");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

        // Using symlink as root dir should work (following the symlink)
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&[
            "--root-dir",
            symlink_path.to_str().unwrap(),
            "run",
            "test",
            "echo",
            "hello",
        ])
        .assert()
        .success();

        // Verify files were created in the real directory (following symlink)
        std::thread::sleep(Duration::from_millis(100));
        assert!(real_dir.join("test.pid").exists());
        assert!(real_dir.join("test.stdout").exists());
        assert!(real_dir.join("test.stderr").exists());
    }
}

#[test]
fn test_root_dir_is_symlink_to_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create a regular file
    let regular_file = temp_dir.path().join("regular_file");
    fs::write(&regular_file, "content").unwrap();

    // Create a symlink to the file
    let symlink_path = temp_dir.path().join("symlink_to_file");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&regular_file, &symlink_path).unwrap();

        // Using symlink to file as root dir should fail
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&[
            "--root-dir",
            symlink_path.to_str().unwrap(),
            "run",
            "test",
            "echo",
            "hello",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
    }
}

#[test]
fn test_root_dir_is_broken_symlink() {
    let temp_dir = TempDir::new().unwrap();

    // Create a symlink to a non-existent target
    let broken_symlink = temp_dir.path().join("broken_symlink");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("nonexistent_target", &broken_symlink).unwrap();

        // Using broken symlink as root dir should fail
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&[
            "--root-dir",
            broken_symlink.to_str().unwrap(),
            "run",
            "test",
            "echo",
            "hello",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
    }
}

#[test]
fn test_process_properly_detached() {
    let temp_dir = TempDir::new().unwrap();

    // Start a short-lived process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "detach-test", "sleep", "1"])
        .assert()
        .success();

    // Get the PID from the pid file
    let pid_content = fs::read_to_string(temp_dir.path().join("detach-test.pid")).unwrap();
    let lines: Vec<&str> = pid_content.lines().collect();
    let pid: u32 = lines[0].trim().parse().unwrap();

    // Verify the process is initially running
    let output = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Process should be running initially"
    );

    // Wait for the process to complete
    std::thread::sleep(Duration::from_millis(1500));

    // Verify the process has completed
    let output = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success(), "Process should have completed");

    // Critical test: Check that the process is not a zombie
    // by examining /proc/PID/stat. A zombie process has state 'Z'
    let proc_stat_path = format!("/proc/{}/stat", pid);
    if std::path::Path::new(&proc_stat_path).exists() {
        let stat_content = fs::read_to_string(&proc_stat_path).unwrap();
        let fields: Vec<&str> = stat_content.split_whitespace().collect();
        if fields.len() > 2 {
            let state = fields[2];
            assert_ne!(
                state, "Z",
                "Process should not be in zombie state, but found state: {}",
                state
            );
        }
    }

    // Additional check: Verify that the process has been properly reaped
    // by checking if the /proc/PID directory still exists after a reasonable delay
    std::thread::sleep(Duration::from_millis(100));
    let proc_dir = format!("/proc/{}", pid);
    // If the directory still exists, the process might not have been properly reaped
    if std::path::Path::new(&proc_dir).exists() {
        // Double-check by reading the stat file again
        let stat_content = fs::read_to_string(&proc_stat_path).unwrap();
        let fields: Vec<&str> = stat_content.split_whitespace().collect();
        if fields.len() > 2 {
            let state = fields[2];
            // This should fail with the current implementation using std::mem::forget
            assert_ne!(
                state, "Z",
                "Process should not be in zombie state after completion"
            );
        }
    }
}

#[test]
fn test_improper_child_process_management() {
    let temp_dir = TempDir::new().unwrap();

    // This test specifically demonstrates the issue with std::mem::forget(child)
    // The current implementation fails to properly manage child process resources

    // Start a very short-lived process
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["run", "resource-test", "true"]) // 'true' command exits immediately
        .assert()
        .success();

    // Read the PID to confirm process was started
    let pid_content = fs::read_to_string(temp_dir.path().join("resource-test.pid")).unwrap();
    let lines: Vec<&str> = pid_content.lines().collect();
    let pid: u32 = lines[0].trim().parse().unwrap();

    // Give the process time to start and complete
    std::thread::sleep(Duration::from_millis(100));

    // Test the core issue: std::mem::forget prevents proper resource cleanup
    // With std::mem::forget, the Child struct's Drop implementation never runs
    // This can lead to resource leaks or zombie processes under certain conditions

    // Check for potential zombie state by examining /proc filesystem
    let proc_stat_path = format!("/proc/{}/stat", pid);

    // Even if the process completed quickly, we want to ensure proper cleanup
    // The issue is architectural: std::mem::forget is not the right approach

    // For now, let's just verify the process completed and was detached
    // The real fix will replace std::mem::forget with proper detachment

    // This assertion will pass now but documents the architectural issue
    // that will be fixed in the implementation

    println!(
        "Process {} started and managed with current std::mem::forget approach",
        pid
    );
    println!("Issue: std::mem::forget prevents Child destructor from running");
    println!("This can lead to resource leaks and improper process lifecycle management");

    // Force the test to fail to demonstrate the issue needs fixing
    // This documents that std::mem::forget is problematic for process management
    assert!(
        false,
        "Current implementation uses std::mem::forget(child) which is improper for process management - Child destructor should run for proper cleanup"
    );
}

// Tests for flag logic issues in cat and tail commands
// These tests demonstrate the incorrect behavior that needs to be fixed

#[test]
fn test_cat_flag_combinations() {
    let temp_dir = TempDir::new().unwrap();

    // Create a process that outputs to both stdout and stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&[
            "run",
            "flag_test",
            "--",
            "sh",
            "-c",
            "echo 'stdout content'; echo 'stderr content' >&2",
        ])
        .assert()
        .success();

    // Wait for process to complete
    std::thread::sleep(Duration::from_millis(100));

    // Test 1: No flags - should show both stdout and stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["cat", "flag_test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout content"))
        .stdout(predicate::str::contains("stderr content"));

    // Test 2: --stdout only - should show only stdout
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["cat", "flag_test", "--stdout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout content"))
        .stdout(predicate::str::contains("stderr content").not());

    // Test 3: --stderr only - should show only stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["cat", "flag_test", "--stderr"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stderr content"))
        .stdout(predicate::str::contains("stdout content").not());

    // Test 4: Both --stdout and --stderr - should show both
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["cat", "flag_test", "--stdout", "--stderr"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout content"))
        .stdout(predicate::str::contains("stderr content"));
}

#[test]
fn test_tail_flag_combinations() {
    let temp_dir = TempDir::new().unwrap();

    // Create a process that outputs to both stdout and stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&[
            "run",
            "tail_flag_test",
            "--",
            "sh",
            "-c",
            "echo 'stdout line'; echo 'stderr line' >&2",
        ])
        .assert()
        .success();

    // Wait for process to complete
    std::thread::sleep(Duration::from_millis(100));

    // Test 1: No flags - should show both stdout and stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["tail", "tail_flag_test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout line"))
        .stdout(predicate::str::contains("stderr line"));

    // Test 2: --stdout only - should show only stdout
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["tail", "tail_flag_test", "--stdout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout line"))
        .stdout(predicate::str::contains("stderr line").not());

    // Test 3: --stderr only - should show only stderr
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["tail", "tail_flag_test", "--stderr"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stderr line"))
        .stdout(predicate::str::contains("stdout line").not());

    // Test 4: Both --stdout and --stderr - should show both
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.env("DEMON_ROOT_DIR", temp_dir.path())
        .args(&["tail", "tail_flag_test", "--stdout", "--stderr"])
        .assert()
        .success()
        .stdout(predicate::str::contains("stdout line"))
        .stdout(predicate::str::contains("stderr line"));
}

#[test]
fn test_flag_logic_validation() {
    // This test validates the boolean logic directly
    // Let's check if the current implementation matches expected behavior

    // Test case 1: No flags (both false)
    let stdout_flag = false;
    let stderr_flag = false;
    let show_stdout = !stderr_flag || stdout_flag;
    let show_stderr = !stdout_flag || stderr_flag;
    assert!(show_stdout, "Should show stdout when no flags are set");
    assert!(show_stderr, "Should show stderr when no flags are set");

    // Test case 2: Only stdout flag (stdout=true, stderr=false)
    let stdout_flag = true;
    let stderr_flag = false;
    let show_stdout = !stderr_flag || stdout_flag;
    let show_stderr = !stdout_flag || stderr_flag;
    assert!(show_stdout, "Should show stdout when --stdout flag is set");
    assert!(
        !show_stderr,
        "Should NOT show stderr when only --stdout flag is set"
    );

    // Test case 3: Only stderr flag (stdout=false, stderr=true)
    let stdout_flag = false;
    let stderr_flag = true;
    let show_stdout = !stderr_flag || stdout_flag;
    let show_stderr = !stdout_flag || stderr_flag;
    assert!(
        !show_stdout,
        "Should NOT show stdout when only --stderr flag is set"
    );
    assert!(show_stderr, "Should show stderr when --stderr flag is set");

    // Test case 4: Both flags (both true)
    let stdout_flag = true;
    let stderr_flag = true;
    let show_stdout = !stderr_flag || stdout_flag;
    let show_stderr = !stdout_flag || stderr_flag;
    assert!(show_stdout, "Should show stdout when both flags are set");
    assert!(show_stderr, "Should show stderr when both flags are set");
}

#[test]
fn test_readme_contains_correct_tail_syntax() {
    // This test ensures the README.md file contains the correct "demon tail -f" syntax
    let project_root = env!("CARGO_MANIFEST_DIR");
    let readme_path = format!("{}/README.md", project_root);
    let readme_content =
        std::fs::read_to_string(&readme_path).expect("README.md should exist and be readable");

    // The README should contain "demon tail -f" syntax, not "demon tail =f"
    assert!(
        readme_content.contains("demon tail -f"),
        "README.md should contain 'demon tail -f' syntax"
    );

    // Ensure it doesn't contain the incorrect syntax
    assert!(
        !readme_content.contains("demon tail =f"),
        "README.md should not contain incorrect 'demon tail =f' syntax"
    );
}
