# Demon CLI Improvement Plan v2

## Overview
This plan outlines four major improvements to enhance the demon CLI tool's usability, error handling, and documentation.

## Task 1: Rename PidFileData to PidFile

### Rationale
- Shorter, cleaner name
- More intuitive - it represents a PID file, not just data about one
- Follows Rust naming conventions better

### Implementation Steps
1. Rename struct `PidFileData` to `PidFile`
2. Update all references throughout the codebase
3. Update comments and documentation
4. Verify compilation and tests pass

### Files to modify
- `src/main.rs` - struct definition and all usages

### Risk Assessment
- **Low risk** - Simple rename refactor
- No functional changes
- All tests should continue to pass

## Task 2: Implement PidFileReadError Enum

### Rationale
- Better error handling with specific error types
- Eliminates redundant file existence checks
- More idiomatic Rust error handling
- Cleaner code in error handling paths

### Design
```rust
#[derive(Debug)]
pub enum PidFileReadError {
    /// The PID file does not exist
    FileNotFound,
    /// The PID file exists but has invalid content
    FileInvalid(String), // Include reason for invalidity
    /// IO error occurred while reading
    IoError(std::io::Error),
}
```

### Implementation Steps
1. Define `PidFileReadError` enum with appropriate variants
2. Implement `Display` and `Error` traits for the enum
3. Update `PidFile::read_from_file()` to return `Result<PidFile, PidFileReadError>`
4. Update all call sites to handle the specific error types:
   - `is_process_running()` - handle FileNotFound and FileInvalid as "not running"
   - `stop_daemon()` - handle FileNotFound as "not running", FileInvalid as "cleanup needed"
   - `list_daemons()` - handle FileInvalid as "INVALID" entry
   - `status_daemon()` - handle FileNotFound as "NOT FOUND", FileInvalid as "ERROR"
   - `clean_orphaned_files()` - handle FileInvalid as "needs cleanup"
5. Remove redundant `Path::new().exists()` checks where the error type provides this info
6. Test all error scenarios

### Files to modify
- `src/main.rs` - enum definition, read_from_file method, all usage sites

### Risk Assessment
- **Medium risk** - Changes error handling logic
- Need thorough testing of error scenarios
- Must ensure all edge cases are handled properly

## Task 3: Make --id a Positional Argument

### Analysis

#### Current CLI Pattern
```bash
demon run --id web-server python -m http.server 8080
demon stop --id web-server
demon status --id web-server
```

#### Proposed CLI Pattern
```bash
demon run web-server python -m http.server 8080
demon stop web-server
demon status web-server
```

#### Pros
- **Better UX**: More natural and concise
- **Consistent with common tools**: Similar to git, docker, etc.
- **Faster to type**: No --id flag needed
- **More intuitive**: ID naturally comes first before the command

#### Cons
- **Breaking change**: Existing scripts/users need to update
- **Potential ambiguity**: ID could be confused with command in some cases
- **Parsing complexity**: Need careful handling of edge cases

#### Design Decisions
1. **Make ID positional for all commands that currently use --id**
2. **Keep -- separator support** for complex commands
3. **Update help text** to reflect new usage
4. **Maintain backward compatibility** by supporting both patterns initially (with deprecation warning)

#### Commands to Update
- `run <id> <command...>` - ID becomes first positional arg
- `stop <id>` - ID becomes positional arg, remove timeout flag positioning issues
- `tail <id>` - ID becomes positional arg
- `cat <id>` - ID becomes positional arg  
- `status <id>` - ID becomes positional arg

#### Implementation Strategy
1. **Phase 1**: Support both patterns with deprecation warnings
2. **Phase 2**: Remove old pattern support (future version)

### Implementation Steps
1. Define new argument structures with positional ID fields
2. Update clap derive macros to make ID positional
3. Update help text and documentation strings
4. Add deprecation warnings for --id usage (optional)
5. Update all internal function calls
6. Update tests to use new CLI pattern
7. Update LLM guide output

### Files to modify
- `src/main.rs` - argument structures, help text
- `tests/cli.rs` - all test commands
- LLM guide text

### Risk Assessment
- **High risk** - Breaking change for users
- Need to update all tests
- Must carefully verify argument parsing edge cases
- Consider gradual migration strategy

## Task 4: Write Comprehensive README.md

### Target Audience
- Developers who need background process management
- LLM agents and their operators
- DevOps engineers running long-term tasks
- Anyone working with npm run dev, build processes, etc.

### Content Structure
```markdown
# Demon - Background Process Manager

## Overview
Brief description focusing on core value proposition

## Installation
- `cargo install demon` (when published)
- Building from source
- System requirements

## Quick Start
- Basic examples
- Common workflows

## Use Cases
- Development servers (npm run dev)
- Background tasks and scripts
- LLM agent process management
- CI/CD pipeline tasks
- Long-running computations

## Command Reference
- Complete command documentation
- Examples for each command
- Common flags and options

## Integration with LLM Agents
- How agents can use demon
- Machine-readable output formats
- Best practices for automation

## Advanced Usage
- File management
- Process lifecycle
- Troubleshooting
- Performance considerations

## Contributing
- Development setup
- Testing
- Contribution guidelines
```

### Key Messages
1. **Simplicity**: Easy background process management
2. **Visibility**: Always know what's running and its status
3. **Integration**: Built for automation and LLM agents
4. **Reliability**: Robust process lifecycle management

### Implementation Steps
1. Research similar tools for README inspiration
2. Write comprehensive content covering all sections
3. Include practical examples and screenshots/command outputs
4. Add badges for build status, crates.io, etc. (when applicable)
5. Review and refine for clarity and completeness

### Files to create
- `README.md` - comprehensive documentation

### Risk Assessment
- **Low risk** - Documentation only
- No functional changes
- Easy to iterate and improve

## Execution Order

1. **Task 1**: Rename PidFileData to PidFile (Low risk, enables clean foundation)
2. **Task 2**: Implement PidFileReadError enum (Medium risk, improves error handling)
3. **Task 3**: Make --id positional (High risk, but significant UX improvement)
4. **Task 4**: Write README.md (Low risk, improves project presentation)

## Testing Strategy

After each task:
1. Run `cargo build` to ensure compilation
2. Run `cargo test` to ensure all tests pass
3. Manual testing of affected functionality
4. Format code with `cargo fmt`
5. Commit changes with descriptive message

## Success Criteria

- All tests pass after each change
- No regressions in functionality
- Improved error messages and handling
- Better CLI usability
- Comprehensive documentation
- Clean, maintainable code

## Rollback Plan

Each task will be committed separately, allowing for easy rollback if issues arise:
1. Git commit after each successful task
2. If issues found, can revert specific commits
3. Tests provide safety net for functionality

## Timeline Estimate

- Task 1: 15-20 minutes (straightforward refactor)
- Task 2: 30-45 minutes (error handling logic)
- Task 3: 45-60 minutes (CLI argument changes + tests)
- Task 4: 30-45 minutes (documentation writing)

Total: ~2-3 hours