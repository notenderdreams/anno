use crate::models::Annotation;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const DEFAULT_CAPACITY: usize = 100;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub annotations: Vec<Annotation>,
    pub selected: HashSet<u32>,
    pub next_id: u32,
}

pub struct History {
    undo_stack: Vec<AppSnapshot>,
    redo_stack: Vec<AppSnapshot>,
    pending_edit: Option<AppSnapshot>,
    max_capacity: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pending_edit: None,
            max_capacity: capacity.max(1),
        }
    }

    /// Record a direct snapshot before a discrete mutation (e.g. deletion, color preset change).
    pub fn record(&mut self, snapshot: AppSnapshot) {
        self.pending_edit = None;
        if let Some(top) = self.undo_stack.last()
            && top == &snapshot
        {
            return;
        }
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > self.max_capacity {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Begin tracking an interactive edit (e.g. drag-resizing or typing into a field).
    /// If an edit is already in progress, this is a no-op so the initial baseline is preserved.
    pub fn begin_edit(&mut self, snapshot: AppSnapshot) {
        if self.pending_edit.is_none() {
            self.pending_edit = Some(snapshot);
        }
    }

    /// Complete an interactive edit. If the current state changed compared to when
    /// `begin_edit` was called, the baseline snapshot is committed to the undo stack.
    pub fn commit_edit(&mut self, current: &AppSnapshot) {
        if let Some(before) = self.pending_edit.take()
            && &before != current
        {
            if let Some(top) = self.undo_stack.last()
                && top == &before
            {
                self.redo_stack.clear();
                return;
            }
            self.undo_stack.push(before);
            if self.undo_stack.len() > self.max_capacity {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
    }

    pub fn has_pending_edit(&self) -> bool {
        self.pending_edit.is_some()
    }

    /// Perform undo. If an uncommitted pending edit was active, commits the baseline
    /// then restores the previous state.
    pub fn undo(&mut self, current: AppSnapshot) -> Option<AppSnapshot> {
        if let Some(before) = self.pending_edit.take()
            && before != current
        {
            self.undo_stack.push(before);
        }
        let previous = self.undo_stack.pop()?;
        self.redo_stack.push(current);
        if self.redo_stack.len() > self.max_capacity {
            self.redo_stack.remove(0);
        }
        Some(previous)
    }

    /// Perform redo. Pushes `current` state to undo stack and returns the next state.
    pub fn redo(&mut self, current: AppSnapshot) -> Option<AppSnapshot> {
        self.pending_edit = None;
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current);
        if self.undo_stack.len() > self.max_capacity {
            self.undo_stack.remove(0);
        }
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pending_edit = None;
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_annotation(id: u32) -> Annotation {
        Annotation {
            id,
            label: format!("region_{id}"),
            description: None,
            x: 10.0 * id as f32,
            y: 10.0 * id as f32,
            width: 50.0,
            height: 50.0,
            color: [255, 0, 0],
            parent_id: None,
            locked: false,
            points: None,
        }
    }

    fn snapshot(ids: &[u32], selected: &[u32], next_id: u32) -> AppSnapshot {
        AppSnapshot {
            annotations: ids.iter().copied().map(sample_annotation).collect(),
            selected: selected.iter().copied().collect(),
            next_id,
        }
    }

    #[test]
    fn test_record_and_undo_redo() {
        let mut history = History::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());

        let s0 = snapshot(&[], &[], 1);
        let s1 = snapshot(&[1], &[1], 2);
        let s2 = snapshot(&[1, 2], &[2], 3);

        // Record s0 -> current is s1
        history.record(s0.clone());
        assert!(history.can_undo());
        assert!(!history.can_redo());

        // Record s1 -> current is s2
        history.record(s1.clone());

        // Undo to s1
        let restored1 = history.undo(s2.clone()).unwrap();
        assert_eq!(restored1, s1);
        assert!(history.can_undo());
        assert!(history.can_redo());

        // Undo to s0
        let restored0 = history.undo(s1.clone()).unwrap();
        assert_eq!(restored0, s0);
        assert!(!history.can_undo());
        assert!(history.can_redo());

        // Redo to s1
        let redone1 = history.redo(s0.clone()).unwrap();
        assert_eq!(redone1, s1);

        // Redo to s2
        let redone2 = history.redo(s1.clone()).unwrap();
        assert_eq!(redone2, s2);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_multi_select_undo_redo() {
        let mut history = History::new();
        let s0 = snapshot(&[1, 2, 3], &[1, 2], 4);
        let s1 = snapshot(&[1, 2, 3], &[1, 2, 3], 4);

        history.record(s0.clone());
        let restored = history.undo(s1.clone()).unwrap();
        assert_eq!(restored.selected.len(), 2);
        assert!(restored.selected.contains(&1));
        assert!(restored.selected.contains(&2));

        let redone = history.redo(s0).unwrap();
        assert_eq!(redone.selected.len(), 3);
        assert!(redone.selected.contains(&1));
        assert!(redone.selected.contains(&2));
        assert!(redone.selected.contains(&3));
    }

    #[test]
    fn test_begin_and_commit_edit() {
        let mut history = History::new();
        let s0 = snapshot(&[1], &[1], 2);
        let s1 = snapshot(&[1, 2], &[2], 3);

        history.begin_edit(s0.clone());
        // Duplicate call does not overwrite baseline
        history.begin_edit(snapshot(&[999], &[], 999));

        // Commit with change
        history.commit_edit(&s1);
        assert!(history.can_undo());

        let restored = history.undo(s1.clone()).unwrap();
        assert_eq!(restored, s0);
    }

    #[test]
    fn test_commit_edit_without_change_ignored() {
        let mut history = History::new();
        let s0 = snapshot(&[1], &[1], 2);

        history.begin_edit(s0.clone());
        history.commit_edit(&s0);
        assert!(!history.can_undo());
    }

    #[test]
    fn test_capacity_limit() {
        let mut history = History::with_capacity(3);

        for i in 0..5 {
            history.record(snapshot(&[i], &[i], i + 1));
        }

        assert_eq!(history.undo_count(), 3);
    }

    #[test]
    fn test_clear() {
        let mut history = History::new();
        history.record(snapshot(&[1], &[1], 2));
        history.begin_edit(snapshot(&[2], &[2], 3));
        history.clear();

        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_count(), 0);
    }
}
