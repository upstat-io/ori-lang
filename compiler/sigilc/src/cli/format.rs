// CLI format command for Sigil
// Provides code formatting functionality

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

/// Format a file and print to stdout or write back
pub fn format_file(path: &str, in_place: bool) {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        }
    };

    match sigilc::format::format(&source) {
        Ok(formatted) => {
            if in_place {
                if let Err(e) = fs::write(path, &formatted) {
                    eprintln!("Error writing {}: {}", path, e);
                    std::process::exit(1);
                }
                println!("Formatted {}", path);
            } else {
                print!("{}", formatted);
            }
        }
        Err(e) => {
            eprintln!("Error formatting {}: {}", path, e);
            std::process::exit(1);
        }
    }
}

/// Format code from stdin and print to stdout
pub fn format_stdin() {
    let mut source = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut source) {
        eprintln!("Error reading stdin: {}", e);
        std::process::exit(1);
    }

    match sigilc::format::format(&source) {
        Ok(formatted) => {
            print!("{}", formatted);
        }
        Err(e) => {
            eprintln!("Error formatting: {}", e);
            std::process::exit(1);
        }
    }
}

/// Format all .si files in a directory recursively
pub fn format_directory(dir: &str, in_place: bool) {
    let path = Path::new(dir);
    if !path.is_dir() {
        eprintln!("{} is not a directory", dir);
        std::process::exit(1);
    }

    format_dir_recursive(path, in_place);
}

fn format_dir_recursive(dir: &Path, in_place: bool) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", dir.display(), e);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            format_dir_recursive(&path, in_place);
        } else if path.extension().map_or(false, |ext| ext == "si") {
            format_file(path.to_str().unwrap_or(""), in_place);
        }
    }
}

/// Check if files are formatted (returns error if not)
pub fn check_formatted(path: &str) -> bool {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return false;
        }
    };

    match sigilc::format::format(&source) {
        Ok(formatted) => {
            if source == formatted {
                true
            } else {
                eprintln!("{} is not formatted", path);
                false
            }
        }
        Err(e) => {
            eprintln!("Error formatting {}: {}", path, e);
            false
        }
    }
}

/// Check if all .si files in a directory are formatted
pub fn check_directory(dir: &str) -> bool {
    let path = Path::new(dir);
    if !path.is_dir() {
        return check_formatted(dir);
    }

    check_dir_recursive(path)
}

fn check_dir_recursive(dir: &Path) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", dir.display(), e);
            return false;
        }
    };

    let mut all_formatted = true;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !check_dir_recursive(&path) {
                all_formatted = false;
            }
        } else if path.extension().map_or(false, |ext| ext == "si") {
            if !check_formatted(path.to_str().unwrap_or("")) {
                all_formatted = false;
            }
        }
    }

    all_formatted
}
