# Demon CLI Improvement Plan

## Overview
This document outlines the planned improvements to the demon CLI tool based on feedback and best practices.

## Improvement Tasks

### 1. Switch to `anyhow` for Error Handling
**Priority**: High  
**Status**: Pending

**Goal**: Replace `Box<dyn std::error::Error>` with `anyhow::Result` throughout the codebase for better error handling.

**Tasks**:
- Replace all `Result<(), Box<dyn std::error::Error>>` with `anyhow::Result<()>`
- Use `anyhow::Context` for better error context
- Simplify error handling code
- Update imports and error propagation

**Benefits**:
- Better error messages with context
- Simpler error handling
- More idiomatic Rust error handling

### 2. Implement CLI Testing with `assert_cmd`
**Priority**: High  
**Status**: Pending

**Goal**: Create comprehensive integration tests for all CLI commands using the `assert_cmd` crate.

**Prerequisites**:
- Research and document `assert_cmd` usage in CLAUDE.md
- Add `assert_cmd` dependency
- Create test infrastructure

**Test Coverage Required**:
- `demon run`: Process spawning, file creation, duplicate detection
- `demon stop`: Process termination, timeout handling, cleanup
- `demon tail`: File watching behavior (basic scenarios)
- `demon cat`: File content display, flag handling
- `demon list`: Process listing, status detection
- `demon status`: Individual process status checks
- `demon clean`: Orphaned file cleanup
- Error scenarios: missing files, invalid PIDs, etc.

**Test Structure**:
```
tests/
├── cli.rs              # Main CLI integration tests
├── fixtures/           # Test data and helper files
└── common/             # Shared test utilities
```

### 3. Add Quiet Flag to List Command
**Priority**: Medium  
**Status**: Pending

**Goal**: Add `-q/--quiet` flag to the `demon list` command for machine-readable output.

**Requirements**:
- Add `quiet` field to `ListArgs` struct (if needed, since `List` currently has no args)
- Convert `List` command to use `ListArgs` struct
- When quiet flag is used:
  - No headers
  - One line per process: `id:pid:status`
  - No "No daemon processes found" message when empty

**Example Output**:
```bash
# Normal mode
$ demon list
ID                   PID      STATUS     COMMAND
--------------------------------------------------
my-app               12345    RUNNING    N/A

# Quiet mode
$ demon list -q
my-app:12345:RUNNING
```

### 4. Add LLM Command
**Priority**: Medium  
**Status**: Pending

**Goal**: Add a `demon llm` command that outputs a comprehensive usage guide for other LLMs.

**Requirements**:
- Add `Llm` variant to `Commands` enum
- No arguments needed
- Output to stdout (not stderr like other messages)
- Include all commands with examples
- Assume the reader is an LLM that needs to understand how to use the tool

**Content Structure**:
- Tool overview and purpose
- All available commands with syntax
- Practical examples for each command
- Common workflows
- File structure explanation
- Error handling tips

### 5. Remove `glob` Dependency
**Priority**: Low  
**Status**: Pending

**Goal**: Replace the `glob` crate with standard library `std::fs` functionality.

**Implementation**:
- Remove `glob` from Cargo.toml
- Replace `glob("*.pid")` with `std::fs::read_dir()` + filtering
- Update imports
- Ensure same functionality is maintained

**Functions to Update**:
- `list_daemons()`: Find all .pid files
- `clean_orphaned_files()`: Find all .pid files

**Implementation Pattern**:
```rust
// Replace glob("*.pid") with:
std::fs::read_dir(".")?
    .filter_map(|entry| entry.ok())
    .filter(|entry| {
        entry.path().extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "pid")
            .unwrap_or(false)
    })
```

## Implementation Order

1. **Document assert_cmd** - Add understanding to CLAUDE.md
2. **Switch to anyhow** - Foundation for better error handling
3. **Implement tests** - Ensure current functionality works correctly
4. **Add quiet flag** - Small feature addition
5. **Add LLM command** - Documentation feature
6. **Remove glob** - Cleanup and reduce dependencies

## Success Criteria

- [ ] All existing functionality remains intact
- [ ] Comprehensive test coverage (>80% of CLI scenarios)
- [ ] Better error messages with context
- [ ] Machine-readable list output option
- [ ] LLM-friendly documentation command
- [ ] Reduced dependency footprint
- [ ] All changes committed with proper messages

## Risk Assessment

**Low Risk**:
- anyhow migration (straightforward replacement)
- quiet flag addition (additive change)
- LLM command (new, isolated feature)

**Medium Risk**:
- glob removal (need to ensure exact same behavior)
- CLI testing (need to handle file system interactions carefully)

## Notes

- Each improvement should be implemented, tested, and committed separately
- Maintain backward compatibility for all existing commands
- Update IMPLEMENTATION_PLAN.md as work progresses
- Consider adding integration tests that verify the actual daemon functionality