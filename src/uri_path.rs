//! Cross-target shim for `Url`'s file-path conversions.
//!
//! On native targets these delegate to the real `url` crate methods. On
//! `wasm32-unknown-unknown` the `url` crate omits `to_file_path`/`from_file_path`
//! (there is no OS filesystem), so we provide a best-effort conversion that
//! treats `file://` URIs as opaque virtual paths. This keeps the ~two dozen
//! call sites compiling and behaving sensibly when PHPantom runs in the browser
//! against an in-memory document model.

use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

pub trait UrlPathExt: Sized {
    fn to_file_path_compat(&self) -> Result<PathBuf, ()>;
    fn from_file_path_compat<P: AsRef<Path>>(path: P) -> Result<Self, ()>;
}

#[cfg(not(target_arch = "wasm32"))]
impl UrlPathExt for Url {
    fn to_file_path_compat(&self) -> Result<PathBuf, ()> {
        self.to_file_path()
    }
    fn from_file_path_compat<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        Url::from_file_path(path)
    }
}

#[cfg(target_arch = "wasm32")]
impl UrlPathExt for Url {
    fn to_file_path_compat(&self) -> Result<PathBuf, ()> {
        if self.scheme() != "file" {
            return Err(());
        }
        Ok(PathBuf::from(percent_decode(self.path())))
    }

    fn from_file_path_compat<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        let p = path.as_ref().to_string_lossy();
        let mut s = String::from("file://");
        if !p.starts_with('/') {
            s.push('/');
        }
        s.push_str(&percent_encode(&p));
        Url::parse(&s).map_err(|_| ())
    }
}

#[cfg(target_arch = "wasm32")]
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_arch = "wasm32")]
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &byte in s.as_bytes() {
        match byte {
            // unreserved per RFC 3986, plus '/' which is a path separator
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(target_arch = "wasm32")]
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
