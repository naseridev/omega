use std::borrow::Cow;
use std::fs::Metadata;
use std::io::Write;
use std::path::Path;

const SECONDS_PER_DAY: u64 = 86_400;
const SIZE_UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

#[cfg(unix)]
pub fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
pub fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    match path.to_string_lossy() {
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
    }
}

pub fn write_csv_field(out: &mut Vec<u8>, field: &[u8]) {
    let quoted = field
        .iter()
        .any(|b| matches!(b, b',' | b'"' | b'\n' | b'\r' | b'\t'));

    if !quoted {
        out.extend_from_slice(field);
        return;
    }

    out.push(b'"');
    for &byte in field {
        if byte == b'"' {
            out.push(b'"');
        }
        out.push(byte);
    }
    out.push(b'"');
}

pub fn write_size(out: &mut Vec<u8>, bytes: u64) {
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < SIZE_UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    let _ = if unit == 0 {
        write!(out, "{} {}", bytes, SIZE_UNITS[0])
    } else {
        write!(out, "{:.2} {}", size, SIZE_UNITS[unit])
    };
}

pub fn write_timestamp(out: &mut Vec<u8>, timestamp: u64) {
    let remaining = timestamp % SECONDS_PER_DAY;
    let (year, month, day) = civil_from_days(timestamp / SECONDS_PER_DAY);

    let _ = write!(
        out,
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        remaining / 3600,
        (remaining % 3600) / 60,
        remaining % 60
    );
}

fn civil_from_days(days_since_epoch: u64) -> (u64, u64, u64) {
    let shifted = days_since_epoch + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    };

    (if month <= 2 { year + 1 } else { year }, month, day)
}

pub fn modified_seconds(metadata: &Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(unix)]
pub fn write_permissions(out: &mut Vec<u8>, metadata: &Metadata) {
    use std::os::unix::fs::PermissionsExt;

    const FLAGS: [(u32, u8); 9] = [
        (0o400, b'r'),
        (0o200, b'w'),
        (0o100, b'x'),
        (0o040, b'r'),
        (0o020, b'w'),
        (0o010, b'x'),
        (0o004, b'r'),
        (0o002, b'w'),
        (0o001, b'x'),
    ];

    let mode = metadata.permissions().mode();
    let mut rendered = [b'-'; 9];

    for (slot, (bit, symbol)) in rendered.iter_mut().zip(FLAGS) {
        if mode & bit != 0 {
            *slot = symbol;
        }
    }

    out.extend_from_slice(&rendered);
}

#[cfg(windows)]
pub fn write_permissions(out: &mut Vec<u8>, metadata: &Metadata) {
    let rendered: &[u8] = if metadata.permissions().readonly() {
        b"r--r--r--"
    } else {
        b"rw-rw-rw-"
    };
    out.extend_from_slice(rendered);
}

#[cfg(not(any(unix, windows)))]
pub fn write_permissions(out: &mut Vec<u8>, _metadata: &Metadata) {
    out.extend_from_slice(b"rwxrwxrwx");
}

#[cfg(windows)]
pub fn is_hidden(_name: &str, metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
pub fn is_hidden(name: &str, _metadata: &Metadata) -> bool {
    name.as_bytes().first() == Some(&b'.')
}
