//! Slash-separated path used to address nested objects.
//! A simpler analogue of [`std::path::PathBuf`]:
//!
//! - Both `""` and `"/"` represent the **root**; iterating either
//!   yields no segments.
//! - `"/s1/s2"` iterates as `"s1"`, `"s2"`.
//! - Anything else — missing leading slash, trailing slash, double
//!   slashes, segments with embedded slashes — is rejected with a
//!   [`PathError`]. The path is canonical by construction.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An owned slash-separated path.
///
/// The only valid forms are:
/// - the empty string `""` (the root);
/// - `/seg₁/seg₂/.../segₙ` where every `segᵢ` is non-empty and
///   contains no `/`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Path {
    /// Always either `""` (root) or `/seg/.../seg` — never `"/"`,
    /// never doubled slashes, never trailing slash.
    inner: String,
}

/// Reasons a string can fail to be a valid [`Path`] (or a segment
/// can fail to be appended).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// A segment is empty (e.g. `"//"`, `"/a//b"`, `push("")`).
    EmptySegment,
    /// A non-root path doesn't start with `/`.
    MissingLeadingSlash,
    /// A non-root path ends with `/` (e.g. `"/a/"`). The only
    /// path that may be a single slash is the root, written `"/"`.
    TrailingSlash,
    /// A pushed segment contains an internal slash. Use
    /// [`Path::parse`] if you want to admit a multi-segment string.
    SegmentContainsSlash,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySegment => f.write_str("empty path segment"),
            Self::MissingLeadingSlash => f.write_str("path must start with `/`"),
            Self::TrailingSlash => f.write_str("path must not end with `/`"),
            Self::SegmentContainsSlash => f.write_str("segment must not contain `/`"),
        }
    }
}

impl std::error::Error for PathError {}

impl Path {
    /// Construct a root path.
    pub const fn new() -> Self {
        Self {
            inner: String::new(),
        }
    }

    /// Parse a slash-separated string. Accepts `""` and `"/"` as the
    /// root, and `/s1/s2/.../sn` for any non-zero `n`. Rejects every
    /// non-canonical form.
    pub fn parse(s: &str) -> Result<Self, PathError> {
        if s.is_empty() || s == "/" {
            return Ok(Self::new());
        }
        if !s.starts_with('/') {
            return Err(PathError::MissingLeadingSlash);
        }
        if s.ends_with('/') {
            return Err(PathError::TrailingSlash);
        }
        // Walk segments after the leading `/`. Any empty one means a
        // doubled slash.
        for seg in s[1..].split('/') {
            if seg.is_empty() {
                return Err(PathError::EmptySegment);
            }
        }
        Ok(Self {
            inner: s.to_owned(),
        })
    }

    /// Append a single segment. The segment must be non-empty and
    /// must not contain `/`. To add multiple segments, call `push`
    /// multiple times.
    pub fn push(&mut self, segment: &str) -> Result<(), PathError> {
        if segment.is_empty() {
            return Err(PathError::EmptySegment);
        }
        if segment.contains('/') {
            return Err(PathError::SegmentContainsSlash);
        }
        self.inner.push('/');
        self.inner.push_str(segment);
        Ok(())
    }

    /// `true` if the path has no segments.
    pub fn is_root(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate path segments.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            // Skip the leading `/` (if any) so split doesn't yield a
            // leading empty segment.
            inner: self
                .inner
                .strip_prefix('/')
                .unwrap_or(&self.inner)
                .split('/'),
            done: self.inner.is_empty(),
        }
    }

    /// Borrow the underlying string. The root form is always `""`.
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl TryFrom<&str> for Path {
    type Error = PathError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Path {
    type Error = PathError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inner.is_empty() {
            f.write_str("/")
        } else {
            f.write_str(&self.inner)
        }
    }
}

impl<'a> IntoIterator for &'a Path {
    type Item = &'a str;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the segments of a [`Path`].
pub struct Iter<'a> {
    inner: std::str::Split<'a, char>,
    /// `true` when the path was empty — `Split` over `""` would yield
    /// one empty item, which we don't want.
    done: bool,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.done {
            return None;
        }
        self.inner.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_is_root() {
        let p = Path::parse("").unwrap();
        assert!(p.is_root());
        assert!(p.iter().next().is_none());
    }

    #[test]
    fn slash_is_root() {
        let p = Path::parse("/").unwrap();
        assert!(p.is_root());
        assert!(p.iter().next().is_none());
    }

    #[test]
    fn iterates_segments() {
        let p = Path::parse("/s1/s2").unwrap();
        let segs: Vec<&str> = p.iter().collect();
        assert_eq!(segs, vec!["s1", "s2"]);
    }

    #[test]
    fn missing_leading_slash_rejected() {
        assert_eq!(Path::parse("s1/s2"), Err(PathError::MissingLeadingSlash));
    }

    #[test]
    fn trailing_slash_rejected() {
        assert_eq!(Path::parse("/s1/"), Err(PathError::TrailingSlash));
    }

    #[test]
    fn double_slash_rejected() {
        assert_eq!(Path::parse("//s1"), Err(PathError::EmptySegment));
        assert_eq!(Path::parse("/a//b"), Err(PathError::EmptySegment));
    }

    #[test]
    fn push_appends_segment() {
        let mut p = Path::new();
        p.push("s1").unwrap();
        p.push("s2").unwrap();
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["s1", "s2"]);
        assert_eq!(p.as_str(), "/s1/s2");
    }

    #[test]
    fn push_empty_rejected() {
        let mut p = Path::new();
        assert_eq!(p.push(""), Err(PathError::EmptySegment));
    }

    #[test]
    fn push_segment_with_slash_rejected() {
        let mut p = Path::new();
        assert_eq!(p.push("a/b"), Err(PathError::SegmentContainsSlash));
        assert_eq!(p.push("/a"), Err(PathError::SegmentContainsSlash));
    }

    #[test]
    fn try_from_str() {
        let p: Path = "/x/y".try_into().unwrap();
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["x", "y"]);

        let err: Result<Path, _> = "x/y".try_into();
        assert_eq!(err, Err(PathError::MissingLeadingSlash));
    }

    #[test]
    fn into_iter_for_reference() {
        let p = Path::parse("/a/b/c").unwrap();
        let collected: Vec<_> = (&p).into_iter().collect();
        assert_eq!(collected, vec!["a", "b", "c"]);
    }

    #[test]
    fn display_root_as_slash() {
        assert_eq!(Path::new().to_string(), "/");
        assert_eq!(Path::parse("/").unwrap().to_string(), "/");
        assert_eq!(Path::parse("/s1/s2").unwrap().to_string(), "/s1/s2");
    }

    #[test]
    fn equality_is_byte_for_byte() {
        // No normalisation — there's only one valid representation
        // of any given path.
        assert_eq!(Path::parse("/a/b").unwrap(), Path::parse("/a/b").unwrap());
        assert_ne!(Path::new(), Path::parse("/a").unwrap());
    }
}
