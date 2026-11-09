//! A minimal in-memory [`typst::World`]: the template sources are compiled
//! into the binary, the book data and photos are virtual files, fonts come
//! from [`crate::fonts`]. Nothing is read from disk or the network.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::anyhow;
use chrono::Datelike;
use typst::diag::{FileError, FileResult, Severity, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration, Smart};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_layout::PagedDocument;

use crate::fonts::FONTS;

pub const MAIN_TEMPLATE: &str = include_str!("../templates/cookbook.typ");

/// Template sources: (virtual path, text).
const TEMPLATES: &[(&str, &str)] = &[("/cookbook.typ", MAIN_TEMPLATE), ("/themes.typ", include_str!("../templates/themes.typ"))];

static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::default()));

fn file_id(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).expect("valid virtual path");
    FileId::new(RootedPath::new(VirtualRoot::Project, vpath))
}

pub struct BookWorld {
    main: FileId,
    sources: HashMap<FileId, Source>,
    files: HashMap<FileId, Bytes>,
}

impl BookWorld {
    pub fn new(data_json: Vec<u8>, images: Vec<(String, Vec<u8>)>) -> Self {
        let mut sources = HashMap::new();
        let mut files = HashMap::new();
        for (path, text) in TEMPLATES {
            let id = file_id(path);
            sources.insert(id, Source::new(id, text.to_string()));
            files.insert(id, Bytes::from_string(*text));
        }
        files.insert(file_id("/data.json"), Bytes::new(data_json));
        for (path, bytes) in images {
            files.insert(file_id(&path), Bytes::new(bytes));
        }
        Self { main: file_id("/cookbook.typ"), sources, files }
    }

    /// Lays out the document and exports it as PDF.
    pub fn compile_pdf(&self, title: &str) -> anyhow::Result<Vec<u8>> {
        let started = Instant::now();
        let warned = typst::compile::<PagedDocument>(self);
        for w in warned.warnings.iter().take(5) {
            tracing::debug!(warning = %w.message, "cookbook typst warning");
        }
        let doc = warned.output.map_err(|errs| anyhow!("cookbook layout failed: {}", self.describe(&errs)))?;
        let layout = started.elapsed();
        let options = typst_pdf::PdfOptions { ident: Smart::Custom(format!("larder-cookbook:{title}")), ..Default::default() };
        let pdf = typst_pdf::pdf(&doc, &options).map_err(|errs| anyhow!("cookbook PDF export failed: {}", self.describe(&errs)))?;
        tracing::debug!(
            pages = doc.pages().len(),
            layout_ms = layout.as_millis() as u64,
            total_ms = started.elapsed().as_millis() as u64,
            "typst compile finished"
        );
        // Typst memoises aggressively; keep a long-running server's memory in check.
        typst::comemo::evict(10);
        Ok(pdf)
    }

    /// Formats Typst diagnostics as "message (at file:line:col) — hint: …".
    fn describe(&self, diags: &[SourceDiagnostic]) -> String {
        let mut out = String::new();
        for (n, d) in diags.iter().filter(|d| d.severity == Severity::Error).take(8).enumerate() {
            if n > 0 {
                out.push_str("; ");
            }
            out.push_str(&d.message);
            if let Some(id) = d.span.id() {
                let path = id.get().vpath().get_with_slash().to_string();
                let pos = self.range(d.span).and_then(|r| self.sources.get(&id).and_then(|s| s.lines().byte_to_line_column(r.start)));
                match pos {
                    Some((line, col)) => {
                        let _ = write!(out, " (at {path}:{}:{})", line + 1, col + 1);
                    }
                    None => {
                        let _ = write!(out, " (in {path})");
                    }
                }
            }
            for h in &d.hints {
                let _ = write!(out, " — hint: {}", h.v);
            }
        }
        if out.is_empty() {
            out.push_str("unknown error");
        }
        out
    }
}

impl World for BookWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match self.sources.get(&id) {
            Some(s) => Ok(s.clone()),
            None if self.files.contains_key(&id) => Err(FileError::NotSource),
            None => Err(FileError::NotFound(std::path::PathBuf::from(id.get().vpath().get_without_slash()))),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).cloned().ok_or_else(|| FileError::NotFound(std::path::PathBuf::from(id.get().vpath().get_without_slash())))
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index).cloned()
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        let now = chrono::Utc::now();
        let date = match offset {
            Some(o) => (now + chrono::Duration::seconds(o.seconds() as i64)).date_naive(),
            None => chrono::Local::now().date_naive(),
        };
        Datetime::from_ymd(date.year(), date.month() as u8, date.day() as u8)
    }
}
