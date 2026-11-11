# thallium-files

> File manager for Thallium 81: the Qt/Rust replacement for Thunar.

## What it does

Qt/QML file manager on a Rust backend, built to replace Thunar:

- Tabs, live folders, list and grid views, type-to-find, full keyboard control
- Copy, move, rename, bulk rename, links, new files from `~/Templates`, undo
- Cut/copy/paste and drag and drop shared with every other desktop app
- Trash with restore, drives (mount, unmount, eject), extract and compress
- Open with and default applications, properties with permission editing
- GTK bookmarks and Thunar custom actions (`uca.xml`) carry over
- Answers `org.freedesktop.FileManager1`, so "Show in folder" opens it

## Quick Start

```sh
cargo run -p thallium-backend --features qt -- ~/Downloads
```

Backend crates only, no Qt needed:

```sh
cargo test --workspace
```

## Installation

CI builds a Debian trixie package on every pull request and push to `main`
(the `thallium-files-deb` artifact of the `Debian package` job):

```sh
sudo apt install ./thallium-files_*_amd64.deb
```

To build it yourself: `bash packaging/build-deb.sh` (release build; slow).
Mounting drives needs `udisks2`; formats other than zip and tar need
`libarchive-tools`. Make it the default from the folder menu, or:

```sh
xdg-mime default io.thallium.Files.desktop inode/directory
```

## Usage

`thallium-file-manager [PATH | file://URI | trash:///]` opens a folder, or the
folder holding a file with that file selected.

| Keys | |
|---|---|
| Ctrl+T / Ctrl+W / Ctrl+Tab | New, close, next tab |
| F2 | Rename (bulk rename with several held) |
| Ctrl+X / C / V, Ctrl+Z | Clipboard, undo |
| Delete / Shift+Delete | Trash / delete permanently |
| Ctrl+L, Ctrl+F, Ctrl+H | Type a path, search, hidden files |
| Alt+Enter, Shift+F10 | Properties, context menu |
| Ctrl+Shift+N, Ctrl+D | New folder, bookmark folder |

## Known gaps

- No split view yet; it lands with the UI rework.
- Remotes need `rclone` and have no trash, links or mode bits.
- Video thumbnails need `ffmpegthumbnailer`; SVGs keep their type icon.

## Architecture

```
thallium-fs-core    types, paths, metadata, deltas, errors
thallium-protocol   frontend-visible DTOs (depends on fs-core)
thallium-vfs        VfsFacade trait + LocalFs backend
thallium-rclone     rclone-backed remotes + local/remote routing
thallium-icons      XDG icon theme lookup + freedesktop thumbnails
thallium-watcher    notify-backed watcher, batching, refresh
thallium-scheduler  task queue, progress, cancellation
thallium-state      browser state reducer + history
thallium-actions    command validation, dispatch, feedback pump
thallium-backend    cxx-qt bridge, QML module, binary
```

The `qt` feature is off by default so the backend crates build and test
without a Qt toolchain. Building with it needs Rust 1.95+ and the Qt 6
development packages.

## License

GPL-3.0-or-later.
