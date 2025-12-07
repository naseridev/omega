# Omega

A fast cross-platform file search utility written in rust.

## Overview

Omega is a high-performance command-line utility designed for searching files and directories across your filesystem. It offers parallel processing, fuzzy matching, content search capabilities, and flexible filtering options.

## Installation

### From Source

```bash
git clone https://github.com/naseridev/omega.git
```
```
cd omega
```
```
cargo build --release
```

The compiled binary will be available at `target/release/omega`.

### Adding to PATH

#### Linux/macOS
```bash
sudo cp target/release/omega /usr/local/bin/
```

#### Windows
Add the directory containing `omega.exe` to your system PATH environment variable.

## Basic Usage

Search for files and directories matching a pattern:

```bash
omega pattern
```

Search in a specific directory:

```bash
omega pattern -p /path/to/directory
```

Search with multiple patterns:

```bash
omega pattern1 pattern2 pattern3
```

## Command-Line Options

### Search Patterns

```bash
omega pattern [patterns...]
```

One or more search patterns are required. Omega will match any file or directory containing any of the specified patterns.

### Path Options

`-p, --path <PATH>`

Specify one or more directories to search. Can be used multiple times. If not specified, Omega searches all available drives on Windows or root directory on Unix systems.

```bash
omega pattern -p /home/user -p /var/log
```

### Search Limits

`-l, --limit-found <NUMBER>`

Stop searching after finding the specified number of matches.

```bash
omega pattern -l 100
```

`-s, --limit-scanned <NUMBER>`

Stop searching after scanning the specified number of items.

```bash
omega pattern -s 10000
```

### Performance Options

`-t, --threads <NUMBER>`

Set the number of threads to use for parallel processing. Auto-detected if not specified.

```bash
omega pattern -t 8
```

`-d, --max-depth <NUMBER>`

Limit the search depth in directory tree.

```bash
omega pattern -d 3
```

### Search Modes

`-i, --case-insensitive`

Perform case-insensitive pattern matching.

```bash
omega Pattern -i
```

`-f, --files-only`

Search only files, exclude directories.

```bash
omega pattern -f
```

`-D, --dirs-only`

Search only directories, exclude files.

```bash
omega pattern -D
```

### Fuzzy Search

`-z, --fuzzy`

Enable fuzzy matching using Levenshtein distance algorithm.

```bash
omega patern -z
```

`-T, --fuzzy-threshold <NUMBER>`

Set the fuzzy matching distance threshold (default: 2).

```bash
omega patern -z -T 3
```

### Content Search

`-c, --content-search`

Search inside file contents in addition to filenames.

```bash
omega "function main" -c
```

`--max-content-size <BYTES>`

Maximum file size for content search in bytes (default: 10485760 = 10MB).

```bash
omega pattern -c --max-content-size 5242880
```

### Output Options

`--api`

Output results in CSV format for programmatic use.

```bash
omega pattern --api > results.csv
```

CSV columns: path, name, is_dir, is_file, size, size_human, modified, modified_human, is_hidden, extension, permissions

`-e, --hide-errors`

Suppress error messages in the summary.

```bash
omega pattern -e
```

## Usage Examples

### Basic Searches

Search for all files containing "config":

```bash
omega config
```

Search for multiple patterns:

```bash
omega .txt .md .rst
```

Case-insensitive search:

```bash
omega readme -i
```

### Filtered Searches

Find only directories named "src":

```bash
omega src -D
```

Find only files with "test" in name:

```bash
omega test -f
```

Search up to 3 levels deep:

```bash
omega pattern -d 3
```

### Content Search

Search for files containing specific text:

```bash
omega "TODO" -c
```

Search content in small files only:

```bash
omega "import" -c --max-content-size 1048576
```

### Fuzzy Search

Find files with approximate matches:

```bash
omega dokument -z
```

Adjust fuzzy matching sensitivity:

```bash
omega dokument -z -T 1
```

### Performance Tuning

Limit results for faster searches:

```bash
omega pattern -l 50
```

Use specific number of threads:

```bash
omega pattern -t 16
```

### Programmatic Output

Export results to CSV:

```bash
omega pattern --api > results.csv
```

Filter CSV results with other tools:

```bash
omega pattern --api | grep ".rs"
```

## Output Format

### Normal Mode

Prints the full path of each matching file or directory:

```
/home/user/documents/file.txt
/home/user/projects/src/main.rs
```

### API Mode

Outputs CSV format with detailed file information:

```csv
path,name,is_dir,is_file,size,size_human,modified,modified_human,is_hidden,extension,permissions
/home/user/file.txt,file.txt,false,true,1024,1.00 KB,1701936000,2023-12-07T12:00:00Z,false,txt,rw-r--r--
```

### Summary

After search completion, a summary is displayed:

```
42 found | 1523 scanned
3 errors occurred
```

## Error Handling

Errors are logged to `omega.log` in the current directory. Common errors include:

- Permission denied when accessing protected directories
- Symbolic link loops
- Invalid file metadata

Use `-e` flag to hide error count in summary.

## Performance Considerations

- Searching entire filesystems can be slow on the first run due to cold cache
- Use `-p` to limit search scope for better performance
- Adjust thread count with `-t` based on your CPU cores
- Use `-l` or `-s` to stop early when you have enough results
- Content search is slower; use size limits with `--max-content-size`

## Platform Support

Omega is cross-platform and supports:

- Linux
- macOS
- Windows

Platform-specific features:

- Hidden file detection uses dot-prefix on Unix, file attributes on Windows
- Permissions are formatted as Unix-style on Unix systems, simplified on Windows
- Default search paths are all drives (C:\ to Z:\) on Windows, root (/) on Unix

## Exit Codes

- 0: Success
- 1: Error (invalid arguments, no search paths, fatal search error)

## For More Information

For detailed technical documentation, architecture details, and development guide, see the [Wiki](README.wiki.md).

## License

This project is open source. See LICENSE file for details.

## Author

Nima Naseri