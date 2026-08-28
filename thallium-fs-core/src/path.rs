//! Non-lossy Linux filesystem paths.
//!
//! Informed by `FS-VFS-RESEARCH.md` section 1 (path and URL handling) and the
//! `NativePath(Vec<u8>)` decision in `PHASE2-PLAN.md` section 2.

use std::{borrow::Borrow, ffi::{OsStr, OsString}, fmt, os::unix::ffi::{OsStrExt, OsStringExt}, path::{Path, PathBuf}};

/// An owned Linux filesystem path represented without a UTF-8 requirement.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativePath(pub Vec<u8>);

impl NativePath {
	/// Creates a native path from its exact filesystem bytes.
	#[must_use]
	pub const fn new(bytes: Vec<u8>) -> Self { Self(bytes) }

	/// Returns the exact filesystem bytes.
	#[must_use]
	pub fn as_bytes(&self) -> &[u8] { &self.0 }

	/// Borrows this value as a standard library path without changing its bytes.
	#[must_use]
	pub fn as_path(&self) -> &Path { Path::new(OsStr::from_bytes(&self.0)) }

	/// Returns whether this is an absolute Linux path.
	#[must_use]
	pub fn is_absolute(&self) -> bool { self.0.first() == Some(&b'/') }

	/// Returns whether the path contains no bytes.
	#[must_use]
	pub fn is_empty(&self) -> bool { self.0.is_empty() }

	/// Lexically normalizes separators, `.` components, and resolvable `..`
	/// components without accessing the filesystem.
	///
	/// Leading `..` components of relative paths are retained. Attempts to walk
	/// above an absolute root remain at the root. An empty relative result stays
	/// empty rather than being converted to `.`, so callers cannot accidentally
	/// introduce an implicit process-current-directory lookup.
	#[must_use]
	pub fn normalize(&self) -> Self {
		let absolute = self.is_absolute();
		let mut parts: Vec<&[u8]> = Vec::new();

		for part in self.0.split(|byte| *byte == b'/') {
			match part {
				b"" | b"." => {}
				b".." if parts.last().is_some_and(|last| *last != b"..") => {
					parts.pop();
				}
				b".." if absolute => {}
				_ => parts.push(part),
			}
		}

		let capacity = self.0.len().max(usize::from(absolute));
		let mut bytes = Vec::with_capacity(capacity);
		if absolute {
			bytes.push(b'/');
		}
		for (index, part) in parts.into_iter().enumerate() {
			if index > 0 || absolute && bytes.len() > 1 {
				bytes.push(b'/');
			}
			bytes.extend_from_slice(part);
		}
		Self(bytes)
	}

	/// Appends a relative path component and lexically normalizes the result.
	///
	/// An absolute `child` replaces the receiver, matching standard path-join
	/// semantics while remaining independent of the process working directory.
	#[must_use]
	pub fn join(&self, child: impl AsRef<Self>) -> Self {
		let child = child.as_ref();
		if child.is_absolute() || self.is_empty() {
			return child.normalize();
		}
		if child.is_empty() {
			return self.normalize();
		}

		let mut bytes = Vec::with_capacity(self.0.len() + child.0.len() + 1);
		bytes.extend_from_slice(&self.0);
		if !bytes.ends_with(b"/") {
			bytes.push(b'/');
		}
		bytes.extend_from_slice(&child.0);
		Self(bytes).normalize()
	}

	/// Returns the lexical parent, or `None` for a root or single relative name.
	#[must_use]
	pub fn parent(&self) -> Option<Self> {
		let normalized = self.normalize();
		if normalized.0 == b"/" || normalized.0.is_empty() {
			return None;
		}

		let index = normalized.0.iter().rposition(|byte| *byte == b'/')?;
		if index == 0 { Some(Self(vec![b'/'])) } else { Some(Self(normalized.0[..index].to_vec())) }
	}

	/// Returns the final lexical component, if one exists.
	#[must_use]
	pub fn file_name(&self) -> Option<Self> {
		let normalized = self.normalize();
		if normalized.0 == b"/" || normalized.0.is_empty() {
			return None;
		}
		let start = normalized.0.iter().rposition(|byte| *byte == b'/').map_or(0, |i| i + 1);
		Some(Self(normalized.0[start..].to_vec()))
	}

	/// Produces a presentation-only string, replacing invalid UTF-8 sequences.
	#[must_use]
	pub fn to_string_lossy(&self) -> String { String::from_utf8_lossy(&self.0).into_owned() }
}

impl From<Vec<u8>> for NativePath {
	fn from(bytes: Vec<u8>) -> Self { Self(bytes) }
}

impl From<&[u8]> for NativePath {
	fn from(bytes: &[u8]) -> Self { Self(bytes.to_vec()) }
}

impl From<PathBuf> for NativePath {
	fn from(path: PathBuf) -> Self { Self(path.into_os_string().into_vec()) }
}

impl From<&Path> for NativePath {
	fn from(path: &Path) -> Self { Self(path.as_os_str().as_bytes().to_vec()) }
}

impl From<OsString> for NativePath {
	fn from(value: OsString) -> Self { Self(value.into_vec()) }
}

impl From<&OsStr> for NativePath {
	fn from(value: &OsStr) -> Self { Self(value.as_bytes().to_vec()) }
}

impl From<NativePath> for PathBuf {
	fn from(path: NativePath) -> Self { Self::from(OsString::from_vec(path.0)) }
}

impl AsRef<Self> for NativePath {
	fn as_ref(&self) -> &Self { self }
}

impl AsRef<Path> for NativePath {
	fn as_ref(&self) -> &Path { self.as_path() }
}

impl Borrow<Path> for NativePath {
	fn borrow(&self) -> &Path { self.as_path() }
}

impl fmt::Display for NativePath {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&String::from_utf8_lossy(&self.0))
	}
}

#[cfg(test)]
mod tests {
	use super::NativePath;

	#[test]
	fn normalizes_without_touching_the_filesystem() {
		assert_eq!(NativePath::from(&b"/a//./b/../c/"[..]).normalize().as_bytes(), b"/a/c");
		assert_eq!(NativePath::from(&b"../../a/../b"[..]).normalize().as_bytes(), b"../../b");
		assert_eq!(NativePath::from(&b"/../../a"[..]).normalize().as_bytes(), b"/a");
		assert!(NativePath::from(&b"a/.."[..]).normalize().is_empty());
	}

	#[test]
	fn preserves_non_utf8_bytes() {
		let path = NativePath::from(&b"/tmp/\xff-name"[..]);
		assert_eq!(NativePath::from(path.as_path()), path);
		assert_eq!(path.file_name().expect("file name").as_bytes(), b"\xff-name");
	}

	#[test]
	fn joins_and_finds_parents_lexically() {
		let base = NativePath::from(&b"/a/b"[..]);
		assert_eq!(base.join(NativePath::from(&b"../c"[..])).as_bytes(), b"/a/c");
		assert_eq!(base.parent().expect("parent").as_bytes(), b"/a");
		assert_eq!(NativePath::from(&b"/a"[..]).parent().expect("root parent").as_bytes(), b"/");
		assert!(NativePath::from(&b"/"[..]).parent().is_none());
		assert!(NativePath::from(&b"name"[..]).parent().is_none());
	}
}
