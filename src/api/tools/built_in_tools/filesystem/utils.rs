use std::env;
use std::fs;
use std::path::{
    Component,
    Path,
    PathBuf,
    Prefix,
};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use chrono::{DateTime, Local};
use glob::Pattern;
use walkdir::WalkDir;

use crate::{
    error::MyError,
    parse_paras::PARAS,
};

/// convert requested_path to absolute path, normalize it, check exist
pub fn check_path(requested_path: &Path, check_exist: bool) -> Result<PathBuf, MyError> {
    let absolute_path = if requested_path.is_relative() { // if is relative path, add current dir as preffix
        match env::current_dir() {
            Ok(c) => c.join(requested_path),
            Err(e) => return Err(MyError::OtherError{info: format!("get current dir: {}", e)}),
        }
    } else {
        requested_path.to_path_buf()
    };
    //if check_exist && !(absolute_path.exists() && absolute_path.is_dir()) {
    if check_exist && !absolute_path.exists() {
        return Err(MyError::DirNotExistError{dir: format!("{} (absolute path: {})", requested_path.display(), absolute_path.display())})
    }
    Ok(absolute_path)
}

/// returns the canonical, absolute form of the path with all intermediate components normalized and symbolic links resolved
pub fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

// checks if path component is a  Prefix::VerbatimDisk
fn is_verbatim_disk(component: &Component) -> bool {
    match component {
        Component::Prefix(prefix_comp) => matches!(prefix_comp.kind(), Prefix::VerbatimDisk(_)),
        _ => false,
    }
}

/// Check path contains a symlink
pub fn contains_symlink<P: AsRef<Path>>(path: P) -> Result<bool, MyError> {
    let mut current_path = PathBuf::new();

    for component in path.as_ref().components() {
        current_path.push(component);

        // no need to check symlink_metadata for Prefix::VerbatimDisk
        if is_verbatim_disk(&component) {
            continue;
        }

        if !current_path.exists() {
            break;
        }

        if fs::symlink_metadata(&current_path)?.file_type().is_symlink() {
            return Ok(true)
        }
    }

    Ok(false)
}

/// convert requested_path to absolute path, normalize it, check exist, then check allowed_path contain requested_path
pub fn validate_path(allowed_path: &Vec<(PathBuf, PathBuf)>, requested_path: &Path, check_exist: bool) -> Result<PathBuf, MyError> {
    let absolute_path = check_path(requested_path, check_exist)?;
    let normalized_requested = normalize_path(&absolute_path);

    // Check if path is within allowed directories
    // Must account for both scenarios — the requested path may not exist yet, making canonicalization impossible.
    if !allowed_path.iter().any(|(abs_dir, norm_dir)| normalized_requested.starts_with(abs_dir) || normalized_requested.starts_with(norm_dir)) {
        let symlink_target = if contains_symlink(&absolute_path)? {
            "a symlink target path"
        } else {
            "path"
        };
        return Err(MyError::OtherError{info: format!(
            "access denied, {} is outside allowed directories: {} not in {}",
            symlink_target,
            normalized_requested.display(),
            allowed_path
                .iter()
                .map(|p| p.0.display().to_string())
                .collect::<Vec<_>>()
                .join(",\n"),
        )})
    }

    Ok(absolute_path)
}

pub fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}


fn format_system_time(system_time: SystemTime) -> String {
    // Convert SystemTime to DateTime<Local>
    let datetime: DateTime<Local> = system_time.into();
    datetime.format("%a %b %d %Y %H:%M:%S %:z").to_string()
}

fn format_permissions(metadata: &fs::Metadata) -> String {
    #[cfg(unix)]
    {
        let permissions = metadata.permissions();
        let mode = permissions.mode();
        format!("0{:o}", mode & 0o777) // Octal representation
    }

    #[cfg(windows)]
    {
        let attributes = metadata.file_attributes();
        let read_only = (attributes & 0x1) != 0; // FILE_ATTRIBUTE_READONLY
        let directory = metadata.is_dir();

        let mut result = String::new();

        if directory {
            result.push('d');
        } else {
            result.push('-');
        }

        if read_only {
            result.push('r');
        } else {
            result.push('w');
        }

        result
    }
}

#[derive(Debug)]
pub struct FileInfo {
    pub size: u64,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub is_directory: bool,
    pub is_file: bool,
    pub metadata: fs::Metadata,
}

impl std::fmt::Display for FileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"size: {}
created: {}
modified: {}
accessed: {}
isDirectory: {}
isFile: {}
permissions: {}
"#,
            self.size,
            self.created.map_or("".to_string(), format_system_time),
            self.modified.map_or("".to_string(), format_system_time),
            self.accessed.map_or("".to_string(), format_system_time),
            self.is_directory,
            self.is_file,
            format_permissions(&self.metadata)
        )
    }
}

pub fn list_directory_helper(dir_path: &str) -> Result<Vec<fs::DirEntry>, MyError> {
    let valid_path = validate_path(&PARAS.allowed_path, Path::new(dir_path), true)?;
    let mut dir = fs::read_dir(valid_path)?;
    let mut entries: Vec<fs::DirEntry> = Vec::new();
    // Use a loop to collect the directory entries
    while let Some(entry) = dir.next() {
        entries.push(entry?);
    }
    Ok(entries)
}

pub fn read_file_helper(file_path: &str) -> Result<String, MyError> {
    let valid_path = validate_path(&PARAS.allowed_path, Path::new(file_path), true)?;
    let content = fs::read_to_string(valid_path)?;
    Ok(content)
}

/// Searches for files in the directory tree starting at `root_path` that match the given `include_pattern`,
/// excluding paths that match any of the `exclude_patterns`.
pub fn search_files_helper(root_path: &str, include_pattern: String, exclude_patterns: Option<Vec<String>>) -> Result<Vec<walkdir::DirEntry>, MyError> {
    let root_path = Path::new(root_path);
    let valid_path = validate_path(&PARAS.allowed_path, root_path, true)?;

    let updated_pattern = if include_pattern.contains('*') {
        include_pattern.to_lowercase()
    } else {
        format!("**/*{}*", include_pattern.to_lowercase())
    };
    let glob_pattern = Pattern::new(&updated_pattern);

    let exclude_patterns = exclude_patterns.unwrap_or_default();
    let result = WalkDir::new(valid_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(move |dir_entry| {
            let full_path = dir_entry.path();

            // Validate each path before processing
            let validated_path = validate_path(&PARAS.allowed_path, full_path, true).ok();

            if validated_path.is_none() {
                // Skip invalid paths during search
                return false;
            }

            // Get the relative path from the root_path
            let relative_path = full_path.strip_prefix(root_path).unwrap_or(full_path);

            let should_exclude = exclude_patterns.iter().any(|pattern| {
                let glob_pattern = if pattern.contains('*') {
                    pattern.clone()
                } else {
                    format!("*{pattern}*")
                };

                Pattern::new(&glob_pattern)
                    .map(|glob| glob.matches(relative_path.to_str().unwrap_or("")))
                    .unwrap_or(false)
            });

            !should_exclude
        })
        .filter_map(|v| v.ok())
        .filter(move |entry| {
            if root_path == entry.path() {
                return false;
            }

            let is_match = glob_pattern
                .as_ref()
                .map(|glob| glob.matches(&entry.file_name().to_str().unwrap_or("").to_lowercase()))
                .unwrap_or(false);
            is_match
        }).collect::<Vec<walkdir::DirEntry>>();

    Ok(result)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    let units = [(TB, "TB"), (GB, "GB"), (MB, "MB"), (KB, "KB")];

    for (threshold, unit) in units {
        if bytes >= threshold {
            return format!("{:.2} {}", bytes as f64 / threshold as f64, unit);
        }
    }
    format!("{bytes} bytes")
}
