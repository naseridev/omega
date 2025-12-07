# Omega Wiki

Technical documentation and developer guide for the Omega file search tool.

## Table of Contents

- [Project Overview](#project-overview)
- [Architecture](#architecture)
- [Core Components](#core-components)
- [Algorithm Details](#algorithm-details)
- [Performance Characteristics](#performance-characteristics)
- [Building and Development](#building-and-development)
- [Testing](#testing)
- [Contributing](#contributing)
- [Advanced Usage](#advanced-usage)

## Project Overview

### Purpose

Omega is a high-performance file search utility designed to efficiently search large filesystems using parallel processing. It addresses the need for fast, flexible file discovery across different operating systems.

### Design Goals

- Maximum search performance through parallelization
- Cross-platform compatibility (Linux, macOS, Windows)
- Low memory footprint even on large filesystems
- Flexible search options (fuzzy matching, content search, filtering)
- Both human-readable and machine-readable output formats

### Technology Stack

- Language: Rust (stable)
- Key Dependencies:
  - `clap`: Command-line argument parsing
  - `rayon`: Data parallelism
  - `crossbeam`: Lock-free concurrent data structures
  - `walkdir`: Filesystem traversal

## Architecture

### System Design

Omega follows a modular architecture with clear separation of concerns:

```
System Design:

CLI Layer
  - Parse arguments
  |
  v
Engine Layer
  - Orchestrate search
  |
  +-- Scanner (Filesystem traversal)
  +-- Matcher (Pattern matching)
  +-- Metrics (Statistics tracking)
  +-- Printer (Result output)
```

### Threading Model

Omega uses a producer-consumer pattern:

1. Main thread creates a Rayon thread pool
2. Multiple scanner threads traverse directories in parallel
3. Found files are sent through a lock-free channel
4. A dedicated printer thread consumes and outputs results
5. Atomic counters track progress across threads

### Data Flow

```
Data Flow:

1. Input Args
2. Search Roots Determination
3. Parallel Directory Traversal
4. Pattern Matching + Filtering
5. Channel Communication
6. Result Output + Metrics
```

## Core Components

### cli.rs

Defines the command-line interface using the `clap` parser. All search parameters are encapsulated in the `Args` struct with validation annotations.

Key responsibilities:
- Parse command-line arguments
- Provide help text and version information
- Validate argument combinations

### engine.rs

The orchestration layer that coordinates the search process.

**SearchEngine struct:**
- Initializes scanner, matcher, and output components
- Creates thread pool based on configuration
- Manages the producer-consumer pipeline
- Handles graceful shutdown

**Search flow:**
1. Build Rayon thread pool
2. Create unbounded channel for results
3. Spawn printer thread
4. Execute parallel directory scans
5. Wait for completion and collect metrics

### scanner.rs

Implements filesystem traversal and filtering logic.

**FileSystemScanner:**
- Uses `walkdir` for depth-first traversal
- Applies filtering based on file type (files/directories)
- Respects depth limits
- Handles symbolic links (does not follow)
- Checks access permissions before traversal

**SearchConfig:**
- Thread count (auto-detected from CPU)
- Maximum depth limit
- File/directory filtering flags

Key methods:
- `scan_directory`: Main traversal logic
- `can_access_path`: Permission checking

### matcher.rs

Pattern matching engine supporting multiple match modes.

**PatternMatcher:**
- Exact substring matching (default)
- Case-sensitive and case-insensitive modes
- Fuzzy matching using Levenshtein distance
- Content search for text files

**Matching strategies:**

1. **Exact Match:**
   - Simple substring containment
   - Case handling via string transformation

2. **Fuzzy Match:**
   - First tries exact match
   - Falls back to word-level Levenshtein distance
   - Configurable distance threshold

3. **Content Match:**
   - Reads file contents as UTF-8 string
   - Applies same matching logic as filenames
   - Respects maximum file size limit

### metrics.rs

Thread-safe metrics collection using atomic operations.

**SearchMetrics:**
- Found count: Number of matching items
- Scanned count: Total items examined
- Error count: Access/read failures
- Shutdown flag: Coordinate early termination

All counters use `AtomicU64` with relaxed ordering for performance.

**SearchLimits:**
- Optional limits on found/scanned counts
- Triggers shutdown when limits reached

**Error logging:**
- Concurrent file writing with mutex protection
- Timestamped error entries
- Filters out common "Access denied" errors

### output.rs

Handles result formatting and printing.

**OutputMode:**
- Normal: Simple path output
- API: CSV format with detailed metadata

**ResultPrinter:**
- Runs in dedicated thread
- Consumes results from channel
- Prints in chosen format
- No buffering for real-time output

**SearchResult:**
- Final summary statistics
- Formatted output to stderr (doesn't interfere with piped results)

### file_info.rs

Rich file metadata representation.

**FileInfo struct fields:**
- path: Full path string
- name: Filename only
- is_dir, is_file: Type flags
- size: Bytes and human-readable format
- modified: Unix timestamp and ISO 8601 string
- is_hidden: Platform-specific detection
- extension: File extension
- permissions: Unix-style permission string

**CSV serialization:**
- Proper escaping of special characters
- Quote wrapping for fields with commas/newlines

### paths.rs

Platform-specific root path determination.

**Windows:**
- Checks drives C:\ through Z:\
- Tests existence of each drive letter
- Returns all available drives

**Unix (Linux/macOS):**
- Always uses "/" as root

**Custom paths:**
- Validates existence before use
- Can specify multiple paths via repeated `-p` flag

### utils.rs

Utility functions for formatting and platform compatibility.

**Functions:**
- `escape_csv`: Proper CSV field escaping
- `format_size`: Human-readable byte sizes (B, KB, MB, GB, TB)
- `format_timestamp`: Unix epoch to ISO 8601 conversion
- `is_hidden_file`: Platform-specific hidden file detection
- `format_permissions`: Unix-style permission strings

**Date conversion:**
- Custom implementation avoiding external dependencies
- Handles leap years correctly
- Unix epoch (1970-01-01) based calculations

## Algorithm Details

### Levenshtein Distance

Omega implements the Wagner-Fischer dynamic programming algorithm for computing edit distance:

```rust
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();

    // Base cases
    if len1 == 0 { return len2; }
    if len2 == 0 { return len1; }

    // Dynamic programming with space optimization
    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row = vec![0; len2 + 1];

    for (i, &ch1) in s1_chars.iter().enumerate() {
        curr_row[0] = i + 1;

        for j in 0..len2 {
            let cost = if ch1 == s2_chars[j] { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)      // insertion
                .min(prev_row[j + 1] + 1)            // deletion
                .min(prev_row[j] + cost);            // substitution
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[len2]
}
```

**Complexity:**
- Time: O(m * n) where m and n are string lengths
- Space: O(min(m, n)) using row optimization

**Fuzzy matching strategy:**
1. Split target string into words (alphanumeric boundaries)
2. Compare each word against patterns
3. Match if any word is within threshold distance
4. Default threshold is 2 edits

### Parallel Traversal

Rayon's parallel iterator splits work across threads:

```rust
roots.par_iter().for_each(|root| {
    self.scanner.scan_directory(root, tx.clone());
});
```

Each root path is independently processed. Within each root, `walkdir` provides a depth-first iterator that is consumed sequentially per thread.

**Work distribution:**
- Multiple roots: Rayon distributes across threads
- Single root: Less parallelism, but still benefits from async I/O
- Work stealing: Rayon balances load automatically

### Early Termination

Omega supports early termination via limits:

```rust
pub fn should_continue(&self, metrics: &SearchMetrics) -> bool {
    if metrics.is_shutdown() {
        return false;
    }

    if let Some(limit) = self.found {
        if metrics.get_found() >= limit {
            metrics.trigger_shutdown();
            return false;
        }
    }

    // Similar check for scanned limit
    true
}
```

The shutdown flag uses `AtomicBool` to propagate termination across threads without locks.

## Performance Characteristics

### Time Complexity

**Directory traversal:**
- O(N) where N is total filesystem entries
- Bounded by I/O speed and filesystem cache

**Pattern matching:**
- Exact: O(m * k) where m is filename length, k is pattern count
- Fuzzy: O(m * w * k * p) where w is word count, p is pattern length

**Content search:**
- Adds O(f * c) where f is file size, c is content matching complexity
- Significantly slower than filename matching

### Space Complexity

**Memory usage:**
- O(D) for depth-first traversal where D is max depth
- O(T) for channel buffer where T is thread count
- O(1) for metrics (atomic counters)
- FileInfo objects are streamed, not accumulated

**Typical memory footprint:**
- Base: ~5-10 MB
- Per thread: ~1-2 MB stack
- Channel: Unbounded but flows quickly
- No large data structures retained

### I/O Characteristics

**Filesystem operations:**
- Mostly sequential reads (directory listings)
- Metadata reads for each entry
- Content reads only when content search enabled
- Benefits from OS filesystem cache

**Bottlenecks:**
- Cold cache: 100-1000x slower than warm cache
- Network filesystems: High latency impact
- Spinning disks: Sequential access helps
- SSDs: Parallel access is effective

### Scalability

**Thread scaling:**
- Near-linear speedup with multiple roots
- Diminishing returns on single root
- Optimal thread count typically equals CPU cores
- I/O bound more than CPU bound

**Dataset scaling:**
- Linear time growth with filesystem size
- Constant memory usage regardless of size
- Early termination limits worst-case time

## Building and Development

### Prerequisites

- Rust toolchain (1.70.0 or later recommended)
- Cargo package manager

### Building from Source

```bash
# Clone repository
git clone https://github.com/naseridev/omega.git
cd omega

# Build debug version
cargo build

# Build optimized release version
cargo build --release

# Run tests
cargo test

# Generate documentation
cargo doc --open
```

### Project Structure

```
Project Structure:

omega/
  - src/
    - main.rs          (Entry point)
    - lib.rs           (Library root)
    - cli.rs           (Argument parsing)
    - engine.rs        (Search orchestration)
    - scanner.rs       (Filesystem traversal)
    - matcher.rs       (Pattern matching)
    - metrics.rs       (Statistics tracking)
    - output.rs        (Result formatting)
    - file_info.rs     (File metadata)
    - paths.rs         (Root path logic)
    - utils.rs         (Utility functions)
  - Cargo.toml         (Dependencies)
  - README.md          (User guide)
```

### Dependencies

**Core dependencies:**
- `clap = "4.x"` - CLI parsing with derive macros
- `rayon = "1.x"` - Data parallelism
- `crossbeam = "0.8"` - Lock-free channels
- `walkdir = "2.x"` - Directory traversal

**Platform-specific:**
- Unix: Uses `std::os::unix::fs::PermissionsExt`
- Windows: Uses `std::os::windows::fs::MetadataExt`

### Build Optimizations

**Release profile in Cargo.toml:**

```toml
[profile.release]
opt-level = 3           # Maximum optimization
lto = true              # Link-time optimization
codegen-units = 1       # Better optimization
strip = true            # Remove debug symbols
```

### Code Style

Omega follows Rust standard conventions:
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Maximum line length: 100 characters
- Prefer explicit types in public APIs

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_levenshtein_distance

# Run with coverage (requires tarpaulin)
cargo tarpaulin --out Html
```

### Test Categories

**Unit tests:**
- Levenshtein distance correctness
- CSV escaping edge cases
- Timestamp formatting
- Permission string formatting

**Integration tests:**
- End-to-end search scenarios
- Multi-threaded correctness
- Limit enforcement
- Error handling

**Manual testing:**

```bash
# Create test directory structure
mkdir -p test_data/dir1/dir2
echo "test content" > test_data/file1.txt
echo "other data" > test_data/dir1/file2.log

# Test basic search
omega test -p test_data

# Test content search
omega "content" -c -p test_data

# Test fuzzy search
omega tset -z -p test_data

# Test CSV output
omega test --api -p test_data > results.csv
```

## Contributing

### Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Make changes with clear commit messages
4. Add tests for new functionality
5. Ensure all tests pass: `cargo test`
6. Format code: `cargo fmt`
7. Check lints: `cargo clippy`
8. Submit pull request with description

### Contribution Areas

**Priority improvements:**
- Additional pattern matching algorithms (regex, glob)
- Interactive TUI mode with progress display
- Incremental search with result updates
- Configuration file support
- Bookmark/saved search functionality

**Code quality:**
- Expand test coverage (target: 80%+)
- Add benchmarks for performance tracking
- Improve error messages
- Add more inline documentation

**Platform support:**
- Extended attribute handling (macOS)
- Windows registry search
- Network path optimization

### Code Review Criteria

- Correctness: Does it work as intended?
- Performance: No significant regressions
- Safety: Proper error handling, no panics
- Style: Follows Rust conventions
- Tests: Adequate test coverage
- Documentation: Clear comments and docs

## Advanced Usage

### Combining with Other Tools

**Using with grep:**

```bash
# Find and filter results
omega .rs --api | grep "src/" | cut -d, -f1

# Count matches by extension
omega pattern --api | awk -F, '{print $10}' | sort | uniq -c
```

**Using with xargs:**

```bash
# Delete found files (use with caution!)
omega .tmp -f | xargs rm

# Copy found files to directory
omega .pdf -f | xargs -I {} cp {} /backup/
```

**Using with jq (convert CSV to JSON):**

```bash
omega pattern --api > results.csv
# Convert CSV to JSON using external tool
```

### Scripting Examples

**Bash script to archive old files:**

```bash
#!/bin/bash
# Find and archive files older than 1 year

omega ".log" -f --api | while IFS=, read -r path rest; do
    # Extract timestamp, compare with current time
    # Archive if older than threshold
    tar -czf archive.tar.gz "$path"
done
```

**Python script for analysis:**

```python
import csv
import subprocess

# Run omega and capture CSV output
result = subprocess.run(
    ['omega', 'pattern', '--api'],
    capture_output=True,
    text=True
)

# Parse CSV
reader = csv.DictReader(result.stdout.splitlines())
for row in reader:
    size = int(row['size'])
    if size > 1000000:  # Files > 1MB
        print(f"Large file: {row['path']}")
```

### Performance Tuning Guide

**For maximum speed:**

```bash
# Use many threads, limit results
omega pattern -t 16 -l 100

# Search specific paths only
omega pattern -p /specific/directory

# Avoid content search unless needed
omega pattern  # filename only
```

**For thorough search:**

```bash
# Include content, use fuzzy matching
omega pattern -c -z

# No limits, search everything
omega pattern  # no -l or -s flags
```

**For large filesystems:**

```bash
# Limit depth to avoid deep traversal
omega pattern -d 4

# Use scan limit to sample filesystem
omega pattern -s 100000
```

### Common Patterns

**Find configuration files:**

```bash
omega config -i  # case-insensitive
omega .conf .cfg .ini -f  # by extension
```

**Find source code files:**

```bash
omega .rs .go .py -f
omega "func main" -c  # by content
```

**Find large files:**

```bash
omega "" --api | awk -F, '$5 > 1000000000 {print $1,$6}'
```

**Find recently modified:**

```bash
omega "" --api | sort -t, -k7 -rn | head -n 20
```

## Troubleshooting

### Common Issues

**Issue: Search is too slow**

Solutions:
- Use `-p` to limit search scope
- Increase threads with `-t`
- Add limits with `-l` or `-s`
- Disable content search if not needed

**Issue: Too many errors**

Solutions:
- Use `-e` to hide error count
- Check file permissions
- Review `omega.log` for details
- Exclude problematic directories

**Issue: Out of memory**

Solutions:
- Reduce thread count with `-t`
- Use scan limit with `-s`
- Avoid searching network filesystems

**Issue: Wrong results**

Solutions:
- Check case sensitivity (use `-i`)
- Verify fuzzy threshold with `-T`
- Ensure patterns are correct
- Test with `--api` to see metadata

### Debugging

**Enable verbose error logging:**

```bash
# Errors always logged to omega.log
tail -f omega.log  # monitor in real-time
```

**Test with small dataset:**

```bash
mkdir test && cd test
touch file1.txt file2.log
omega txt  # should find file1.txt
```

**Verify thread behavior:**

```bash
omega pattern -t 1  # single-threaded
omega pattern -t 4  # multi-threaded
# Compare results to ensure consistency
```
## Resources

### Documentation

- User Guide: [README.md](README.md)
- API Documentation: `cargo doc --open`
- Code Comments: Inline documentation in source files

### Community

- Issues: GitHub Issues for bug reports
- Discussions: GitHub Discussions for questions
- Pull Requests: Contributions welcome

### Related Projects

- ripgrep: Fast grep alternative (content search focused)
- fd: Fast find alternative (simpler but less features)
- fzf: Fuzzy finder (interactive selection)
- ag (The Silver Searcher): Fast code search

## License

This project is open source. Please refer to the LICENSE file in the repository for specific terms and conditions.

## Acknowledgments

This project builds upon the excellent work of the Rust community and leverages several high-quality open-source libraries. Special thanks to all contributors and maintainers of the dependencies used in this project.

## Contact

**Author:** Nima Naseri

For questions, suggestions, or contributions, please use GitHub Issues or submit a pull request.
