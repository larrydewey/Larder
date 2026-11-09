//! Zip reading with zip-bomb and path-traversal protection.

use std::cell::Cell;
use std::io::{Cursor, Read};
use std::rc::Rc;

use crate::ImportError;

/// Maximum decompressed size of a single entry.
pub const MAX_ENTRY_BYTES: u64 = 100 * 1024 * 1024;
/// Maximum total decompressed bytes read from one upload (nested archives included).
pub const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
/// Maximum number of entries considered in one archive.
pub const MAX_ENTRIES: usize = 50_000;

/// Remaining decompression budget, shared by nested archives.
#[derive(Clone)]
pub(crate) struct Budget(Rc<Cell<u64>>);

impl Budget {
    pub fn new(total: u64) -> Self {
        Budget(Rc::new(Cell::new(total)))
    }

    pub fn take(&self, n: u64) -> Result<(), ImportError> {
        let left = self.0.get();
        if n > left {
            return Err(ImportError::TooLarge("archive expands beyond the allowed total size".into()));
        }
        self.0.set(left - n);
        Ok(())
    }

    fn remaining(&self) -> u64 {
        self.0.get()
    }
}

struct Entry {
    /// Normalised path with '/' separators, no leading slash.
    pub name: String,
    index: usize,
}

pub(crate) struct SafeZip {
    zip: zip::ZipArchive<Cursor<Vec<u8>>>,
    entries: Vec<Entry>,
    budget: Budget,
}

pub(crate) fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

pub(crate) fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

impl SafeZip {
    pub fn open(bytes: Vec<u8>, budget: Budget) -> Result<Self, ImportError> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| ImportError::InvalidArchive(e.to_string()))?;
        let mut entries = vec![];
        for index in 0..zip.len().min(MAX_ENTRIES) {
            let Ok(f) = zip.by_index_raw(index) else { continue };
            if f.is_dir() {
                continue;
            }
            // enclosed_name() rejects absolute paths and any `..` escaping the root.
            let Some(path) = f.enclosed_name() else {
                tracing::warn!(name = f.name(), "skipping zip entry with unsafe path");
                continue;
            };
            let name: String = path
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            // Zips made on Windows sometimes use backslashes.
            let name = name.replace('\\', "/");
            if name.is_empty() || name.split('/').any(|p| p == ".." || p.is_empty()) {
                continue;
            }
            let base = name.rsplit('/').next().unwrap_or(&name);
            if name.starts_with("__MACOSX/") || base.starts_with("._") || base == ".DS_Store" || base == "Thumbs.db" {
                continue;
            }
            entries.push(Entry { name, index });
        }
        Ok(SafeZip { zip, entries, budget })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.name.as_str())
    }

    pub fn budget(&self) -> Budget {
        self.budget.clone()
    }

    /// Finds an entry by exact (normalised) name.
    pub fn find(&self, name: &str) -> Option<&str> {
        let name = name.trim_start_matches("./").trim_start_matches('/');
        self.entries.iter().find(|e| e.name == name).map(|e| e.name.as_str())
    }

    /// Finds an entry by exact name, falling back to a case-insensitive match.
    pub fn find_ci(&self, name: &str) -> Option<String> {
        let name = name.trim_start_matches("./").trim_start_matches('/');
        self.find(name)
            .map(str::to_string)
            .or_else(|| self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).map(|e| e.name.clone()))
    }

    /// Reads an entry, enforcing the per-entry and total limits.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, ImportError> {
        let idx = self
            .entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.index)
            .ok_or_else(|| ImportError::InvalidArchive(format!("missing entry {name}")))?;
        let f = self.zip.by_index(idx).map_err(|e| ImportError::InvalidArchive(e.to_string()))?;
        if f.size() > MAX_ENTRY_BYTES {
            return Err(ImportError::TooLarge(format!("{name} is larger than {} MB", MAX_ENTRY_BYTES / 1024 / 1024)));
        }
        let limit = MAX_ENTRY_BYTES.min(self.budget.remaining());
        let mut buf = Vec::with_capacity(f.size().min(limit) as usize);
        // Don't trust the header: cap the actual decompressed stream.
        f.take(limit + 1).read_to_end(&mut buf).map_err(|e| ImportError::InvalidArchive(format!("{name}: {e}")))?;
        if buf.len() as u64 > limit {
            return Err(ImportError::TooLarge(format!("{name} expands beyond the allowed size")));
        }
        self.budget.take(buf.len() as u64)?;
        Ok(buf)
    }

    pub fn read_string(&mut self, name: &str) -> Result<String, ImportError> {
        let b = self.read(name)?;
        Ok(String::from_utf8_lossy(strip_bom(&b)).into_owned())
    }
}

pub(crate) fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

/// Decompresses gzip data with a size cap.
pub(crate) fn gunzip_limited(bytes: &[u8], limit: u64) -> Result<Vec<u8>, ImportError> {
    let mut out = vec![];
    flate2::read::GzDecoder::new(bytes)
        .take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|e| ImportError::InvalidFormat(format!("invalid gzip data: {e}")))?;
    if out.len() as u64 > limit {
        return Err(ImportError::TooLarge("gzip data expands beyond the allowed size".into()));
    }
    Ok(out)
}

/// Directory part of a path ("a/b/c.json" -> "a/b", "c.json" -> "").
pub(crate) fn dirname(p: &str) -> &str {
    p.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

pub(crate) fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// Resolves a relative reference (e.g. an `<img src>`) against a directory,
/// refusing to leave the archive root.
pub(crate) fn resolve_relative(dir: &str, rel: &str) -> Option<String> {
    let rel = rel.split(['?', '#']).next().unwrap_or(rel);
    let rel = percent_decode(rel);
    let mut parts: Vec<&str> = if rel.starts_with('/') { vec![] } else { dir.split('/').filter(|s| !s.is_empty()).collect() };
    for seg in rel.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() && s.is_char_boundary(i + 3) {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
pub(crate) mod testutil {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    /// Builds an in-memory zip from (name, bytes) pairs.
    pub fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            w.start_file(*name, SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    pub fn gzip(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::make_zip;
    use super::*;

    #[test]
    fn traversal_entries_are_skipped() {
        let z = make_zip(&[("../evil.json", b"{}"), ("/abs.json", b"{}"), ("ok/a.json", b"{}"), ("__MACOSX/._a", b"x")]);
        let zip = SafeZip::open(z, Budget::new(MAX_TOTAL_BYTES)).unwrap();
        let names: Vec<_> = zip.names().collect();
        assert!(names.contains(&"ok/a.json"));
        assert!(!names.iter().any(|n| n.contains("evil")));
        assert!(!names.iter().any(|n| n.contains("MACOSX")));
    }

    #[test]
    fn budget_is_enforced() {
        let big = vec![b'a'; 10_000];
        let z = make_zip(&[("a.txt", &big), ("b.txt", &big)]);
        let mut zip = SafeZip::open(z, Budget::new(15_000)).unwrap();
        assert!(zip.read("a.txt").is_ok());
        assert!(matches!(zip.read("b.txt"), Err(ImportError::TooLarge(_))));
    }

    #[test]
    fn relative_paths() {
        assert_eq!(resolve_relative("", "images/a%20b.jpg").as_deref(), Some("images/a b.jpg"));
        assert_eq!(resolve_relative("x", "../images/a.jpg").as_deref(), Some("images/a.jpg"));
        assert_eq!(resolve_relative("", "../../etc/passwd"), None);
    }
}
