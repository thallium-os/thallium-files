use std::os::unix::fs::{MetadataExt, PermissionsExt};

use tempfile::TempDir;
use thallium_fs_core::{BackendId, ConflictPolicy, FileAttrs, FileKind, FsErrorCode, Location, NativePath};
use thallium_vfs::{CreateDirOptions, DeleteOptions, DirectoryReader, FileSystem, LocalBackend, MoveOptions, OpenWriteOptions, StagedCopyOptions, StatOptions, local::LocalDirectoryReader};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn location(path: impl AsRef<std::path::Path>) -> Location {
	Location::local(NativePath::from(path.as_ref()))
}

fn assert_send<T: Send>(_: T) {}

fn assert_filesystem_futures_are_send<B: FileSystem>(
	backend: &B,
	source: &Location,
	destination: &Location,
) {
	assert_send(backend.scan(source));
	assert_send(backend.read_dir(source));
	assert_send(backend.stat(source, StatOptions::default()));
	assert_send(backend.read_link(source));
	assert_send(backend.open_read(source));
	assert_send(backend.open_write(source, OpenWriteOptions::default()));
	assert_send(backend.create_dir(source, CreateDirOptions::default()));
	assert_send(backend.create_temp(destination));
	assert_send(backend.set_attrs(source, &FileAttrs::default()));
	assert_send(backend.symlink(&source.path, destination));
	assert_send(backend.hard_link(source, destination));
	assert_send(backend.rename(source, destination));
	assert_send(backend.move_entry(source, destination, MoveOptions::default()));
	assert_send(backend.trash(source));
	assert_send(backend.delete(source, DeleteOptions::default()));
	assert_send(backend.staged_copy(source, destination, StagedCopyOptions::default()));
}

fn assert_directory_future_is_send<R: DirectoryReader>(reader: &mut R) {
	assert_send(reader.next_entry());
}

#[test]
fn public_async_contracts_guarantee_send_futures() {
	let backend = LocalBackend::new();
	let source = Location::local(NativePath::from(&b"/not-polled-source"[..]));
	let destination = Location::local(NativePath::from(&b"/not-polled-destination"[..]));
	assert_filesystem_futures_are_send(&backend, &source, &destination);
	let _ = assert_directory_future_is_send::<LocalDirectoryReader>;
}

#[tokio::test]
async fn creates_scans_opens_and_stats_entries() {
	let fixture = TempDir::new().expect("temporary directory");
	let backend = LocalBackend::new();
	let root = location(fixture.path());
	let nested = location(fixture.path().join("nested/child"));
	backend
		.create_dir(&nested, CreateDirOptions { recursive: true, attributes: None })
		.await
		.expect("create nested directories");

	let file = location(fixture.path().join("file.bin"));
	let mut writer = backend
		.open_write(&file, OpenWriteOptions {
			create_new: true,
			attributes: Some(FileAttrs { permissions: Some(0o640), ..FileAttrs::default() }),
			..OpenWriteOptions::default()
		})
		.await
		.expect("create file");
	writer.write_all(b"thallium").await.expect("write file");
	writer.shutdown().await.expect("flush file");
	drop(writer);

	let entry = backend.stat(&file, StatOptions::default()).await.expect("stat file");
	assert_eq!(entry.metadata.kind, FileKind::File);
	assert_eq!(entry.metadata.size, 8);
	assert_eq!(entry.metadata.permissions & 0o777, 0o640);

	let mut reader = backend.open_read(&file).await.expect("open file");
	let mut contents = Vec::new();
	reader.read_to_end(&mut contents).await.expect("read file");
	assert_eq!(contents, b"thallium");

	let entries = backend.scan(&root).await.expect("scan directory");
	assert_eq!(entries.len(), 2);
	assert_eq!(entries[0].name.as_bytes(), b"file.bin");
	assert_eq!(entries[1].name.as_bytes(), b"nested");

	let mut directory = backend.read_dir(&root).await.expect("open directory reader");
	let mut streamed = 0;
	while directory.next_entry().await.expect("read entry").is_some() {
		streamed += 1;
	}
	assert_eq!(streamed, 2);
}

#[tokio::test]
async fn copies_moves_links_and_deletes_entries() {
	let fixture = TempDir::new().expect("temporary directory");
	let backend = LocalBackend::new();
	let source = location(fixture.path().join("source"));
	tokio::fs::write(source.path.as_path(), b"payload").await.expect("write source");

	let copied = location(fixture.path().join("copied"));
	let copy = backend
		.staged_copy(&source, &copied, StagedCopyOptions {
			conflict_policy: ConflictPolicy::Overwrite,
			attributes:      None,
			preserve_sparse: false,
		})
		.await
		.expect("copy file");
	assert!(copy.published);
	assert_eq!(copy.bytes_copied, 7);
	assert_eq!(tokio::fs::read(copied.path.as_path()).await.expect("read copy"), b"payload");
	assert!(!fixture.path().read_dir().expect("read fixture").any(|entry| {
		entry.expect("fixture entry").file_name().to_string_lossy().starts_with(".thallium-")
	}));

	let hard_link = location(fixture.path().join("hard-link"));
	backend.hard_link(&source, &hard_link).await.expect("create hard link");
	let source_stat = backend.stat(&source, StatOptions::default()).await.expect("stat source");
	let hard_link_stat = backend.stat(&hard_link, StatOptions::default()).await.expect("stat link");
	assert_eq!(source_stat.metadata.inode, hard_link_stat.metadata.inode);

	let symbolic_link = location(fixture.path().join("symbolic-link"));
	backend.symlink(&source.path, &symbolic_link).await.expect("create symbolic link");
	assert_eq!(backend.read_link(&symbolic_link).await.expect("read link"), source.path);
	assert_eq!(
		backend.stat(&symbolic_link, StatOptions::default()).await.expect("lstat link").metadata.kind,
		FileKind::Symlink
	);

	let moved = location(fixture.path().join("moved"));
	let outcome = backend
		.move_entry(&copied, &moved, MoveOptions {
			copy:   StagedCopyOptions {
				conflict_policy: ConflictPolicy::Ask,
				attributes:      None,
				preserve_sparse: false,
			},
			delete: DeleteOptions::default(),
		})
		.await
		.expect("move file");
	assert!(!outcome.copied);
	assert!(outcome.published);
	assert!(!tokio::fs::try_exists(copied.path.as_path()).await.expect("copied absent"));
	assert_eq!(tokio::fs::read(moved.path.as_path()).await.expect("read moved"), b"payload");

	for entry in [&symbolic_link, &hard_link, &moved, &source] {
		backend.delete(entry, DeleteOptions::default()).await.expect("delete entry");
	}
	assert!(fixture.path().read_dir().expect("read fixture").next().is_none());
}

#[tokio::test]
async fn conflict_policies_do_not_expose_partial_files() {
	let fixture = TempDir::new().expect("temporary directory");
	let backend = LocalBackend::new();
	let source = location(fixture.path().join("source"));
	let destination = location(fixture.path().join("destination"));
	tokio::fs::write(source.path.as_path(), b"new").await.expect("write source");
	tokio::fs::write(destination.path.as_path(), b"old").await.expect("write destination");

	let error = backend
		.staged_copy(&source, &destination, StagedCopyOptions {
			conflict_policy: ConflictPolicy::Ask,
			attributes:      None,
			preserve_sparse: false,
		})
		.await
		.expect_err("ask requires resolution");
	assert_eq!(error.code, FsErrorCode::AlreadyExists);
	assert_eq!(tokio::fs::read(destination.path.as_path()).await.expect("read old"), b"old");

	let skipped = backend
		.staged_copy(&source, &destination, StagedCopyOptions {
			conflict_policy: ConflictPolicy::Skip,
			attributes:      None,
			preserve_sparse: false,
		})
		.await
		.expect("skip conflict");
	assert!(!skipped.published);
	assert_eq!(tokio::fs::read(destination.path.as_path()).await.expect("read old"), b"old");

	let renamed = backend
		.staged_copy(&source, &destination, StagedCopyOptions {
			conflict_policy: ConflictPolicy::Rename,
			attributes:      None,
			preserve_sparse: false,
		})
		.await
		.expect("rename conflict");
	assert_ne!(renamed.destination, destination);
	assert_eq!(
		tokio::fs::read(renamed.destination.path.as_path()).await.expect("read renamed"),
		b"new"
	);
}

#[tokio::test]
async fn falls_back_to_copy_and_delete_across_filesystems_when_available() {
	let source_fixture = TempDir::new().expect("source temporary directory");
	let Ok(destination_fixture) = TempDir::new_in("/dev/shm") else {
		return;
	};
	if std::fs::metadata(source_fixture.path()).expect("source metadata").dev()
		== std::fs::metadata(destination_fixture.path()).expect("destination metadata").dev()
	{
		return;
	}

	let backend = LocalBackend::new();
	let source = location(source_fixture.path().join("source"));
	let destination = location(destination_fixture.path().join("destination"));
	tokio::fs::write(source.path.as_path(), b"cross-device").await.expect("write source");
	let outcome = backend
		.move_entry(&source, &destination, MoveOptions {
			copy:   StagedCopyOptions {
				conflict_policy: ConflictPolicy::Overwrite,
				attributes:      None,
				preserve_sparse: false,
			},
			delete: DeleteOptions::default(),
		})
		.await
		.expect("cross-device move");
	assert!(outcome.copied);
	assert!(outcome.published);
	assert!(!tokio::fs::try_exists(source.path.as_path()).await.expect("source removed"));
	assert_eq!(
		tokio::fs::read(destination.path.as_path()).await.expect("read destination"),
		b"cross-device"
	);
}

#[tokio::test]
async fn rejects_implicit_and_foreign_locations_and_protects_root() {
	let backend = LocalBackend::new();
	let relative = Location::local(NativePath::from(&b"relative"[..]));
	let foreign = Location::new(BackendId(9), NativePath::from(&b"/tmp"[..]));

	assert_eq!(
		backend.stat(&relative, StatOptions::default()).await.expect_err("reject relative").code,
		FsErrorCode::InvalidInput
	);
	assert_eq!(
		backend.stat(&foreign, StatOptions::default()).await.expect_err("reject backend").code,
		FsErrorCode::Unsupported
	);
	assert_eq!(
		backend
			.delete(&Location::local(NativePath::from(&b"/"[..])), DeleteOptions { recursive: true },)
			.await
			.expect_err("protect root")
			.code,
		FsErrorCode::InvalidInput
	);
	assert_eq!(
		std::fs::metadata("/").expect("root metadata").permissions().mode() & 0o170000,
		0o040000
	);
}

#[test]
fn advertises_linux_mvp_capabilities() {
	let capabilities = LocalBackend::new().capabilities();
	assert!(capabilities.atomic_rename);
	assert!(capabilities.trash);
	assert!(capabilities.symlink);
	assert!(capabilities.hard_link);
	assert!(!capabilities.sparse_files);
	assert!(!capabilities.progressive_copy);
}
