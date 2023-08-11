use std::collections::HashMap;
use std::collections::BTreeMap;
use anyhow::{anyhow,Result};
use chrono::{Utc,Local};
use timeago::Formatter;
use std::path::{Path,PathBuf};
use std::fs::{File};
use std::io::Read;
use md5::{Context};
#[allow(unused_imports)]
use log::{info, trace, debug};
use colored::*;
use unicode_width::UnicodeWidthStr;

use crate::data::{StatusEntry,LocalStatusCode};
use crate::remote::RemoteStatusCode;
use super::remote::{Remote};


pub fn load_file(path: &PathBuf) -> String {
    let mut file = File::open(&path).expect("unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("unable to read file");
    contents
}

pub fn ensure_directory(dir: &Path) -> Result<()> {
    let path = Path::new(dir);
    if path.is_dir() {
        Ok(())
    } else {
        Err(anyhow!("'{}' is not a directory or doesn't exist.", dir.to_string_lossy()))
    }
}

pub fn ensure_exists(path: &Path) -> Result<()> {
    if path.exists() {
        Ok(())
    } else {
        Err(anyhow!("Path does not exist: {:?}", path))
    }
}

/// Compute the MD5 of a file returning None if the file is empty.
pub fn compute_md5(file_path: &Path) -> Result<Option<String>> {
    const BUFFER_SIZE: usize = 1024;

    let mut file = match File::open(file_path) {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };

    let mut buffer = [0; BUFFER_SIZE];
    let mut md5 = Context::new();

    loop {
        let bytes_read = match file.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(bytes_read) => bytes_read,
            Err(e) => return Err(anyhow!("I/O reading file: {:?}", e)),
        };

        md5.consume(&buffer[..bytes_read]);
    }
    
    let result = md5.compute();
    Ok(Some(format!("{:x}", result)))
}

pub fn print_fixed_width(rows: HashMap<String, Vec<StatusEntry>>, nspaces: Option<usize>, indent: Option<usize>, color: bool) {
    let indent = indent.unwrap_or(0);
    let nspaces = nspaces.unwrap_or(6);

    let max_cols = rows.values()
        .flat_map(|v| v.iter())
        .filter_map(|entry| {
            match &entry.cols {
                None => None,
                Some(cols) => Some(cols.len())
            }
        })
        .max()
        .unwrap_or(0);

    let mut max_lengths = vec![0; max_cols];

    // compute max lengths across all rows
    for entry in rows.values().flat_map(|v| v.iter()) {
        if let Some(cols) = &entry.cols {
            for (i, col) in cols.iter().enumerate() {
                max_lengths[i] = max_lengths[i].max(col.width());
            }
        }
    }
    // print status table
    let mut keys: Vec<&String> = rows.keys().collect();
    keys.sort();
    for (key, value) in &rows {
        let pretty_key = if color { key.bold().to_string() } else { key.clone() };
        println!("[{}]", pretty_key);

        // Print the rows with the correct widths
        for row in value {
            let mut fixed_row = Vec::new();
            let tracked = &row.tracked;
            let local_status = &row.local_status;
            let remote_status = &row.remote_status;
            if let Some(cols) = &row.cols {
                for (i, col) in cols.iter().enumerate() {
                // push a fixed-width column to vector
                    let fixed_col = format!("{:width$}", col, width = max_lengths[i]);
                    fixed_row.push(fixed_col);
                }
            }
            let spacer = " ".repeat(nspaces);
            let status_line = fixed_row.join(&spacer);
            println!("{}{}", " ".repeat(indent), status_line);
        }
        println!();
    }
}

// More specialized version of print_fixed_width() for statuses.
// Handles coloring, manual annotation, etc 
pub fn print_fixed_width_status(rows: BTreeMap<String, Vec<StatusEntry>>, nspaces: Option<usize>, indent: Option<usize>, color: bool) {
    let indent = indent.unwrap_or(0);
    let nspaces = nspaces.unwrap_or(6);

    let max_cols = rows.values()
        .flat_map(|v| v.iter())
        .filter_map(|entry| {
            match &entry.cols {
                None => None,
                Some(cols) => Some(cols.len())
            }
        })
        .max()
        .unwrap_or(0);

        let mut max_lengths = vec![0; max_cols];

        // compute max lengths across all rows
        for row in rows.values().flat_map(|v| v.iter()) {
            if let Some(cols) = &row.cols {
                for (i, col) in cols.iter().enumerate() {
                    max_lengths[i] = max_lengths[i].max(col.width());
                }
            }
        }

        // print status table
        let mut keys: Vec<&String> = rows.keys().collect();
        keys.sort();
        for (key, value) in &rows {
            let pretty_key = if color { key.bold().to_string() } else { key.clone() };
            println!("[{}]", pretty_key);

            // Print the rows with the correct widths
            for row in value {
                let mut fixed_row = Vec::new();
                let tracked = &row.tracked;
                let local_status = &row.local_status;
                let remote_status = &row.remote_status;
                if let Some(cols) = &row.cols {
                    for (i, col) in cols.iter().enumerate() {
                        // push a fixed-width column to vector
                        let spacer = if i == 0 { " " } else { "" };
                        let fixed_col = format!("{}{:width$}", spacer, col, width = max_lengths[i]);
                        fixed_row.push(fixed_col);
                    }
                }
                let spacer = " ".repeat(nspaces);

                // color row
                let status_line = fixed_row.join(&spacer);
                let status_line = match (tracked, local_status, remote_status) {
                    (Some(true), LocalStatusCode::Current, Some(RemoteStatusCode::Current)) => status_line.green().to_string(),
                    (Some(true), LocalStatusCode::Current, None) => status_line.green().to_string(),
                    // not tracked, but on remote
                    (Some(false), LocalStatusCode::Current, Some(RemoteStatusCode::Current)) => status_line.cyan().to_string(),
                    // not tracked, not on remote
                    (Some(false), LocalStatusCode::Current, None) => status_line.yellow().to_string(),
                    (Some(false), LocalStatusCode::Current, Some(RemoteStatusCode::NotExists)) => status_line.yellow().to_string(),
                    (None, LocalStatusCode::Current, None) => status_line.green().to_string(),

                    (Some(true), LocalStatusCode::Modified, _)  => status_line.red().to_string(),
                    (Some(false), LocalStatusCode::Modified, _)  => status_line.red().to_string(),
                    (Some(true), LocalStatusCode::Current, Some(RemoteStatusCode::NotExists))  => status_line.yellow().to_string(),
                    (Some(true), LocalStatusCode::Current, Some(RemoteStatusCode::MD5Mismatch))  => status_line.yellow().to_string(),
                    (Some(false), LocalStatusCode::Current, _)  => status_line.green().to_string(),
                    _ => status_line.cyan().to_string()
                };
                println!("{}{}", " ".repeat(indent), status_line);
            }
            println!();
        }
}

fn organize_by_dir(rows: Vec<StatusEntry>) -> BTreeMap<String, Vec<StatusEntry>> {
    let mut dir_map: BTreeMap<String, Vec<StatusEntry>> = BTreeMap::new();

    for entry in rows {
        if let Some(cols) = &entry.cols {
            if let Some(first_elem) = cols.first() {
                let path = Path::new(first_elem);
                if let Some(parent_path) = path.parent() {
                    let parent_dir = parent_path.to_string_lossy().into_owned();
                    dir_map.entry(parent_dir).or_default().push(entry);
                }
            }
        }
    }
    dir_map
}

pub fn print_status(rows: Vec<StatusEntry>, remote: Option<&HashMap<String,Remote>>) {
    println!("{}", "Project data status:".bold());
    println!("{} data file{} registered.\n", rows.len(), if rows.len() > 1 {"s"} else {""});

    let organized_rows = organize_by_dir(rows);

    let rows_by_dir: BTreeMap<String, Vec<StatusEntry>> = match remote {
        Some(remote_map) => {
            let mut new_map = BTreeMap::new();
            for (key, value) in organized_rows {
                if let Some(remote) = remote_map.get(&key) {
                    let new_key = format!("{} > {}", key, remote.name());
                    new_map.insert(new_key, value);
                } else {
                    new_map.insert(key, value);
                }
            }
            new_map
        },
        None => organized_rows,
    };

    print_fixed_width_status(rows_by_dir, None, None, true);
}

pub fn format_bytes(size: u64) -> String {
    const BYTES_IN_KB: f64 = 1024.0;
    const BYTES_IN_MB: f64 = BYTES_IN_KB * 1024.0;
    const BYTES_IN_GB: f64 = BYTES_IN_MB * 1024.0;
    const BYTES_IN_TB: f64 = BYTES_IN_GB * 1024.0;
    const BYTES_IN_PB: f64 = BYTES_IN_TB * 1024.0;
    let size = size as f64;

    if size < BYTES_IN_MB {
        return format!("{:.2} MB", size / BYTES_IN_KB);
    } else if size < BYTES_IN_GB {
        return format!("{:.2} MB", size / BYTES_IN_MB);
    } else if size < BYTES_IN_TB {
        return format!("{:.2} GB", size / BYTES_IN_GB);
    } else if size < BYTES_IN_PB {
        return format!("{:.2} TB", size / BYTES_IN_TB);
    } else {
        return format!("{:.2} PB", size / BYTES_IN_PB);
    }
}


pub fn format_mod_time(mod_time: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration_since_mod = now.signed_duration_since(mod_time);

    // convert chrono::Duration to std::time::Duration
    let std_duration = std::time::Duration::new(
        duration_since_mod.num_seconds() as u64,
        0
        );

    let formatter = Formatter::new();
    let local_time = mod_time.with_timezone(&Local);
    let timestamp = local_time.format("%Y-%m-%d %l:%M%p").to_string();
    format!("{} ({})", timestamp, formatter.convert(std_duration))
}
