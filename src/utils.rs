use std::path::Path;

pub fn escape_csv(s: &str) -> String {
    if s.contains(',')
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
    {
        format!("\"{}\"", s.replace("\"", "\"\""))
    } else {
        s.to_string()
    }
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

pub fn format_timestamp(timestamp: u64) -> String {
    const SECONDS_PER_DAY: u64 = 86400;

    let days = timestamp / SECONDS_PER_DAY;
    let remaining = timestamp % SECONDS_PER_DAY;

    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    let (year, month, day) = days_to_date(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_date(days_since_epoch: u64) -> (u32, u32, u32) {
    let mut year = 1970u32;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };

        if remaining_days < days_in_year as u64 {
            break;
        }

        remaining_days -= days_in_year as u64;
        year += 1;
    }

    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &days_in_month in &days_in_months {
        if remaining_days < days_in_month as u64 {
            break;
        }
        remaining_days -= days_in_month as u64;
        month += 1;
    }

    let day = (remaining_days + 1) as u32;

    (year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn is_hidden_file(path: &Path, _name: &str) -> bool {
    #[cfg(unix)]
    {
        _name.starts_with('.')
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0
        } else {
            false
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        _name.starts_with('.')
    }
}

#[cfg(unix)]
pub fn format_permissions(metadata: &std::fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode();

    format!(
        "{}{}{}{}{}{}{}{}{}",
        if mode & 0o400 != 0 { "r" } else { "-" },
        if mode & 0o200 != 0 { "w" } else { "-" },
        if mode & 0o100 != 0 { "x" } else { "-" },
        if mode & 0o040 != 0 { "r" } else { "-" },
        if mode & 0o020 != 0 { "w" } else { "-" },
        if mode & 0o010 != 0 { "x" } else { "-" },
        if mode & 0o004 != 0 { "r" } else { "-" },
        if mode & 0o002 != 0 { "w" } else { "-" },
        if mode & 0o001 != 0 { "x" } else { "-" }
    )
}

#[cfg(windows)]
pub fn format_permissions(metadata: &std::fs::Metadata) -> String {
    let readonly = metadata.permissions().readonly();
    if readonly {
        "r--r--r--".to_string()
    } else {
        "rw-rw-rw-".to_string()
    }
}

#[cfg(not(any(unix, windows)))]
pub fn format_permissions(_metadata: &std::fs::Metadata) -> String {
    "rwxrwxrwx".to_string()
}
