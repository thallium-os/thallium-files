//! Pure browser-state reducer.
//!
//! The single-owner reducer model is specified by `PHASE2-PLAN.md` sections 1
//! and 4 and derived from the useful ownership properties documented in
//! `WATCHER-STATE-RESEARCH.md` sections 2.2-3.

use std::collections::BTreeSet;

use thallium_fs_core::{EntryId, FsChange, FsDelta, ListingGeneration, Location};
use thallium_protocol::{DirectoryEntryRow, EntryIdDto, LocationDto, StateChange, StateDelta};

use crate::{BrowserState, DirectoryState, StateAction, StateEffect};

/// Complete result of one deterministic state transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Reduction {
	/// Replacement browser state; no mutable borrow escapes this call.
	pub state:   BrowserState,
	/// Contiguous protocol update, absent when an action was rejected or a no-op.
	pub delta:   Option<StateDelta>,
	/// Data-only effects for `thallium-actions` to execute asynchronously.
	pub effects: Vec<StateEffect>,
}

impl Reduction {
	fn unchanged(state: &BrowserState) -> Self {
		Self { state: state.clone(), delta: None, effects: Vec::new() }
	}
}

/// Computes one browser-state transition without performing I/O or rendering.
#[must_use]
pub fn reduce(state: &BrowserState, action: StateAction) -> Reduction {
	match action {
		StateAction::NavigateTo(location) => navigate_to(state, location, HistoryMode::Push),
		StateAction::NavigateBack => navigate_history(state, HistoryDirection::Back),
		StateAction::NavigateForward => navigate_history(state, HistoryDirection::Forward),
		StateAction::DirectoryScanned { location, generation, entries, complete, error } => {
			directory_scanned(state, location, generation, entries, complete, error)
		}
		StateAction::ApplyFsDelta(delta) => apply_fs_delta(state, delta),
		StateAction::SetSelection { selected, focused } => set_selection(state, selected, focused),
		StateAction::ToggleSelection(id) => toggle_selection(state, id),
		StateAction::ClearSelection => clear_selection(state),
		StateAction::UpdateTask(task) => update_task(state, task),
		StateAction::RemoveTask(id) => remove_task(state, id),
	}
}

#[derive(Clone, Copy)]
enum HistoryMode {
	Push,
	AlreadyUpdated,
}

#[derive(Clone, Copy)]
enum HistoryDirection {
	Back,
	Forward,
}

fn navigate_to(state: &BrowserState, location: Location, mode: HistoryMode) -> Reduction {
	let location = location.normalize();
	if state.current_location.as_ref() == Some(&location) {
		return Reduction::unchanged(state);
	}

	let mut next = state.clone();
	if matches!(mode, HistoryMode::Push) {
		if let Some(current) = next.current_location.take() {
			next.history.push_back(current);
		}
		next.history.forward.clear();
	}
	next.current_location = Some(location.clone());
	let generation = next.directory.generation.next();
	next.directory = DirectoryState::loading(location.clone(), generation);
	next.selection = Default::default();

	let changes = vec![
		StateChange::LocationChanged {
			location:       LocationDto::from(&location),
			can_go_back:    next.history.can_go_back(),
			can_go_forward: next.history.can_go_forward(),
		},
		StateChange::DirectoryReplaced(next.directory.snapshot()),
		StateChange::SelectionChanged(Vec::new()),
	];
	finish(state, next, changes, vec![
		StateEffect::ScanDirectory { location: location.clone(), generation },
		StateEffect::WatchLocations { locations: vec![location], generation },
	])
}

fn navigate_history(state: &BrowserState, direction: HistoryDirection) -> Reduction {
	let mut next = state.clone();
	let target = match direction {
		HistoryDirection::Back => next.history.back.pop(),
		HistoryDirection::Forward => next.history.forward.pop(),
	};
	let Some(target) = target else { return Reduction::unchanged(state) };
	if state.current_location.as_ref() == Some(&target) {
		return Reduction::unchanged(state);
	}

	if let Some(current) = next.current_location.clone() {
		match direction {
			HistoryDirection::Back => next.history.push_forward(current),
			HistoryDirection::Forward => next.history.push_back(current),
		}
	}
	navigate_to(&next, target, HistoryMode::AlreadyUpdated)
}

fn directory_scanned(
	state: &BrowserState,
	location: Location,
	generation: ListingGeneration,
	entries: Vec<thallium_fs_core::FileEntry>,
	complete: bool,
	error: Option<thallium_fs_core::FsError>,
) -> Reduction {
	if state.current_location.as_ref() != Some(&location)
		|| state.directory.location.as_ref() != Some(&location)
		|| state.directory.generation != generation
	{
		return Reduction::unchanged(state);
	}

	let mut next = state.clone();
	next.directory.replace(entries, complete, error);
	next.selection.retain(&next.directory.ids());
	if next.directory == state.directory && next.selection == state.selection {
		return Reduction::unchanged(state);
	}

	let mut changes = vec![StateChange::DirectoryReplaced(next.directory.snapshot())];
	if next.selection != state.selection {
		changes.push(selection_change(&next));
	}
	finish(state, next, changes, Vec::new())
}

fn apply_fs_delta(state: &BrowserState, delta: FsDelta) -> Reduction {
	let current_generation = state.directory.generation;
	let generation_advanced =
		delta.generation == current_generation.next() && delta.generation != current_generation;
	if state.current_location.as_ref() != Some(&delta.parent)
		|| state.directory.location.as_ref() != Some(&delta.parent)
		|| delta.generation != current_generation && !generation_advanced
	{
		return Reduction::unchanged(state);
	}

	let mut next = state.clone();
	if generation_advanced {
		next.directory.generation = delta.generation;
	}
	let mut changes = Vec::new();
	match delta.change {
		FsChange::Created(entry) | FsChange::Updated(entry) => {
			if next.directory.entries.get(&entry.id) != Some(&entry) {
				next.directory.upsert((*entry).clone());
				changes.push(StateChange::EntriesUpserted(vec![DirectoryEntryRow::from(&*entry)]));
			}
		}
		FsChange::Deleted { id, .. } => {
			if next.directory.remove(id) {
				changes.push(StateChange::EntriesRemoved(vec![EntryIdDto::from(id)]));
				reconcile_selection(state, &mut next, &mut changes);
			}
		}
		FsChange::Renamed { old_id, entry, .. } => {
			let old_present = next.directory.remove(old_id);
			let replacement_changed = next.directory.entries.get(&entry.id) != Some(&entry);
			if old_present || replacement_changed {
				next.directory.upsert((*entry).clone());
				next.selection.rename(old_id, entry.id);
				if old_present && old_id != entry.id {
					changes.push(StateChange::EntriesRemoved(vec![EntryIdDto::from(old_id)]));
				}
				changes.push(StateChange::EntriesUpserted(vec![DirectoryEntryRow::from(&*entry)]));
				reconcile_selection(state, &mut next, &mut changes);
			}
		}
		FsChange::Reset(entries) => {
			next.directory.replace(entries, true, None);
			next.selection.retain(&next.directory.ids());
			if next.directory != state.directory || next.selection != state.selection {
				changes.push(StateChange::DirectoryReplaced(next.directory.snapshot()));
				if next.selection != state.selection {
					changes.push(selection_change(&next));
				}
			}
		}
		FsChange::Error(error) => {
			if next.directory.error.as_ref() != Some(&error) {
				next.directory.error = Some(error);
				changes.push(StateChange::DirectoryErrorChanged(
					next.directory.error.as_ref().map(thallium_protocol::FsErrorDto::from),
				));
			}
		}
	}
	if generation_advanced {
		changes = vec![StateChange::DirectoryReplaced(next.directory.snapshot())];
		if next.selection != state.selection {
			changes.push(selection_change(&next));
		}
	} else if changes.is_empty() {
		return Reduction::unchanged(state);
	}
	finish(state, next, changes, Vec::new())
}

fn set_selection(
	state: &BrowserState,
	selected: Vec<EntryId>,
	focused: Option<EntryId>,
) -> Reduction {
	let present = state.directory.ids();
	let selected = selected.into_iter().filter(|id| present.contains(id)).collect::<BTreeSet<_>>();
	let focused = focused.filter(|id| present.contains(id));
	if state.selection.selected == selected && state.selection.focused == focused {
		return Reduction::unchanged(state);
	}

	let mut next = state.clone();
	next.selection.selected = selected;
	next.selection.focused = focused;
	let changes = vec![selection_change(&next)];
	finish(state, next, changes, Vec::new())
}

fn toggle_selection(state: &BrowserState, id: EntryId) -> Reduction {
	if !state.directory.entries.contains_key(&id) {
		return Reduction::unchanged(state);
	}
	let mut next = state.clone();
	if !next.selection.selected.remove(&id) {
		next.selection.selected.insert(id);
	}
	next.selection.focused = Some(id);
	let changes = vec![selection_change(&next)];
	finish(state, next, changes, Vec::new())
}

fn clear_selection(state: &BrowserState) -> Reduction {
	if state.selection == Default::default() {
		return Reduction::unchanged(state);
	}
	let mut next = state.clone();
	next.selection = Default::default();
	let changes = vec![selection_change(&next)];
	finish(state, next, changes, Vec::new())
}

fn update_task(state: &BrowserState, task: thallium_protocol::TaskSnapshot) -> Reduction {
	if state.tasks.get(&task.id).is_some_and(|current| current.revision >= task.revision) {
		return Reduction::unchanged(state);
	}
	let mut next = state.clone();
	if !next.tasks.contains_key(&task.id) {
		next.task_order.push(task.id);
	}
	next.tasks.insert(task.id, task.clone());
	finish(state, next, vec![StateChange::TaskUpserted(task)], Vec::new())
}

fn remove_task(state: &BrowserState, id: thallium_protocol::TaskId) -> Reduction {
	let mut next = state.clone();
	if next.tasks.remove(&id).is_none() {
		return Reduction::unchanged(state);
	}
	next.task_order.retain(|candidate| *candidate != id);
	finish(state, next, vec![StateChange::TaskRemoved(id)], Vec::new())
}

fn reconcile_selection(
	state: &BrowserState,
	next: &mut BrowserState,
	changes: &mut Vec<StateChange>,
) {
	next.selection.retain(&next.directory.ids());
	if next.selection != state.selection {
		changes.push(selection_change(next));
	}
}

fn selection_change(state: &BrowserState) -> StateChange {
	StateChange::SelectionChanged(
		state.selection.selected.iter().copied().map(EntryIdDto::from).collect(),
	)
}

fn finish(
	previous: &BrowserState,
	mut state: BrowserState,
	changes: Vec<StateChange>,
	effects: Vec<StateEffect>,
) -> Reduction {
	debug_assert!(!changes.is_empty());
	state.revision = previous.revision.next();
	let delta =
		Some(StateDelta { from_revision: previous.revision, revision: state.revision, changes });
	Reduction { state, delta, effects }
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{EntryMetadata, FileEntry, FileKind, FsChange, FsDelta, FsError, FsErrorCode, ListingGeneration, Location, NativePath, StateRevision};
	use thallium_protocol::{StateChange, TaskId, TaskInfo, TaskKind, TaskProgress, TaskRevision, TaskStatus};

	use super::reduce;
	use crate::{BrowserState, StateAction, StateEffect};

	fn location(path: &[u8]) -> Location { Location::local(NativePath::from(path)) }

	fn entry(parent: &[u8], name: &[u8], inode: u64) -> FileEntry {
		let location = location(parent).join(NativePath::from(name));
		FileEntry::new(
			location,
			EntryMetadata {
				kind: FileKind::File,
				size: inode,
				permissions: 0o100644,
				uid: 1000,
				gid: 1000,
				accessed: None,
				created: None,
				changed: None,
				modified: None,
				device: 1,
				inode,
				link_count: 1,
			},
			None,
		)
	}

	fn loaded(entries: Vec<FileEntry>) -> BrowserState {
		let navigated = reduce(&BrowserState::default(), StateAction::NavigateTo(location(b"/tmp")));
		reduce(&navigated.state, StateAction::DirectoryScanned {
			location: location(b"/tmp"),
			generation: ListingGeneration(1),
			entries,
			complete: true,
			error: None,
		})
		.state
	}

	fn task(id: u64, revision: u64, title: &str) -> TaskInfo {
		TaskInfo {
			id:            TaskId(id),
			revision:      TaskRevision(revision),
			kind:          TaskKind::Copy,
			status:        TaskStatus::Started,
			title:         title.into(),
			progress:      TaskProgress::default(),
			current_item:  None,
			issue:         None,
			created_at_ms: id,
			updated_at_ms: revision,
		}
	}

	#[test]
	fn navigation_updates_history_and_emits_scan_and_watch_effects() {
		let initial = reduce(&BrowserState::default(), StateAction::NavigateTo(location(b"/a")));
		assert_eq!(initial.state.current_location, Some(location(b"/a")));
		assert_eq!(initial.state.revision, StateRevision(1));
		assert_eq!(initial.state.directory.generation, ListingGeneration(1));
		assert_eq!(initial.effects.len(), 2);
		assert!(matches!(initial.effects[0], StateEffect::ScanDirectory { .. }));

		let second = reduce(&initial.state, StateAction::NavigateTo(location(b"/b")));
		assert_eq!(second.state.history.back, vec![location(b"/a")]);
		assert!(second.state.history.forward.is_empty());

		let back = reduce(&second.state, StateAction::NavigateBack);
		assert_eq!(back.state.current_location, Some(location(b"/a")));
		assert_eq!(back.state.history.forward, vec![location(b"/b")]);

		let forward = reduce(&back.state, StateAction::NavigateForward);
		assert_eq!(forward.state.current_location, Some(location(b"/b")));
		assert_eq!(forward.state.history.back, vec![location(b"/a")]);
	}

	#[test]
	fn unavailable_or_same_navigation_is_a_revision_preserving_noop() {
		let state = BrowserState::default();
		assert!(reduce(&state, StateAction::NavigateBack).delta.is_none());
		let state = reduce(&state, StateAction::NavigateTo(location(b"/a/./"))).state;
		let result = reduce(&state, StateAction::NavigateTo(location(b"/a")));
		assert_eq!(result.state.revision, state.revision);
		assert!(result.delta.is_none());
	}

	#[test]
	fn scans_are_sorted_deduplicated_and_reject_stale_generations() {
		let first = entry(b"/tmp", b"z", 1);
		let duplicate = entry(b"/tmp", b"z", 1);
		let second = entry(b"/tmp", b"a", 2);
		let state = loaded(vec![first, duplicate, second]);
		let names = state
			.directory
			.ordered_entries()
			.map(|entry| entry.name.as_bytes().to_vec())
			.collect::<Vec<_>>();
		assert_eq!(names, vec![b"a".to_vec(), b"z".to_vec()]);

		let stale = reduce(&state, StateAction::DirectoryScanned {
			location:   location(b"/tmp"),
			generation: ListingGeneration(0),
			entries:    Vec::new(),
			complete:   true,
			error:      None,
		});
		assert_eq!(stale.state, state);
		assert!(stale.delta.is_none());
	}

	#[test]
	fn create_update_delete_reset_and_error_deltas_reduce_exact_generation() {
		let original = entry(b"/tmp", b"a", 1);
		let mut state = loaded(vec![original.clone()]);
		let created = entry(b"/tmp", b"b", 2);
		state = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Created(Box::new(created.clone())),
			}),
		)
		.state;
		assert!(state.directory.entries.contains_key(&created.id));

		let mut updated = created.clone();
		updated.metadata.size = 99;
		state = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Updated(Box::new(updated.clone())),
			}),
		)
		.state;
		assert_eq!(state.directory.entries[&created.id].metadata.size, 99);

		state = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Deleted {
					id:       original.id,
					location: original.location.clone(),
				},
			}),
		)
		.state;
		assert!(!state.directory.entries.contains_key(&original.id));

		let error = FsError::new(FsErrorCode::PermissionDenied, "denied", None);
		state = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Error(error.clone()),
			}),
		)
		.state;
		assert_eq!(state.directory.error, Some(error));

		let reset = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Reset(vec![original]),
			}),
		);
		assert_eq!(reset.state.directory.entries.len(), 1);
		assert!(reset.state.directory.error.is_none());

		let skipped = reduce(
			&reset.state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(3),
				change:     FsChange::Reset(Vec::new()),
			}),
		);
		assert!(skipped.delta.is_none());
	}

	#[test]
	fn watcher_batch_advances_once_then_accepts_more_deltas_at_that_generation() {
		let original = entry(b"/tmp", b"old", 1);
		let created = entry(b"/tmp", b"new", 2);
		let initial = loaded(vec![original.clone()]);

		let first = reduce(
			&initial,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(2),
				change:     FsChange::Created(Box::new(created.clone())),
			}),
		);
		assert_eq!(first.state.directory.generation, ListingGeneration(2));
		assert!(first.state.directory.entries.contains_key(&created.id));
		assert!(matches!(first.delta.as_ref().expect("first batch delta").changes.as_slice(), [
			StateChange::DirectoryReplaced(_)
		]));

		let second = reduce(
			&first.state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(2),
				change:     FsChange::Deleted { id: original.id, location: original.location },
			}),
		);
		assert_eq!(second.state.directory.generation, ListingGeneration(2));
		assert!(!second.state.directory.entries.contains_key(&original.id));
		assert_eq!(second.state.revision, first.state.revision.next());
		assert!(matches!(second.delta.as_ref().expect("second batch delta").changes.as_slice(), [
			StateChange::EntriesRemoved(_)
		]));
	}

	#[test]
	fn watcher_delta_rejects_stale_and_skipped_generations() {
		let original = entry(b"/tmp", b"old", 1);
		let advanced = reduce(
			&loaded(vec![original.clone()]),
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(2),
				change:     FsChange::Updated(Box::new(original)),
			}),
		)
		.state;
		assert_eq!(advanced.directory.generation, ListingGeneration(2));

		for rejected_generation in [ListingGeneration(1), ListingGeneration(4)] {
			let rejected = reduce(
				&advanced,
				StateAction::ApplyFsDelta(FsDelta {
					parent:     location(b"/tmp"),
					generation: rejected_generation,
					change:     FsChange::Reset(Vec::new()),
				}),
			);
			assert_eq!(rejected.state, advanced);
			assert!(rejected.delta.is_none());
		}
	}

	#[test]
	fn deletion_and_reset_clean_selection_while_rename_preserves_it() {
		let old = entry(b"/tmp", b"old", 1);
		let keep = entry(b"/tmp", b"keep", 2);
		let mut state = loaded(vec![old.clone(), keep.clone()]);
		state = reduce(&state, StateAction::SetSelection {
			selected: vec![old.id, keep.id],
			focused:  Some(old.id),
		})
		.state;

		let renamed = entry(b"/tmp", b"new", 1);
		state = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Renamed {
					old_id: old.id,
					from:   old.location,
					entry:  Box::new(renamed.clone()),
				},
			}),
		)
		.state;
		assert!(state.selection.selected.contains(&renamed.id));
		assert_eq!(state.selection.focused, Some(renamed.id));

		state = reduce(
			&state,
			StateAction::ApplyFsDelta(FsDelta {
				parent:     location(b"/tmp"),
				generation: ListingGeneration(1),
				change:     FsChange::Reset(vec![keep]),
			}),
		)
		.state;
		assert!(!state.selection.selected.contains(&renamed.id));
		assert_eq!(state.selection.focused, None);
	}

	#[test]
	fn selection_actions_filter_unknown_ids_toggle_and_clear() {
		let known = entry(b"/tmp", b"a", 1);
		let unknown = entry(b"/else", b"b", 2);
		let state = loaded(vec![known.clone()]);
		let selected = reduce(&state, StateAction::SetSelection {
			selected: vec![known.id, unknown.id],
			focused:  Some(unknown.id),
		});
		assert_eq!(selected.state.selection.selected.len(), 1);
		assert_eq!(selected.state.selection.focused, None);

		let toggled = reduce(&selected.state, StateAction::ToggleSelection(known.id));
		assert!(toggled.state.selection.selected.is_empty());
		assert_eq!(toggled.state.selection.focused, Some(known.id));
		let cleared = reduce(&toggled.state, StateAction::ClearSelection);
		assert_eq!(cleared.state.selection, Default::default());
	}

	#[test]
	fn task_updates_keep_insertion_order_and_reject_stale_revisions() {
		let mut state = BrowserState::default();
		state = reduce(&state, StateAction::UpdateTask(task(2, 1, "second"))).state;
		state = reduce(&state, StateAction::UpdateTask(task(1, 1, "first"))).state;
		state = reduce(&state, StateAction::UpdateTask(task(2, 2, "updated"))).state;
		assert_eq!(state.task_order, vec![TaskId(2), TaskId(1)]);
		assert_eq!(state.ordered_tasks().map(|task| task.title.as_str()).collect::<Vec<_>>(), vec![
			"updated", "first"
		]);

		let revision = state.revision;
		let stale = reduce(&state, StateAction::UpdateTask(task(2, 1, "stale")));
		assert_eq!(stale.state.revision, revision);
		assert!(stale.delta.is_none());

		let removed = reduce(&state, StateAction::RemoveTask(TaskId(2)));
		assert_eq!(removed.state.task_order, vec![TaskId(1)]);
		assert!(!removed.state.tasks.contains_key(&TaskId(2)));
	}

	#[test]
	fn accepted_transitions_emit_contiguous_monotonic_protocol_deltas() {
		let first = reduce(&BrowserState::default(), StateAction::NavigateTo(location(b"/tmp")));
		let delta = first.delta.expect("navigation delta");
		assert_eq!(delta.from_revision, StateRevision(0));
		assert_eq!(delta.revision, StateRevision(1));
		assert!(delta.is_applicable_to(StateRevision(0)));
		assert_eq!(first.state.snapshot().revision, StateRevision(1));
	}
}
