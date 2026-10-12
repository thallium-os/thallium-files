//! End-to-end coverage of the assembled service graph.
//!
//! The unit tests inside each crate exercise one layer in isolation. These
//! drive a real `Backend` over a real temporary directory, so a command has to
//! travel coordinator -> state -> scheduler -> VFS and come back as an
//! observable snapshot before a test passes.

use std::{fs, path::Path, thread, time::{Duration, Instant}};

use tempfile::TempDir;
use thallium_actions::Command;
use thallium_backend::{Backend, BackendConfig};
use thallium_fs_core::{ConflictPolicy, Location, NativePath};
use thallium_protocol::{TaskInfo, TaskKind, TaskStatus};

const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);

fn at(path: &Path) -> Location { Location::local(NativePath::from(path)) }

fn start(root: &Path) -> Backend {
	Backend::start(BackendConfig {
		initial_location: Some(NativePath::from(root)),
		..BackendConfig::default()
	})
	.expect("start backend")
}

/// Poll until `probe` yields a value, draining events so the bounded queue
/// never masks progress. Returns the value or fails the test.
fn wait_for<T>(
	backend: &mut Backend,
	label: &str,
	mut probe: impl FnMut(&Backend) -> Option<T>,
) -> T {
	let deadline = Instant::now() + SETTLE_TIMEOUT;
	loop {
		while matches!(backend.next_event(), Ok(Some(_))) {}
		if let Some(value) = probe(backend) {
			return value;
		}
		assert!(Instant::now() < deadline, "timed out waiting for {label}");
		thread::sleep(Duration::from_millis(20));
	}
}

/// Wait for exactly one task of `kind` to stop changing.
fn settle_task(backend: &mut Backend, kind: TaskKind) -> TaskInfo {
	wait_for(backend, "a terminal task", |backend| {
		backend
			.snapshot()
			.expect("snapshot")
			.tasks
			.into_iter()
			.find(|task| task.kind == kind && task.status.is_terminal())
	})
}

fn names(backend: &Backend) -> Vec<Vec<u8>> {
	backend
		.snapshot()
		.expect("snapshot")
		.directory
		.entries
		.into_iter()
		.map(|e| e.name.bytes)
		.collect()
}

#[test]
fn copy_lands_the_file_and_leaves_the_source() {
	let root = TempDir::new().expect("root");
	let source = root.path().join("source.txt");
	let target = root.path().join("target");
	fs::write(&source, b"payload").expect("seed");
	fs::create_dir(&target).expect("seed target");

	let mut backend = start(root.path());
	backend
		.submit(Command::Copy {
			sources:         vec![at(&source)],
			destination:     at(&target),
			conflict_policy: ConflictPolicy::Overwrite,
		})
		.expect("submit copy");

	let task = settle_task(&mut backend, TaskKind::Copy);
	assert_eq!(task.status, TaskStatus::Succeeded, "copy failed: {:?}", task.issue);
	assert_eq!(fs::read(target.join("source.txt")).expect("copied file"), b"payload");
	assert!(source.exists(), "copy must not consume the source");
	backend.shutdown().expect("shutdown");
}

#[test]
fn copy_reproduces_a_directory_tree() {
	let root = TempDir::new().expect("root");
	let tree = root.path().join("tree");
	let target = root.path().join("target");
	fs::create_dir_all(tree.join("nested")).expect("seed tree");
	fs::write(tree.join("nested/leaf.txt"), b"leaf").expect("seed leaf");
	fs::create_dir(&target).expect("seed target");

	let mut backend = start(root.path());
	backend
		.submit(Command::Copy {
			sources:         vec![at(&tree)],
			destination:     at(&target),
			conflict_policy: ConflictPolicy::Overwrite,
		})
		.expect("submit copy");

	let task = settle_task(&mut backend, TaskKind::Copy);
	assert_eq!(task.status, TaskStatus::Succeeded);
	assert_eq!(fs::read(target.join("tree/nested/leaf.txt")).expect("copied leaf"), b"leaf");
	assert!(tree.join("nested/leaf.txt").exists(), "copy must not consume the source");
	backend.shutdown().expect("shutdown");
}

#[test]
fn copy_with_rename_policy_keeps_the_existing_file_intact() {
	let root = TempDir::new().expect("root");
	let source = root.path().join("source.txt");
	let target = root.path().join("target");
	fs::write(&source, b"new").expect("seed");
	fs::create_dir(&target).expect("seed target");
	fs::write(target.join("source.txt"), b"original").expect("seed conflict");

	let mut backend = start(root.path());
	backend
		.submit(Command::Copy {
			sources:         vec![at(&source)],
			destination:     at(&target),
			conflict_policy: ConflictPolicy::Rename,
		})
		.expect("submit copy");

	let task = settle_task(&mut backend, TaskKind::Copy);
	assert_eq!(task.status, TaskStatus::Succeeded, "copy failed: {:?}", task.issue);
	assert_eq!(
		fs::read(target.join("source.txt")).expect("existing file"),
		b"original",
		"Rename must not overwrite the conflicting destination"
	);
	let copies = fs::read_dir(&target).expect("read target").count();
	assert_eq!(copies, 2, "Rename must add a second, non-conflicting entry");
	backend.shutdown().expect("shutdown");
}

#[test]
fn move_removes_the_source() {
	let root = TempDir::new().expect("root");
	let source = root.path().join("source.txt");
	let target = root.path().join("target");
	fs::write(&source, b"payload").expect("seed");
	fs::create_dir(&target).expect("seed target");

	let mut backend = start(root.path());
	backend
		.submit(Command::Move {
			sources:         vec![at(&source)],
			destination:     at(&target),
			conflict_policy: ConflictPolicy::Overwrite,
		})
		.expect("submit move");

	let task = settle_task(&mut backend, TaskKind::Move);
	assert_eq!(task.status, TaskStatus::Succeeded, "move failed: {:?}", task.issue);
	assert_eq!(fs::read(target.join("source.txt")).expect("moved file"), b"payload");
	assert!(!source.exists(), "move must consume the source");
	backend.shutdown().expect("shutdown");
}

#[test]
fn delete_removes_the_target_permanently() {
	let root = TempDir::new().expect("root");
	let doomed = root.path().join("doomed.txt");
	fs::write(&doomed, b"payload").expect("seed");

	let mut backend = start(root.path());
	backend.submit(Command::Delete { targets: vec![at(&doomed)] }).expect("submit delete");

	let task = settle_task(&mut backend, TaskKind::Delete);
	assert_eq!(task.status, TaskStatus::Succeeded, "delete failed: {:?}", task.issue);
	assert!(!doomed.exists());
	backend.shutdown().expect("shutdown");
}

#[test]
fn delete_recurses_into_a_directory_tree() {
	let root = TempDir::new().expect("root");
	let tree = root.path().join("tree");
	fs::create_dir_all(tree.join("nested")).expect("seed tree");
	fs::write(tree.join("nested/leaf.txt"), b"leaf").expect("seed leaf");

	let mut backend = start(root.path());
	backend.submit(Command::Delete { targets: vec![at(&tree)] }).expect("submit delete");

	let task = settle_task(&mut backend, TaskKind::Delete);
	assert_eq!(task.status, TaskStatus::Succeeded, "delete failed: {:?}", task.issue);
	assert!(!tree.exists());
	backend.shutdown().expect("shutdown");
}

#[test]
fn a_failing_operation_surfaces_an_issue_rather_than_hanging() {
	let root = TempDir::new().expect("root");
	let missing = root.path().join("does-not-exist.txt");
	let target = root.path().join("target");
	fs::create_dir(&target).expect("seed target");

	let mut backend = start(root.path());
	backend
		.submit(Command::Copy {
			sources:         vec![at(&missing)],
			destination:     at(&target),
			conflict_policy: ConflictPolicy::Overwrite,
		})
		.expect("submit copy");

	let task = settle_task(&mut backend, TaskKind::Copy);
	assert_eq!(task.status, TaskStatus::Failed);
	assert!(task.issue.is_some(), "a failed task must carry a presentable issue");
	backend.shutdown().expect("shutdown");
}

#[test]
fn navigation_history_walks_back_and_forward() {
	let root = TempDir::new().expect("root");
	let child = root.path().join("child");
	fs::create_dir(&child).expect("seed child");

	let mut backend = start(root.path());
	assert!(!backend.snapshot().expect("snapshot").can_go_back);

	backend.submit(Command::Navigate(at(&child))).expect("navigate");
	let after = backend.snapshot().expect("snapshot");
	assert!(after.can_go_back);
	assert!(!after.can_go_forward);

	backend.submit(Command::NavigateBack).expect("back");
	let back = backend.snapshot().expect("snapshot");
	assert_eq!(
		back.current_location.expect("location").path.bytes,
		root.path().as_os_str().as_encoded_bytes()
	);
	assert!(back.can_go_forward);

	backend.submit(Command::NavigateForward).expect("forward");
	assert_eq!(
		backend.snapshot().expect("snapshot").current_location.expect("location").path.bytes,
		child.as_os_str().as_encoded_bytes()
	);
	backend.shutdown().expect("shutdown");
}

#[test]
fn selection_commands_round_trip_through_state() {
	let root = TempDir::new().expect("root");
	fs::write(root.path().join("a.txt"), b"a").expect("seed a");
	fs::write(root.path().join("b.txt"), b"b").expect("seed b");

	let mut backend = start(root.path());
	let entries = backend.snapshot().expect("snapshot").directory.entries;
	assert_eq!(entries.len(), 2);
	let first = entries[0].id.to_entry_id();

	backend
		.submit(Command::SetSelection { selected: vec![first], focused: Some(first) })
		.expect("select");
	assert_eq!(backend.snapshot().expect("snapshot").selection.len(), 1);

	backend.submit(Command::ToggleSelection(first)).expect("toggle");
	assert!(backend.snapshot().expect("snapshot").selection.is_empty());

	backend
		.submit(Command::SetSelection { selected: vec![first], focused: Some(first) })
		.expect("reselect");
	backend.submit(Command::ClearSelection).expect("clear");
	assert!(backend.snapshot().expect("snapshot").selection.is_empty());
	backend.shutdown().expect("shutdown");
}

#[test]
fn refresh_picks_up_a_file_created_behind_the_backend() {
	let root = TempDir::new().expect("root");
	let mut backend = start(root.path());
	assert!(names(&backend).is_empty());

	fs::write(root.path().join("appeared.txt"), b"payload").expect("seed");
	backend.submit(Command::Refresh).expect("refresh");

	wait_for(&mut backend, "the new entry", |backend| {
		names(backend).iter().any(|name| name == b"appeared.txt").then_some(())
	});
	backend.shutdown().expect("shutdown");
}

#[test]
fn a_completed_task_can_be_dismissed_from_the_task_list() {
	let root = TempDir::new().expect("root");
	let doomed = root.path().join("doomed.txt");
	fs::write(&doomed, b"payload").expect("seed");

	let mut backend = start(root.path());
	backend.submit(Command::Delete { targets: vec![at(&doomed)] }).expect("submit delete");
	let task = settle_task(&mut backend, TaskKind::Delete);

	backend.submit(Command::DismissTask(task.id)).expect("dismiss");
	wait_for(&mut backend, "the task row to disappear", |backend| {
		backend.snapshot().expect("snapshot").tasks.iter().all(|row| row.id != task.id).then_some(())
	});
	backend.shutdown().expect("shutdown");
}

#[test]
fn trash_reaches_a_terminal_state_and_consumes_the_source_when_it_succeeds() {
	// The XDG trash directory is not guaranteed to exist, or to be on the same
	// filesystem as the temporary root, in a container or on CI. The contract
	// asserted here is that the task always terminates and reports honestly --
	// never that trashing is available.
	let root = TempDir::new().expect("root");
	let doomed = root.path().join("doomed.txt");
	fs::write(&doomed, b"payload").expect("seed");

	let mut backend = start(root.path());
	backend.submit(Command::Trash { targets: vec![at(&doomed)] }).expect("submit trash");

	let task = settle_task(&mut backend, TaskKind::Trash);
	match task.status {
		TaskStatus::Succeeded => {
			assert!(!doomed.exists(), "a succeeding trash must consume the source")
		}
		TaskStatus::Failed => assert!(task.issue.is_some(), "a failed trash must explain itself"),
		other => panic!("unexpected terminal trash status: {other:?}"),
	}
	backend.shutdown().expect("shutdown");
}

#[test]
fn shutdown_is_idempotent() {
	let root = TempDir::new().expect("root");
	let mut backend = start(root.path());
	backend.shutdown().expect("first shutdown");
	backend.shutdown().expect("second shutdown must be a no-op");
}
