# Omega File Search

A blazing fast, cross-platform file search utility built with Rust. Omega leverages parallel processing and efficient directory traversal to provide rapid file system searches with advanced filtering capabilities.

## Overview

Omega is designed to search through file systems at scale, utilizing multi-threaded scanning and pattern matching capabilities. The application supports various search configurations including custom paths, depth limits, result limits, and multiple output modes.

## Architecture

The project follows a modular architecture with clear separation of concerns:

- **Pattern Matching**: Handles search pattern processing and matching logic
- **File System Scanner**: Manages directory traversal and file discovery
- **Metrics Collection**: Tracks search progress, statistics, and errors
- **Result Management**: Handles output formatting and result delivery
- **Progress Reporting**: Provides real-time feedback during search operations
- **Path Provider**: Manages search root configuration for system-wide or targeted searches

## Features

- Multi-threaded parallel file system scanning
- Cross-platform support (Windows, Linux, macOS)
- Custom path targeting with multiple path support
- Case-sensitive and case-insensitive search modes
- File-only or directory-only filtering
- Configurable search depth limitation
- Result count and scan count limits
- Real-time progress reporting
- Multiple output modes: normal, quiet, and verbose
- Symbolic link following support
- Error tracking and reporting
- Automatic thread pool optimization
- Size formatting in verbose mode

## Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo package manager

### Build from Source

```bash
git clone <repository-url>
```

```bash
cd omega
```

```bash
cargo build --release
```

The compiled binary will be available at `target/release/omega`.

## Usage

### Basic Syntax

```bash
omega [OPTIONS] <PATTERNS>...
```

### Arguments

- `<PATTERNS>...`: One or more search patterns (required)

### Options

#### Search Configuration

- `-p, --path <PATH>`: Search paths (can be used multiple times for multiple paths)
- `-i, --case-insensitive`: Enable case-insensitive search (default: case-sensitive)
- `-d, --max-depth <DEPTH>`: Maximum search depth in directory tree
- `--follow-links`: Follow symbolic links during search

#### Filtering

- `-f, --files-only`: Search only files
- `-D, --dirs-only`: Search only directories

#### Limits

- `-l, --limit-found <COUNT>`: Limit the number of results found
- `-s, --limit-scanned <COUNT>`: Limit the number of items scanned

#### Performance

- `-t, --threads <COUNT>`: Specify number of threads (default: auto-detected)

#### Output

- `-q, --quiet`: Quiet mode - only print file paths
- `-v, --verbose`: Verbose mode - show detailed information including file sizes
- `-e, --show-errors`: Display errors encountered during search

### Examples

#### Basic Search

Search for files containing "config" in their name:

```bash
omega config
```

#### Custom Path Search

Search in specific directory:

```bash
omega config -p /etc
```

Search in multiple directories:

```bash
omega config -p /etc -p /var -p /home
```

#### Combined Short Options

Case-insensitive quiet search with path:

```bash
omega -qip /home document
```

#### Advanced Filtering

Search only files with result limit:

```bash
omega -f -l 100 readme
```

Search only directories with depth limit:

```bash
omega -D -d 3 lib
```

#### Performance Tuning

Custom thread count with scan limit:

```bash
omega -t 8 -s 10000 report
```

#### Verbose Output

Detailed information with file sizes:

```bash
omega -v config
```

Follow symbolic links with error reporting:

```bash
omega --follow-links -e document
```

#### Multiple Patterns

Search for multiple patterns simultaneously:

```bash
omega readme license changelog
```

## Output Format

### Normal Mode

Results are displayed with file type markers:

```
[F] /path/to/file.txt
[D] /path/to/directory
```

Progress information is shown on stderr:

```
omega: 125 scanned | 15 found
```

Final summary:

```
omega: 15 found in 2.34s (53/s)
```

### Quiet Mode

Only file paths are printed to stdout:

```
/path/to/file.txt
/path/to/directory
```

No progress or summary information is displayed.

### Verbose Mode

Detailed information including file type and size:

```
[FILE]      1.25 MB /path/to/document.pdf
[DIR ]     unknown /path/to/directory
```

Final summary with error count:

```
omega: 15 found in 2.34s (53/s) | 3 errors
```

## Performance Considerations

- Thread count is automatically optimized based on available CPU cores
- The application uses Rayon for work-stealing parallelism
- WalkDir provides efficient directory traversal with minimal allocations
- Atomic operations ensure thread-safe metric collection with minimal overhead
- Channel-based architecture decouples scanning from output operations
- Multiple path searches are parallelized across thread pool

## Platform-Specific Behavior

### Windows

When no custom path is specified, searches all available drive letters (C: through Z:) that exist on the system.

### Unix-like Systems (Linux, macOS)

When no custom path is specified, searches from the root directory (/).

### Custom Paths

When using `-p` or `--path`, the specified paths are validated before search begins. Non-existent paths will cause an error and exit.

## Dependencies

- `clap`: Command-line argument parsing with version support
- `rayon`: Data parallelism library for thread pool management
- `crossbeam`: Concurrent programming primitives and channels
- `walkdir`: Recursive directory traversal with symlink control
- Standard Rust library for atomics, threading, and I/O

## Technical Details

### Thread Safety

All shared state uses atomic operations for lock-free synchronization. The metrics collection system employs `AtomicU64` and `AtomicBool` types with relaxed ordering for optimal performance.

### Resource Management

The application properly manages system resources through:

- Scoped thread pools with controlled lifecycle
- Unbounded channels for non-blocking result transmission
- Graceful shutdown mechanism triggered by limit conditions
- Proper cleanup of file handles and directory iterators

### Search Algorithm

1. Root paths are determined based on custom paths or operating system defaults
2. Path validation ensures all specified paths exist before search begins
3. Directory traversal begins in parallel across all roots using thread pool
4. Each entry is checked against the pattern matcher and type filters
5. Matching results are sent through channels to the printer thread
6. Progress is reported asynchronously on a separate thread
7. Search terminates when limits are reached or all paths are exhausted
8. Final metrics including errors are collected and reported

### Size Formatting

File sizes are automatically formatted using appropriate units:

- Bytes (B) for sizes under 1 KB
- Kilobytes (KB), Megabytes (MB), Gigabytes (GB), Terabytes (TB) as appropriate
- Two decimal precision for formatted sizes

## Error Handling

The application handles common file system errors gracefully:

- Inaccessible directories are skipped and counted as errors
- Permission errors do not halt the search
- Invalid symbolic links are ignored unless `--follow-links` is enabled
- Failed path conversions are filtered out
- Non-existent custom paths trigger immediate error and exit
- Conflicting options (files-only + dirs-only) are validated at startup

## Exit Codes

- `0`: Successful execution
- `1`: Error occurred (invalid path, conflicting options, no valid search paths)

## Use Cases

### System Administration

Find configuration files across the system:

```bash
omega -p /etc -p /usr/local/etc config
```

### Development

Locate source files in project directories:

```bash
omega -f -p ./src -p ./lib -i .rs
```

### Log Analysis

Find recent log files with size information:

```bash
omega -v -p /var/log log
```

### Cleanup Operations

Identify large directories for cleanup:

```bash
omega -D -v cache temp
```

## Performance Benchmarks

Omega is designed for speed. Typical performance characteristics:

- Scans hundreds of thousands of files per second on modern hardware
- Scales linearly with CPU core count
- Minimal memory footprint through streaming architecture
- Efficient pattern matching with early termination

## Contributing

Contributions are welcome. Please ensure code follows Rust best practices and includes appropriate documentation.

### Guidelines

- Follow existing code structure and naming conventions
- Add tests for new features
- Update documentation for user-facing changes
- Ensure all tests pass before submitting
- Use `cargo fmt` and `cargo clippy` for code quality