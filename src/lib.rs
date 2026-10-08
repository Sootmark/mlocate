//! The locate database (`/var/lib/mlocate/mlocate.db`, written by
//! `updatedb` for `locate`), for forensics: every directory the last run
//! indexed, with its modification time then and the names of the files and
//! folders in it. A record of what was on a Linux system, deleted files
//! included, as of that run (most distributions run it daily).
//!
//! A big-endian file:
//!
//! - a header: `\0mlocate`, the configuration block's size, the format
//!   version (0), whether `locate` must check the user may see a file, two
//!   padding bytes, and the root path `updatedb` started from;
//! - the configuration block: variables (`prunefs`, `prunenames`,
//!   `prunepaths`, `prune_bind_mounts`), each a name then its values,
//!   NUL-terminated strings ending with an empty one;
//! - directories, each: its modification time (seconds and nanoseconds
//!   since 1970), four padding bytes, its path, then its entries, each a
//!   type byte (0 a file, 1 a folder) and a name, until a type byte 2.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let db = mlocate::read(&std::fs::read("/var/lib/mlocate/mlocate.db")?);
//! for dir in &db.directories {
//!     for entry in &dir.entries {
//!         println!("{:?} {}/{}", dir.modified, dir.path, entry.name);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Damage goes to `problems`, never a panic.

use common::time::Ts;

/// This crate's version, for records of what parsed them.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The file's signature.
const SIGNATURE: &[u8; 8] = b"\0mlocate";
/// The header's fixed part: signature, size, version, flag, padding.
const HEADER: usize = 16;
/// An entry's type byte for a folder (0 is a file).
const FOLDER: u8 = 1;
/// The type byte ending a directory's entries.
const END: u8 = 2;

/// A locate database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Database {
    /// The path `updatedb` started from.
    pub root: String,
    /// Whether `locate` checks the user may see each file.
    pub require_visibility: bool,
    /// The configuration: each variable and its values.
    pub configuration: Vec<(String, Vec<String>)>,
    /// The directories, in file order.
    pub directories: Vec<Directory>,
    /// What couldn't be read.
    pub problems: Vec<String>,
}

/// A directory indexed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Directory {
    /// Its offset in the file.
    pub offset: u64,
    /// Its path.
    pub path: String,
    /// Its modification time when indexed (UTC); none when zero.
    pub modified: Option<Ts>,
    /// What it held.
    pub entries: Vec<Entry>,
}

/// A file or folder in a directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// Its name.
    pub name: String,
    /// Whether it is a folder.
    pub folder: bool,
}

/// Whether `head` starts like a locate database.
#[must_use]
pub fn is_mlocate(head: &[u8]) -> bool {
    head.starts_with(SIGNATURE)
}

/// Read a locate database.
#[must_use]
pub fn read(data: &[u8]) -> Database {
    let mut db = Database::default();
    if !is_mlocate(data) || data.len() < HEADER {
        db.problems.push("no mlocate signature".to_owned());
        return db;
    }
    let configuration_size = be_u32(&data[8..12]) as usize;
    if data[12] != 0 {
        db.problems
            .push(format!("format version {} not known", data[12]));
    }
    db.require_visibility = data[13] != 0;
    let mut cursor = Cursor { data, at: HEADER };
    let Some(root) = cursor.c_string() else {
        db.problems.push("a header cut short".to_owned());
        return db;
    };
    db.root = root;
    let configuration_end = cursor.at.saturating_add(configuration_size);
    let Some(block) = data.get(cursor.at..configuration_end) else {
        db.problems
            .push("a configuration block past the end".to_owned());
        return db;
    };
    db.configuration = configuration(block);
    cursor.at = configuration_end;
    while cursor.at < data.len() {
        let offset = cursor.at as u64;
        let Some(dir) = directory(&mut cursor) else {
            db.problems
                .push(format!("offset {offset:#x}: a directory cut short"));
            break;
        };
        db.directories.push(dir);
    }
    db
}

/// The configuration block's variables and values.
fn configuration(block: &[u8]) -> Vec<(String, Vec<String>)> {
    let mut cursor = Cursor { data: block, at: 0 };
    let mut variables = Vec::new();
    while let Some(name) = cursor.c_string() {
        let mut values = Vec::new();
        while let Some(value) = cursor.c_string() {
            if value.is_empty() {
                break;
            }
            values.push(value);
        }
        variables.push((name, values));
    }
    variables
}

/// A directory and its entries at the cursor.
fn directory(cursor: &mut Cursor<'_>) -> Option<Directory> {
    let offset = cursor.at as u64;
    let seconds = be_u64(cursor.take(8)?);
    let nanoseconds = be_u32(cursor.take(4)?);
    cursor.take(4)?;
    let path = cursor.c_string()?;
    let mut entries = Vec::new();
    loop {
        let kind = *cursor.take(1)?.first()?;
        if kind == END {
            break;
        }
        let name = cursor.c_string()?;
        entries.push(Entry {
            name,
            folder: kind == FOLDER,
        });
    }
    let modified = i64::try_from(seconds)
        .ok()
        .and_then(|s| s.checked_mul(1_000_000_000))
        .and_then(|ns| ns.checked_add(i64::from(nanoseconds)))
        .filter(|&ns| ns > 0)
        .map(Ts::from_unix_nanos);
    Some(Directory {
        offset,
        path,
        modified,
        entries,
    })
}

/// A reader over bytes.
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Option<&'a [u8]> {
        let bytes = self.data.get(self.at..self.at.checked_add(size)?)?;
        self.at += size;
        Some(bytes)
    }

    /// A NUL-terminated string (bytes as UTF-8, invalid ones replaced).
    fn c_string(&mut self) -> Option<String> {
        let rest = self.data.get(self.at..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        self.at += end + 1;
        Some(String::from_utf8_lossy(&rest[..end]).into_owned())
    }
}

fn be_u32(bytes: &[u8]) -> u32 {
    bytes.try_into().map(u32::from_be_bytes).unwrap_or_default()
}

fn be_u64(bytes: &[u8]) -> u64 {
    bytes.try_into().map(u64::from_be_bytes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_files() {
        assert!(!is_mlocate(b"not a database"));
        assert!(!read(b"\0mlocate").problems.is_empty());
    }
}
