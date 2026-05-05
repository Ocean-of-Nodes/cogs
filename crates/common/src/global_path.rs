//! Path — global address of a sub-object inside an entity.
//!
//! A `Path` is `<uuid>/<seg₁>[/<seg₂>...]`: a UUID identifying the
//! entity, followed by **at least one** field-name segment naming
//! the location inside that entity's `Object`. There is no "root"
//! form — to refer to an entity as a whole, use
//! [`Pointee::EntityId`](crate::Pointee::EntityId).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::EntityId;
use crate::local_path::{LocalObjPath, PathError as LocalError};

/// An owned `<uuid>/<seg₁>[/<seg₂>...]` path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlobalObjPath {
    entity: EntityId,
    /// Always non-root.
    local: LocalObjPath,
}

/// Reasons a string can fail to be a valid [`Path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// First component isn't a valid UUID.
    InvalidUuid,
    /// The UUID has no field segments after it. `Path` does not admit
    /// a root form.
    MissingSegments,
    /// The local part of the path failed to parse.
    Local(LocalError),
}

impl From<LocalError> for PathError {
    fn from(e: LocalError) -> Self {
        PathError::Local(e)
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUuid => f.write_str("first component must be a valid UUID"),
            Self::MissingSegments => {
                f.write_str("path must have at least one field segment after the UUID")
            }
            Self::Local(e) => write!(f, "local path: {e}"),
        }
    }
}

impl std::error::Error for PathError {}

impl GlobalObjPath {
    /// Build a path from an entity id and at least one field segment.
    pub fn new(entity: EntityId, first_segment: &str) -> Result<Self, PathError> {
        let local = LocalObjPath::new(first_segment)?;
        Ok(Self { entity, local })
    }

    /// Build a path from an entity id and a [`LocalObjPath`].
    pub fn from_parts(entity: EntityId, local: LocalObjPath) -> Self {
        Self { entity, local }
    }

    /// Parse a string of the form `<uuid>/<seg₁>[/<seg₂>...]`.
    pub fn parse(s: &str) -> Result<Self, PathError> {
        let (head, tail) = s.split_once('/').ok_or(PathError::MissingSegments)?;
        let entity = Uuid::parse_str(head).map_err(|_| PathError::InvalidUuid)?;
        if tail.is_empty() {
            return Err(PathError::MissingSegments);
        }
        // LocalPath wants a leading `/` for non-root inputs.
        let local = LocalObjPath::parse(&format!("/{tail}"))?;
        Ok(Self { entity, local })
    }

    /// The entity this path addresses.
    pub fn entity(&self) -> EntityId {
        self.entity
    }

    /// The local navigation inside the entity's object.
    pub fn local(&self) -> &LocalObjPath {
        &self.local
    }

    /// Append a single field segment.
    pub fn push(&mut self, segment: &str) -> Result<(), LocalError> {
        self.local.push(segment)
    }

    /// Iterate over field segments. Does **not** yield the UUID — use
    /// [`Path::entity`] for that.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.local.iter()
    }
}

impl TryFrom<&str> for GlobalObjPath {
    type Error = PathError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl TryFrom<String> for GlobalObjPath {
    type Error = PathError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl FromStr for GlobalObjPath {
    type Err = PathError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for GlobalObjPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // LocalPath display is `/seg/seg…` for non-root.
        write!(f, "{}{}", self.entity, self.local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }

    #[test]
    fn new_requires_one_segment() {
        let p = GlobalObjPath::new(u(), "field").unwrap();
        assert_eq!(p.entity(), u());
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["field"]);
    }

    #[test]
    fn new_rejects_empty_segment() {
        let err = GlobalObjPath::new(u(), "").unwrap_err();
        assert!(matches!(err, PathError::Local(LocalError::EmptySegment)));
    }

    #[test]
    fn new_rejects_segment_with_slash() {
        let err = GlobalObjPath::new(u(), "a/b").unwrap_err();
        assert!(matches!(
            err,
            PathError::Local(LocalError::SegmentContainsSlash)
        ));
    }

    #[test]
    fn from_parts_builds_path() {
        let local = LocalObjPath::new("field").unwrap();
        let p = GlobalObjPath::from_parts(u(), local);
        assert_eq!(p.entity(), u());
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["field"]);
    }

    #[test]
    fn parse_single_segment() {
        let s = format!("{}/field", u());
        let p = GlobalObjPath::parse(&s).unwrap();
        assert_eq!(p.entity(), u());
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["field"]);
    }

    #[test]
    fn parse_nested_segments() {
        let s = format!("{}/a/b/c", u());
        let p = GlobalObjPath::parse(&s).unwrap();
        assert_eq!(p.entity(), u());
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_missing_segments_no_slash() {
        assert_eq!(
            GlobalObjPath::parse(&u().to_string()),
            Err(PathError::MissingSegments)
        );
    }

    #[test]
    fn parse_missing_segments_trailing_slash_only() {
        let s = format!("{}/", u());
        assert_eq!(GlobalObjPath::parse(&s), Err(PathError::MissingSegments));
    }

    #[test]
    fn parse_invalid_uuid() {
        assert_eq!(GlobalObjPath::parse("not-a-uuid/field"), Err(PathError::InvalidUuid));
    }

    #[test]
    fn parse_double_slash_rejected() {
        let s = format!("{}/a//b", u());
        assert!(matches!(
            GlobalObjPath::parse(&s),
            Err(PathError::Local(LocalError::EmptySegment))
        ));
    }

    #[test]
    fn parse_trailing_slash_rejected() {
        let s = format!("{}/a/", u());
        assert!(matches!(
            GlobalObjPath::parse(&s),
            Err(PathError::Local(LocalError::TrailingSlash))
        ));
    }

    #[test]
    fn push_appends_segment() {
        let mut p = GlobalObjPath::new(u(), "a").unwrap();
        p.push("b").unwrap();
        p.push("c").unwrap();
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn display_round_trips() {
        let s = format!("{}/a/b", u());
        let p = GlobalObjPath::parse(&s).unwrap();
        assert_eq!(p.to_string(), s);
    }

    #[test]
    fn from_str_works() {
        let s = format!("{}/x", u());
        let p: GlobalObjPath = s.parse().unwrap();
        assert_eq!(p.entity(), u());
        assert_eq!(p.iter().collect::<Vec<_>>(), vec!["x"]);
    }
}
