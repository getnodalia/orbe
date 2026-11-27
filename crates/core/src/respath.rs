// SPDX-License-Identifier: MIT
use core::fmt;

/// Where a resource path is rooted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scheme {
    /// Inside the project folder (`res://`).
    Res,
    /// Inside the per-user data folder (`user://`).
    User,
}

/// Why a path was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    /// It does not start with `res://` or `user://`.
    UnknownScheme,
    /// It contains a backslash, a NUL or a control character.
    InvalidCharacter,
    /// A `..` would climb above the root.
    EscapesRoot,
    /// It is longer than [`ResPath::MAX_LEN`].
    TooLong,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PathError::UnknownScheme => "path must start with res:// or user://",
            PathError::InvalidCharacter => "path contains an invalid character",
            PathError::EscapesRoot => "path escapes its root",
            PathError::TooLong => "path is too long",
        })
    }
}

impl std::error::Error for PathError {}

/// A normalised project or user resource path. `.` and empty segments are dropped and `..` is
/// resolved; a path that would leave its root is an error, so a `ResPath` can always be joined
/// onto its root folder safely.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResPath {
    scheme: Scheme,
    segments: Vec<String>,
}

impl ResPath {
    /// Longest accepted path, in bytes.
    pub const MAX_LEN: usize = 4096;

    /// Parses and normalises a path.
    pub fn parse(s: &str) -> Result<ResPath, PathError> {
        if s.len() > Self::MAX_LEN {
            return Err(PathError::TooLong);
        }
        let (scheme, rest) = if let Some(r) = s.strip_prefix("res://") {
            (Scheme::Res, r)
        } else if let Some(r) = s.strip_prefix("user://") {
            (Scheme::User, r)
        } else {
            return Err(PathError::UnknownScheme);
        };
        if rest.chars().any(|c| c == '\\' || c.is_control()) {
            return Err(PathError::InvalidCharacter);
        }
        let mut segments: Vec<String> = Vec::new();
        for seg in rest.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    segments.pop().ok_or(PathError::EscapesRoot)?;
                }
                s => segments.push(s.to_string()),
            }
        }
        Ok(ResPath { scheme, segments })
    }

    /// The root this path lives under.
    pub fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// The normalised segments, root excluded.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The last segment's extension, lower-cased.
    pub fn extension(&self) -> Option<String> {
        let name = self.segments.last()?;
        let (stem, ext) = name.rsplit_once('.')?;
        if stem.is_empty() || ext.is_empty() {
            None
        } else {
            Some(ext.to_ascii_lowercase())
        }
    }
}

impl fmt::Display for ResPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.scheme {
            Scheme::Res => "res://",
            Scheme::User => "user://",
        })?;
        f.write_str(&self.segments.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises() {
        let p = ResPath::parse("res://a/./b//c/../d.PNG").unwrap();
        assert_eq!(p.to_string(), "res://a/b/d.PNG");
        assert_eq!(p.extension().as_deref(), Some("png"));
        assert_eq!(ResPath::parse("user://").unwrap().to_string(), "user://");
    }

    #[test]
    fn rejects_hostile_paths() {
        assert_eq!(
            ResPath::parse("res://../etc/passwd"),
            Err(PathError::EscapesRoot)
        );
        assert_eq!(
            ResPath::parse("res://a/../../b"),
            Err(PathError::EscapesRoot)
        );
        assert_eq!(ResPath::parse("/etc/passwd"), Err(PathError::UnknownScheme));
        assert_eq!(
            ResPath::parse("res://a\\b"),
            Err(PathError::InvalidCharacter)
        );
        assert_eq!(
            ResPath::parse("res://a\0b"),
            Err(PathError::InvalidCharacter)
        );
        let long = format!("res://{}", "a".repeat(ResPath::MAX_LEN));
        assert_eq!(ResPath::parse(&long), Err(PathError::TooLong));
    }

    #[test]
    fn extension_edge_cases() {
        assert_eq!(ResPath::parse("res://.hidden").unwrap().extension(), None);
        assert_eq!(ResPath::parse("res://a.").unwrap().extension(), None);
        assert_eq!(ResPath::parse("res://").unwrap().extension(), None);
    }
}
