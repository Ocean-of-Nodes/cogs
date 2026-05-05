//! Slash-separated path used to address nested objects.
//! A simpler analogue of [`std::path::PathBuf`]:
//!
//! - Every path has **at least one** segment — there is no root form.
//!   The minimal valid path is `/seg`.
//! - `"/s1/s2"` iterates as `"s1"`, `"s2"`.
//! - Anything else — empty input, a lone `/`, missing leading slash,
//!   trailing slash, doubled slashes, segments with embedded slashes
//!   — is rejected with a [`PathError`]. The path is canonical by
//!   construction.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An owned slash-separated path with at least one segment.
///
/// The only valid form is `/seg₁[/seg₂.../segₙ]` where every `segᵢ`
/// is non-empty and contains no `/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalObjPath {
    /// Always `/seg[/seg...]` — never empty, never `"/"`, never
    /// doubled slashes, never trailing slash.
    inner: String,
}

/// Reasons a string can fail to be a valid [`LocalObjPath`] (or a
/// segment can fail to be appended).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// A segment is empty (e.g. `"/"`, `"//"`, `"/a//b"`, `push("")`).
    EmptySegment,
    /// The input doesn't start with `/`.
    MissingLeadingSlash,
    /// The input ends with `/` (e.g. `"/a/"`).
    TrailingSlash,
    /// A pushed segment contains an internal slash. Use
    /// [`LocalObjPath::parse`] if you want to admit a multi-segment
    /// string.
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

impl LocalObjPath {
    /// Construct a path from a single segment. The segment must be
    /// non-empty and must not contain `/`.
    pub fn new(first_segment: &str) -> Result<Self, PathError> {
        if first_segment.is_empty() {
            return Err(PathError::EmptySegment);
        }
        if first_segment.contains('/') {
            return Err(PathError::SegmentContainsSlash);
        }
        let mut inner = String::with_capacity(first_segment.len() + 1);
        inner.push('/');
        inner.push_str(first_segment);
        Ok(Self { inner })
    }

    /// Parse a slash-separated string `/s1[/s2.../sn]` with at least
    /// one segment. Empty input and a lone `"/"` are both rejected.
    pub fn parse(s: &str) -> Result<Self, PathError> {
        if !s.starts_with('/') {
            return Err(PathError::MissingLeadingSlash);
        }
        if s == "/" {
            return Err(PathError::EmptySegment);
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

    /// Iterate path segments. Always yields at least one segment.
    pub fn iter(&self) -> Iter<'_> {
        // `inner` is invariantly `/seg[/seg...]`, so stripping the
        // leading `/` and splitting on `/` gives the segments.
        Iter {
            inner: self.inner[1..].split('/'),
        }
    }

    /// Borrow the underlying string. Always begins with `/`.
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl TryFrom<&str> for LocalObjPath {
    type Error = PathError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl TryFrom<String> for LocalObjPath {
    type Error = PathError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl fmt::Display for LocalObjPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl<'a> IntoIterator for &'a LocalObjPath {
    type Item = &'a str;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the segments of a [`LocalObjPath`].
pub struct Iter<'a> {
    inner: std::str::Split<'a, char>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        self.inner.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_rejected() {
        assert_eq!(LocalObjPath::parse(""), Err(PathError::MissingLeadingSlash));
    }

    #[test]
    fn lone_slash_rejected() {
        assert_eq!(LocalObjPath::parse("/"), Err(PathError::EmptySegment));
    }

    #[test]
    fn single_segment_accepted() {
        let p = LocalObjPath::parse("/field1").unwrap();
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["field1"]);
    }

    #[test]
    fn iterates_segments() {
        let p = LocalObjPath::parse("/s1/s2").unwrap();
        let segs: Vec<&str> = p.iter().collect();
        assert_eq!(segs, vec!["s1", "s2"]);
    }

    #[test]
    fn missing_leading_slash_rejected() {
        assert_eq!(LocalObjPath::parse("s1/s2"), Err(PathError::MissingLeadingSlash));
    }

    #[test]
    fn trailing_slash_rejected() {
        assert_eq!(LocalObjPath::parse("/s1/"), Err(PathError::TrailingSlash));
    }

    #[test]
    fn double_slash_rejected() {
        assert_eq!(LocalObjPath::parse("//s1"), Err(PathError::EmptySegment));
        assert_eq!(LocalObjPath::parse("/a//b"), Err(PathError::EmptySegment));
    }

    #[test]
    fn new_requires_segment() {
        let p = LocalObjPath::new("field1").unwrap();
        assert_eq!(p.as_str(), "/field1");
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["field1"]);
    }

    #[test]
    fn new_empty_rejected() {
        assert_eq!(LocalObjPath::new(""), Err(PathError::EmptySegment));
    }

    #[test]
    fn new_segment_with_slash_rejected() {
        assert_eq!(LocalObjPath::new("a/b"), Err(PathError::SegmentContainsSlash));
        assert_eq!(LocalObjPath::new("/a"), Err(PathError::SegmentContainsSlash));
    }

    #[test]
    fn push_appends_segment() {
        let mut p = LocalObjPath::new("s1").unwrap();
        p.push("s2").unwrap();
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["s1", "s2"]);
        assert_eq!(p.as_str(), "/s1/s2");
    }

    #[test]
    fn push_empty_rejected() {
        let mut p = LocalObjPath::new("s1").unwrap();
        assert_eq!(p.push(""), Err(PathError::EmptySegment));
    }

    #[test]
    fn push_segment_with_slash_rejected() {
        let mut p = LocalObjPath::new("s1").unwrap();
        assert_eq!(p.push("a/b"), Err(PathError::SegmentContainsSlash));
        assert_eq!(p.push("/a"), Err(PathError::SegmentContainsSlash));
    }

    #[test]
    fn try_from_str() {
        let p: LocalObjPath = "/x/y".try_into().unwrap();
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["x", "y"]);

        let err: Result<LocalObjPath, _> = "x/y".try_into();
        assert_eq!(err, Err(PathError::MissingLeadingSlash));
    }

    #[test]
    fn into_iter_for_reference() {
        let p = LocalObjPath::parse("/a/b/c").unwrap();
        let collected: Vec<_> = (&p).into_iter().collect();
        assert_eq!(collected, vec!["a", "b", "c"]);
    }

    #[test]
    fn display_shows_path() {
        assert_eq!(LocalObjPath::new("s1").unwrap().to_string(), "/s1");
        assert_eq!(LocalObjPath::parse("/s1/s2").unwrap().to_string(), "/s1/s2");
    }

    #[test]
    fn equality_is_byte_for_byte() {
        assert_eq!(LocalObjPath::parse("/a/b").unwrap(), LocalObjPath::parse("/a/b").unwrap());
        assert_ne!(LocalObjPath::new("a").unwrap(), LocalObjPath::parse("/a/b").unwrap());
    }
}
