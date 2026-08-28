//! Bounded stable FIFO work queue and internal leaf shapes.

use std::collections::VecDeque;

use thallium_fs_core::{FileAttrs, Location};
use thallium_protocol::TaskId;

/// Priority is retained in inputs for future scheduling policy. MVP dispatch
/// intentionally ignores it and preserves admission order.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Priority {
	Low,
	#[default]
	Normal,
	High,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceClass {
	Mutation,
	FileIo,
	Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkItem {
	Discover { root: Location },
	CopyFile { source: Location, destination: Location, attributes: Option<FileAttrs> },
	MoveEntry { source: Location, destination: Location },
	DeleteEntry { target: Location },
	TrashEntry { target: Location },
	Cleanup { temporary: Location },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedWork {
	pub task_id:          TaskId,
	pub resource:         ResourceClass,
	pub priority:         Priority,
	pub enqueue_sequence: u64,
	pub item:             WorkItem,
}

/// A fixed-capacity FIFO that never silently discards work.
#[derive(Debug)]
pub struct StableQueue<T> {
	capacity: usize,
	items:    VecDeque<T>,
}

impl<T> StableQueue<T> {
	#[must_use]
	pub fn new(capacity: usize) -> Self { Self { capacity: capacity.max(1), items: VecDeque::new() } }

	pub fn push(&mut self, item: T) -> Result<(), T> {
		if self.items.len() == self.capacity {
			Err(item)
		} else {
			self.items.push_back(item);
			Ok(())
		}
	}

	pub fn pop(&mut self) -> Option<T> { self.items.pop_front() }

	#[must_use]
	pub fn len(&self) -> usize { self.items.len() }

	#[must_use]
	pub fn is_empty(&self) -> bool { self.items.is_empty() }
}
