use std::collections::HashSet;
use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::project::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryActionKind {
    CreateObject,
    CreatePage,
    ModifyObject,
    Group,
    Ungroup,
    DeleteObject,
    DeletePage,
    RenameObject,
    RenamePage,
    ReparentObject,
    Cut,
    Paste,
    Undo,
    Redo,
}

impl HistoryActionKind {
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::CreateObject => "create-object",
            Self::CreatePage => "create-page",
            Self::ModifyObject => "modify-object",
            Self::Group => "group",
            Self::Ungroup => "ungroup",
            Self::DeleteObject => "delete-object",
            Self::DeletePage => "delete-page",
            Self::RenameObject => "rename-object",
            Self::RenamePage => "rename-page",
            Self::ReparentObject => "reparent-object",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Undo => "undo",
            Self::Redo => "redo",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub project_bytes: Vec<u8>,
    pub selected_elements: Vec<Uuid>,
    pub collapsed_outline_nodes: Vec<Uuid>,
}

impl ProjectSnapshot {
    pub fn capture(
        project: &Project,
        selected_elements: &[Uuid],
        collapsed_outline_nodes: &HashSet<Uuid>,
    ) -> Option<Self> {
        let project_bytes = project.snapshot_binary().ok()?;
        Some(Self {
            project_bytes,
            selected_elements: selected_elements.to_vec(),
            collapsed_outline_nodes: collapsed_outline_nodes.iter().copied().collect(),
        })
    }

    pub fn restore_project(&self, project: &mut Project) -> bool {
        project.restore_from_binary(&self.project_bytes).is_ok()
    }

    pub fn is_same_state(&self, other: &Self) -> bool {
        self.project_bytes == other.project_bytes
            && self.selected_elements == other.selected_elements
            && self.collapsed_outline_nodes == other.collapsed_outline_nodes
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntryData {
    pub title: String,
    pub details: String,
    pub timestamp: String,
    pub action_kind: HistoryActionKind,
    pub after: ProjectSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandEntry {
    title: String,
    details: String,
    before: ProjectSnapshot,
    after: ProjectSnapshot,
}

#[derive(Debug, Clone)]
struct PreviewState {
    timeline_index: usize,
    live_snapshot: ProjectSnapshot,
}

#[derive(Debug, Clone)]
pub enum PreviewSelection {
    None,
    Apply(ProjectSnapshot),
    Restore(ProjectSnapshot),
}

#[derive(Debug, Clone)]
pub struct HistoryManager {
    capacity: usize,
    command_entries: Vec<CommandEntry>,
    cursor: usize,
    timeline_entries: Vec<TimelineEntryData>,
    preview: Option<PreviewState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedHistoryState {
    command_entries: Vec<CommandEntry>,
    cursor: usize,
    timeline_entries: Vec<TimelineEntryData>,
}

impl HistoryManager {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            command_entries: Vec::new(),
            cursor: 0,
            timeline_entries: Vec::new(),
            preview: None,
        }
    }

    pub fn reset(&mut self) {
        self.command_entries.clear();
        self.timeline_entries.clear();
        self.preview = None;
        self.cursor = 0;
    }

    pub fn timeline_entries(&self) -> &[TimelineEntryData] {
        &self.timeline_entries
    }

    pub fn active_timeline_index(&self) -> Option<usize> {
        self.preview.as_ref().map(|preview| preview.timeline_index)
    }

    pub fn is_preview_active(&self) -> bool {
        self.preview.is_some()
    }

    pub fn record_change(
        &mut self,
        action_kind: HistoryActionKind,
        title: impl Into<String>,
        details: impl Into<String>,
        before: ProjectSnapshot,
        after: ProjectSnapshot,
    ) {
        if before.is_same_state(&after) {
            return;
        }

        if self.cursor < self.command_entries.len() {
            self.command_entries.truncate(self.cursor);
        }

        let title = title.into();
        let details = details.into();
        self.command_entries.push(CommandEntry {
            title: title.clone(),
            details: details.clone(),
            before: before.clone(),
            after: after.clone(),
        });

        if self.command_entries.len() > self.capacity {
            self.command_entries.remove(0);
            self.cursor = self.cursor.saturating_sub(1);
        }
        self.cursor = self.command_entries.len();

        self.push_timeline(action_kind, title, details, after);
    }

    pub fn save_to_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let payload = PersistedHistoryState {
            command_entries: self.command_entries.clone(),
            cursor: self.cursor,
            timeline_entries: self.timeline_entries.clone(),
        };
        let bytes =
            shared::to_msgpack(&payload).map_err(|err| std::io::Error::other(err.to_string()))?;
        Ok(bytes)
    }

    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let payload: PersistedHistoryState =
            shared::from_msgpack(bytes).map_err(|err| std::io::Error::other(err.to_string()))?;

        self.command_entries = payload.command_entries;
        if self.command_entries.len() > self.capacity {
            self.command_entries.truncate(self.capacity);
        }

        self.timeline_entries = payload.timeline_entries;
        if self.timeline_entries.len() > self.capacity {
            self.timeline_entries.truncate(self.capacity);
        }

        self.cursor = payload.cursor.min(self.command_entries.len());
        self.preview = None;
        Ok(())
    }

    pub fn undo(&mut self) -> Option<ProjectSnapshot> {
        if self.cursor == 0 {
            return None;
        }

        let command = self.command_entries.get(self.cursor - 1)?.clone();
        self.cursor = self.cursor.saturating_sub(1);
        self.push_timeline(
            HistoryActionKind::Undo,
            format!("Откат: {}", command.title),
            command.details.clone(),
            command.before.clone(),
        );
        Some(command.before)
    }

    pub fn redo(&mut self) -> Option<ProjectSnapshot> {
        if self.cursor >= self.command_entries.len() {
            return None;
        }

        let command = self.command_entries.get(self.cursor)?.clone();
        self.cursor += 1;
        self.push_timeline(
            HistoryActionKind::Redo,
            format!("Возврат: {}", command.title),
            command.details.clone(),
            command.after.clone(),
        );
        Some(command.after)
    }

    pub fn select_timeline_entry(
        &mut self,
        index: usize,
        live_snapshot: ProjectSnapshot,
    ) -> PreviewSelection {
        let Some(entry) = self.timeline_entries.get(index).cloned() else {
            return PreviewSelection::None;
        };

        if let Some(preview) = &self.preview {
            if preview.timeline_index == index {
                let restore = preview.live_snapshot.clone();
                self.preview = None;
                return PreviewSelection::Restore(restore);
            }
        }

        let current_live = self
            .preview
            .as_ref()
            .map(|preview| preview.live_snapshot.clone())
            .unwrap_or(live_snapshot);
        self.preview = Some(PreviewState {
            timeline_index: index,
            live_snapshot: current_live,
        });
        PreviewSelection::Apply(entry.after)
    }

    pub fn exit_preview(&mut self) -> Option<ProjectSnapshot> {
        self.preview.take().map(|preview| preview.live_snapshot)
    }

    fn push_timeline(
        &mut self,
        action_kind: HistoryActionKind,
        title: String,
        details: String,
        after: ProjectSnapshot,
    ) {
        let timestamp = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        self.timeline_entries.insert(
            0,
            TimelineEntryData {
                title,
                details,
                timestamp,
                action_kind,
                after,
            },
        );
        if self.timeline_entries.len() > self.capacity {
            self.timeline_entries.truncate(self.capacity);
        }

        if let Some(preview) = &self.preview {
            if preview.timeline_index >= self.timeline_entries.len() {
                self.preview = None;
            }
        }
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new(100)
    }
}
