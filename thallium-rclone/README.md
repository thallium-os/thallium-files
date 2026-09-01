# thallium-rclone

Remote filesystems — Google Drive, OneDrive, Dropbox, S3, SFTP, WebDAV and
everything else rclone speaks — behind the same VFS contract as the local disk.

## Boundary invariants

- Nothing here is provider-specific. A remote the user adds with
  `rclone config` is available the next time the application starts; no code in
  this repository names a cloud service.
- Credentials are rclone's. This crate never reads, stores or forwards a token;
  it starts `rclone rcd` and lets rclone use the configuration it already owns.
- The daemon listens on loopback with a fresh 256-bit password per run, so
  another local process cannot drive the user's accounts through it. It is
  killed when the application exits.
- `BackendId(0)` is the local filesystem. Every configured remote is assigned
  the next identity in listing order, and `Location` finally means something
  it has always been able to express.
- Work happens as close to the data as possible. A copy inside one remote is a
  server-side `operations/copyfile`; bytes only pass through this process when
  the two ends are on different backends.
- Reads stream from the daemon's object server and writes stream into its
  upload endpoint. Neither direction stages a whole file on local disk.
- What a remote cannot do, it refuses rather than emulating: no trash, no
  symbolic links, no hard links, no mode or ownership. `set_attrs` is the one
  exception, and it succeeds while recording nothing, because failing it would
  fail every copy that preserves attributes.
- Remotes are configured from the interface, not the terminal. rclone's own
  question-and-answer loop is driven non-interactively, so browser consent for
  Google Drive and a password for SFTP arrive through the same states and a
  provider rclone adds later needs no code here. `all` must be set or rclone
  writes the remote with defaults and asks nothing; the remote exists from the
  first call, so an abandoned conversation is deleted rather than dropped.
- `RoutedFs` is the only type that reads `Location::backend`. The scheduler and
  coordinator stay generic over a single `FileSystem`, exactly as before.

## Requirements

`rclone` on `PATH`. Remotes are added from the sidebar, so none need to exist
beforehand. Without rclone the router degrades to the local filesystem, the
browser behaves as it always has, and the sidebar offers nothing to configure.
