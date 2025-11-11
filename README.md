# Omega File Search

A fast cross-platform file search utility written in rust. Omega leverages parallel processing and efficient directory traversal to provide rapid file system searches with advanced filtering capabilities.

## Overview

Omega is designed to search through file systems at scale, utilizing multi-threaded scanning and pattern matching capabilities. The application supports various search configurations including custom paths, depth limits, result limits, fuzzy search, and multiple output modes including API-compatible CSV format.

## Architecture

The project follows a modular architecture with clear separation of concerns:

- **Pattern Matching**: Handles search pattern processing with support for exact and fuzzy matching using Levenshtein distance
- **File System Scanner**: Manages directory traversal and file discovery
- **Metrics Collection**: Tracks search progress, statistics, and errors
- **Result Management**: Handles output formatting and result delivery in multiple formats
- **File Information**: Comprehensive metadata extraction including permissions, timestamps, and file attributes
- **Path Provider**: Manages search root configuration for system-wide or targeted searches

## Features

- Multi-threaded parallel file system scanning
- Cross-platform support (Windows, Linux, macOS)
- Custom path targeting with multiple path support
- Case-sensitive and case-insensitive search modes
- **Fuzzy search** with configurable Levenshtein distance threshold
- File-only or directory-only filtering
- Configurable search depth limitation
- Result count and scan count limits
- Multiple output modes: normal and API (CSV format)
- **Comprehensive file metadata** including:
  - File size (bytes and human-readable)
  - Modification timestamps (Unix timestamp and ISO 8601 format)
  - File permissions (Unix-style format)
  - File extension
  - Hidden file detection
  - File type (file/directory)
- Error tracking with optional error display
- Automatic thread pool optimization
- Platform-specific hidden file detection

## Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo package manager

### Build from Source

```bash
git clone https://github.com/naseridev/omega.git
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
- `-z, --fuzzy`: Enable fuzzy search using Levenshtein distance algorithm
- `-T, --fuzzy-threshold <NUM>`: Set fuzzy search distance threshold (default: 2)

#### Filtering

- `-f, --files-only`: Search only files
- `-D, --dirs-only`: Search only directories

#### Limits

- `-l, --limit-found <COUNT>`: Limit the number of results found
- `-s, --limit-scanned <COUNT>`: Limit the number of items scanned

#### Performance

- `-t, --threads <COUNT>`: Specify number of threads (default: auto-detected)

#### Output

- `--api`: API mode - outputs results in CSV format with comprehensive metadata
- `-e, --hide-errors`: Hide error messages during search

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

#### Fuzzy Search

Find files with names similar to "readme" (tolerates typos):

```bash
omega -z readme
```

Find files with custom fuzzy threshold:

```bash
omega -z -T 3 dokument
```

This will match "document", "documents", "dokument", etc.

#### Combined Short Options

Case-insensitive search with custom path:

```bash
omega -ip /home document
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

#### API Mode (CSV Output)

Get structured data with all metadata:

```bash
omega --api config
```

Output includes: path, name, is_dir, is_file, size, size_human, modified, modified_human, is_hidden, extension, permissions

#### Error Handling

Show all errors encountered during search:

```bash
omega config
```

Hide errors for cleaner output:

```bash
omega -e document
```

#### Multiple Patterns

Search for multiple patterns simultaneously:

```bash
omega readme license changelog
```

Fuzzy search with multiple patterns:

```bash
omega -z readme lisence
```

## Output Format

### Normal Mode

Results are displayed as simple file paths:

```
/path/to/config.txt
/path/to/configuration
/home/user/app.config
```

Final summary on stderr:

```
15 found | 1,234 scanned
```

If errors occurred and not hidden:

```
15 found | 1,234 scanned
3 errors occurred
```

### API Mode (CSV Format)

Structured output with comprehensive metadata:

```csv
path,name,is_dir,is_file,size,size_human,modified,modified_human,is_hidden,extension,permissions
/etc/config.txt,config.txt,false,true,1024,1.00 KB,1699564800,2023-11-09T20:00:00Z,false,txt,rw-r--r--
/home/user/.config,.config,true,false,0,0 B,1699651200,2023-11-10T20:00:00Z,true,,rwxr-xr-x
```

Field descriptions:
- `path`: Full path to the file/directory
- `name`: File/directory name
- `is_dir`: Boolean indicating if entry is a directory
- `is_file`: Boolean indicating if entry is a file
- `size`: Size in bytes (0 for directories)
- `size_human`: Human-readable size (KB, MB, GB, TB)
- `modified`: Unix timestamp of last modification
- `modified_human`: ISO 8601 formatted timestamp
- `is_hidden`: Boolean indicating if file/directory is hidden
- `extension`: File extension (empty for directories)
- `permissions`: Unix-style permissions (rwxrwxrwx format)

No summary information is displayed in API mode (clean CSV output only).

## Fuzzy Search Algorithm

Omega uses the Levenshtein distance algorithm for fuzzy matching. This allows finding files even with typos or spelling variations.

### How It Works

- The algorithm calculates the minimum number of single-character edits needed to transform one string into another
- Edits include: insertions, deletions, or substitutions
- The fuzzy threshold (`-T`) specifies the maximum allowed distance

### Examples

With threshold 2 (`-T 2`), the pattern "dokument" matches:
- "document" (distance: 1)
- "documents" (distance: 2)
- "dokument" (exact match)

But not:
- "documentation" (distance: 5)

### Best Practices

- Use lower thresholds (1-2) for precise matching
- Use higher thresholds (3-4) for very flexible matching
- Combine with case-insensitive mode (`-i`) for better results
- Fuzzy search also performs exact substring matching for performance

## File Metadata

### Timestamps

Modification times are provided in two formats:
- **Unix timestamp**: Seconds since January 1, 1970 UTC
- **ISO 8601 format**: `YYYY-MM-DDTHH:MM:SSZ` (human-readable)

### Permissions

Unix-style permission format (9 characters):
- **rwx**: User permissions (read, write, execute)
- **rwx**: Group permissions
- **rwx**: Other permissions

Examples:
- `rwxr-xr-x`: Owner can read/write/execute, others can read/execute
- `rw-r--r--`: Owner can read/write, others can only read
- `rwxrwxrwx`: Full permissions for everyone

On Windows, permissions are approximated based on readonly attribute.

### Hidden Files

Platform-specific detection:
- **Unix/Linux/macOS**: Files starting with `.`
- **Windows**: Files with the hidden attribute flag
- **Other platforms**: Files starting with `.` (fallback)

### Size Formatting

File sizes are automatically formatted using appropriate units:
- **Bytes (B)**: For sizes under 1 KB
- **Kilobytes (KB)**, **Megabytes (MB)**, **Gigabytes (GB)**, **Terabytes (TB)**: As appropriate
- Two decimal precision for formatted sizes (e.g., "1.25 MB")

## Performance Considerations

- Thread count is automatically optimized based on available CPU cores
- The application uses Rayon for work-stealing parallelism
- WalkDir provides efficient directory traversal with minimal allocations
- Atomic operations ensure thread-safe metric collection with minimal overhead
- Channel-based architecture decouples scanning from output operations
- Multiple path searches are parallelized across thread pool
- Fuzzy search includes exact substring matching optimization for better performance

## Platform-Specific Behavior

### Windows

- When no custom path is specified, searches all available drive letters (C: through Z:)
- Permissions shown as simplified read-write format
- Hidden files detected via file attributes

### Unix-like Systems (Linux, macOS)

- When no custom path is specified, searches from the root directory (/)
- Full Unix permission support (user/group/other)
- Hidden files detected by name (starting with `.`)

### Custom Paths

When using `-p` or `--path`, the specified paths are validated before search begins. Non-existent paths will cause an error and exit.

## Dependencies

- `clap`: Command-line argument parsing with version support
- `rayon`: Data parallelism library for thread pool management
- `crossbeam`: Concurrent programming primitives and channels
- `walkdir`: Recursive directory traversal
- Standard Rust library for atomics, threading, I/O, and file system operations

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
4. Each entry is checked against the pattern matcher (exact or fuzzy)
5. Entries are filtered based on type filters (files-only/dirs-only)
6. Matching results have full metadata extracted and are sent through channels
7. Search terminates when limits are reached or all paths are exhausted
8. Final metrics including errors are collected and reported

### Fuzzy Matching Algorithm

The Levenshtein distance implementation uses dynamic programming:
- Time complexity: O(m × n) where m and n are pattern and target lengths
- Space complexity: O(n) using row-wise optimization
- Exact matches are checked first for performance
- Word-based matching splits targets on non-alphanumeric characters

### CSV Output Format

API mode produces RFC 4180 compliant CSV:
- Fields containing commas, quotes, newlines, or tabs are quoted
- Internal quotes are escaped by doubling (`""`)
- UTF-8 encoding for all text data
- Boolean values output as `true`/`false`

## Error Handling

The application handles common file system errors gracefully:

- Inaccessible directories are skipped and counted as errors
- Permission errors do not halt the search
- Invalid paths are filtered out
- Non-existent custom paths trigger immediate error and exit
- Conflicting options (files-only + dirs-only) are validated at startup
- Metadata extraction failures are handled per-file

## Exit Codes

- `0`: Successful execution
- `1`: Error occurred (invalid path, conflicting options, no valid search paths, thread pool creation failure)

## Use Cases

### System Administration

Find configuration files across the system:

```bash
omega -p /etc -p /usr/local/etc config
```

Get detailed metadata about log files:

```bash
omega --api -p /var/log log
```

### Development

Locate source files in project directories:

```bash
omega -f -p ./src -p ./lib -i .rs
```

Find files with fuzzy name matching:

```bash
omega -z -p ./project dokument
```

### Data Analysis

Export file system metadata to CSV for analysis:

```bash
omega --api -p /data backup > backup_files.csv
```

### File Management

Find large files for cleanup:

```bash
omega --api -f -p /home cache | sort -t, -k5 -n
```

Find hidden configuration files:

```bash
omega -i -p ~ .config
```

### Content Search

Case-insensitive fuzzy search across multiple directories:

```bash
omega -iz -p /documents -p /downloads report
```

## Performance Benchmarks

Omega is designed for speed. Typical performance characteristics:

- Scans hundreds of thousands of files per second on modern hardware
- Scales linearly with CPU core count
- Minimal memory footprint through streaming architecture
- Efficient pattern matching with early termination
- Fuzzy search optimized with exact substring checking

## Contributing

Contributions are welcome. Please ensure code follows Rust best practices and includes appropriate documentation.

### Guidelines

- Follow existing code structure and naming conventions
- Add tests for new features
- Update documentation for user-facing changes
- Ensure all tests pass before submitting
- Use `cargo fmt` and `cargo clippy` for code quality
- Consider performance implications of changes

## Author

Nima Naseri

## Version

Current version: Check `Cargo.toml` or run `omega --version`
