# Omega File Search

A high-performance, cross-platform file search utility built with Rust. Omega leverages parallel processing and efficient directory traversal to provide rapid file system searches across multiple platforms.

## Overview

Omega is designed to search through file systems at scale, utilizing multi-threaded scanning and pattern matching capabilities. The application supports various search configurations including depth limits, result limits, and case-sensitive matching.

## Architecture

The project follows a modular architecture with clear separation of concerns:

- **Pattern Matching**: Handles search pattern processing and matching logic
- **File System Scanner**: Manages directory traversal and file discovery
- **Metrics Collection**: Tracks search progress and statistics
- **Result Management**: Handles output formatting and result delivery
- **Progress Reporting**: Provides real-time feedback during search operations

## Features

- Multi-threaded parallel file system scanning
- Cross-platform support (Windows, Linux, macOS)
- Case-sensitive and case-insensitive search modes
- Configurable search depth limitation
- Result count and scan count limits
- Real-time progress reporting
- Quiet mode for minimal output
- Automatic thread pool optimization

## Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo package manager

### Build from Source

```bash
git clone <repository-url>
cd omega
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

- `-i, --case-sensitive`: Enable case-sensitive search (default: case-insensitive)
- `-l, --limit-found <COUNT>`: Limit the number of results found
- `-s, --limit-scanned <COUNT>`: Limit the number of items scanned
- `-t, --threads <COUNT>`: Specify number of threads (default: auto-detected)
- `-d, --max-depth <DEPTH>`: Maximum search depth in directory tree
- `-q, --quiet`: Quiet mode - only print file paths without markers or progress

### Examples

Search for files containing "config" in their name:

```bash
omega config
```

Case-sensitive search with result limit:

```bash
omega -i Config -l 100
```

Search with maximum depth of 3 levels:

```bash
omega document -d 3
```

Quiet mode with custom thread count:

```bash
omega -q -t 8 report
```

Multiple patterns with scan limit:

```bash
omega readme license -s 10000
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

## Performance Considerations

- Thread count is automatically optimized based on available CPU cores
- The application uses Rayon for work-stealing parallelism
- WalkDir provides efficient directory traversal with minimal allocations
- Atomic operations ensure thread-safe metric collection with minimal overhead
- Channel-based architecture decouples scanning from output operations

## Platform-Specific Behavior

### Windows

Searches all available drive letters (C: through Z:) that exist on the system.

### Unix-like Systems (Linux, macOS)

Searches from the root directory (/).

## Dependencies

- `clap`: Command-line argument parsing
- `rayon`: Data parallelism library
- `crossbeam`: Concurrent programming primitives
- `walkdir`: Recursive directory traversal
- Standard Rust library for atomics and threading

## Technical Details

### Thread Safety

All shared state uses atomic operations for lock-free synchronization. The metrics collection system employs `AtomicU64` and `AtomicBool` types with relaxed ordering for optimal performance.

### Resource Management

The application properly manages system resources through:

- Scoped thread pools with controlled lifecycle
- Unbounded channels for non-blocking result transmission
- Graceful shutdown mechanism triggered by limit conditions

### Search Algorithm

1. Root paths are determined based on the operating system
2. Directory traversal begins in parallel across all roots
3. Each entry is checked against the pattern matcher
4. Matching results are sent through channels to the printer thread
5. Progress is reported asynchronously on a separate thread
6. Search terminates when limits are reached or all paths are exhausted

## Error Handling

The application handles common file system errors gracefully:

- Inaccessible directories are skipped
- Permission errors do not halt the search
- Invalid symbolic links are ignored
- Failed path conversions are filtered out

## Contributing

Contributions are welcome. Please ensure code follows Rust best practices and includes appropriate documentation.

## Changelog

### Version 1.0.0

- Initial release with core functionality
- Multi-threaded search implementation
- Cross-platform support
- Configurable limits and options
- Quiet mode support