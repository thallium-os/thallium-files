# thallium-icons

Type icons and thumbnails for directory entries, resolved against the
desktop's own icon theme and thumbnail cache rather than a bundled set.

## Boundary invariants

- This crate returns filesystem paths. It never loads, decodes for display,
  scales or tints anything for the frontend, and it does not call Qt.
- Icon lookup follows the XDG icon theme specification through the user's
  configured theme, then `Adwaita` and `breeze`, then the mandatory `hicolor`
  fallback. Symbolic variants are preferred so the frontend can tint them.
- MIME resolution is name-based first. The content sniff that reads the head
  of a file runs only for names `shared-mime-info` cannot place, so listing a
  directory does not read every file in it.
- Thumbnails follow the freedesktop thumbnail managing standard: MD5 of the
  percent-encoded `file://` URI names the file, and `Thumb::URI` and
  `Thumb::MTime` are written into the PNG. Thumbnails other applications have
  already made are reused, including the 256px `large` bucket, and the ones
  written here are theirs to reuse.
- Generation is off the caller's thread and never blocks it. Every lookup
  answers immediately with `Ready`, `Pending` or `Unavailable`, and every
  answer is remembered, because the frontend asks again on every repaint.
- Video thumbnails require `ffmpegthumbnailer` on `PATH`. Without it, video
  files resolve to their type icon and nothing fails. Vector images are not
  rasterised here and resolve to their type icon for the same reason.
