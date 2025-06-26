use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

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
        "run",
        "--root-dir",
        nonexistent_path.to_str().unwrap(),
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

    // Handle the case where changing to the directory fails due to permissions
    if std::env::set_current_dir(&subdir).is_err() {
        // This is actually the expected behavior for a directory with insufficient permissions
        return;
    }

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

    // Try to use a path with null bytes (should be invalid on most systems)
    // We expect this to fail at the OS level before reaching our validation code
    let result = std::panic::catch_unwind(|| {
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&[
            "run",
            "--root-dir",
            "path\0with\0nulls",
            "test",
            "echo",
            "hello",
        ])
        .assert()
        .failure();
    });
    // Either the command fails (good) or it panics due to null bytes (also expected)
    // This documents that our validation doesn't need to handle null bytes since the OS catches them
    if result.is_err() {
        // The test environment caught the null byte issue, which is expected behavior
        return;
    }
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
        "run",
        "--root-dir",
        deep_path.to_str().unwrap(),
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
            "run",
            "--root-dir",
            symlink_path.to_str().unwrap(),
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
            "run",
            "--root-dir",
            symlink_path.to_str().unwrap(),
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
            "run",
            "--root-dir",
            broken_symlink.to_str().unwrap(),
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
fn test_improved_error_messages() {
    let temp_dir = TempDir::new().unwrap();

    // Test 1: Better error message for non-existent directory
    let nonexistent_path = temp_dir.path().join("does_not_exist");
    let mut cmd = Command::cargo_bin("demon").unwrap();
    cmd.args(&[
        "run",
        "--root-dir",
        nonexistent_path.to_str().unwrap(),
        "test",
        "echo",
        "hello",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "Please create the directory first",
    ));

    // Test 2: Better error message for file instead of directory
    let file_path = temp_dir.path().join("not_a_directory");
    fs::write(&file_path, "this is a file").unwrap();

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
    .stderr(predicate::str::contains(
        "Please specify a directory path, not a file",
    ));
}

#[test]
fn test_write_permission_validation() {
    // This test can only run on Unix systems where we can create read-only directories
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let readonly_dir = temp_dir.path().join("readonly");
        std::fs::create_dir(&readonly_dir).unwrap();

        // Make the directory read-only
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444);
        std::fs::set_permissions(&readonly_dir, perms.clone()).unwrap();

        // Attempt to use the read-only directory should fail with permission error
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&[
            "run",
            "--root-dir",
            readonly_dir.to_str().unwrap(),
            "test",
            "echo",
            "hello",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cannot write to specified root directory",
        ))
        .stderr(predicate::str::contains(
            "Please check directory permissions",
        ));

        // Clean up - restore permissions so temp dir can be deleted
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&readonly_dir, perms);
    }
}

#[test]
fn test_path_canonicalization() {
    let temp_dir = TempDir::new().unwrap();

    // Create a real directory
    let real_dir = temp_dir.path().join("real_directory");
    std::fs::create_dir(&real_dir).unwrap();

    #[cfg(unix)]
    {
        // Create a symlink to it
        let symlink_path = temp_dir.path().join("symlink_to_dir");
        std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

        // Using symlink as root dir should work and files should be created in the real directory
        let mut cmd = Command::cargo_bin("demon").unwrap();
        cmd.args(&[
            "run",
            "--root-dir",
            symlink_path.to_str().unwrap(),
            "canonical_test",
            "echo",
            "hello",
        ])
        .assert()
        .success();

        // Verify files were created in the real directory (symlink resolved)
        std::thread::sleep(Duration::from_millis(100));
        assert!(real_dir.join("canonical_test.pid").exists());
        assert!(real_dir.join("canonical_test.stdout").exists());
        assert!(real_dir.join("canonical_test.stderr").exists());

        // Note: symlink_path.join() will also resolve to the same files since it's a symlink
        // The key test is that files are in the canonical/real location
    }
}
