//! `file://` URIs as the desktop clipboard and drag-and-drop carry them.
//!
//! Byte-exact both ways: a name that is not valid UTF-8 still round-trips.

use std::fmt::Write;

/// Percent-encode a local path into a `file://` URI.
pub fn file_uri(path: &[u8]) -> String {
	let mut uri = String::from("file://");
	for &byte in path {
		if byte.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&byte) {
			uri.push(byte as char);
		} else {
			let _ = write!(uri, "%{byte:02X}");
		}
	}
	uri
}

/// The local path a `file://` URI names. Other schemes, and URIs naming
/// another host, are not local and yield `None`.
pub fn file_path(uri: &str) -> Option<Vec<u8>> {
	let rest = uri.trim().strip_prefix("file://")?;
	let path = match rest.strip_prefix("localhost") {
		Some(path) => path,
		None => rest,
	};
	if !path.starts_with('/') {
		return None;
	}
	let bytes = path.as_bytes();
	let mut out = Vec::with_capacity(bytes.len());
	let mut at = 0;
	while at < bytes.len() {
		if bytes[at] == b'%'
			&& let Some(pair) = bytes.get(at + 1..at + 3)
			&& let Ok(hex) = std::str::from_utf8(pair)
			&& let Ok(byte) = u8::from_str_radix(hex, 16)
		{
			out.push(byte);
			at += 3;
			continue;
		}
		out.push(bytes[at]);
		at += 1;
	}
	Some(out)
}

#[cfg(test)]
mod tests {
	use super::{file_path, file_uri};

	#[test]
	fn paths_round_trip_through_uris() {
		for path in [&b"/home/a b/r\xc3\xa9sum\xc3\xa9.txt"[..], b"/tmp/100%/#hash?", b"/bad/\xff\xfe"]
		{
			assert_eq!(file_path(&file_uri(path)).as_deref(), Some(path));
		}
		assert_eq!(file_uri(b"/a b"), "file:///a%20b");
	}

	#[test]
	fn only_local_file_uris_are_paths() {
		assert_eq!(file_path("file:///x").as_deref(), Some(&b"/x"[..]));
		assert_eq!(file_path("file://localhost/x").as_deref(), Some(&b"/x"[..]));
		assert_eq!(file_path("file://server/x"), None);
		assert_eq!(file_path("https://example.com/x"), None);
	}
}
