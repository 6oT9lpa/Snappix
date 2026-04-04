//! Проектный формат для сохранения и загрузки данных проекта.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use core_blueprint::LogicData;
use core_ui_graphs::{ProjectData, ProjectManifest};

pub const FORMAT_VERSION: &str = "1.0.0";
pub const ARCHIVE_VERSION: &str = "1.0.0";

/// Готовая структура проекта для сохранения на диск. Содержит все данные, необходимые для восстановления проекта.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: String,
    pub manifest: ProjectManifest,
    pub ui_data: UiData,
    #[serde(default)]
    pub logic_data: LogicData,
    #[serde(default)]
    pub workspace_data: WorkspaceData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectArchiveHeader {
    pub version: String,
    pub manifest: ProjectManifest,
    #[serde(default)]
    pub workspace_data: WorkspaceData,
    #[serde(default)]
    pub icon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintIndex {
    pub version: String,
    #[serde(default)]
    pub pages: Vec<BlueprintPageIndex>,
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintPageIndex {
    pub page_id: Uuid,
    pub path: String,
}

impl ProjectFile {
    /// Создание нового проекта с заданным манифестом.
    pub fn new(manifest: ProjectManifest) -> Self {
        Self {
            version: FORMAT_VERSION.to_string(),
            manifest,
            ui_data: UiData::default(),
            logic_data: LogicData::default(),
            workspace_data: WorkspaceData::default(),
        }
    }

    /// Создание структуры проекта из данных проекта.
    pub fn from_project_data(data: ProjectData) -> Self {
        Self {
            version: FORMAT_VERSION.to_string(),
            manifest: data.manifest,
            ui_data: UiData {
                pages: data.pages,
                assets: data.assets,
            },
            logic_data: LogicData::default(),
            workspace_data: WorkspaceData::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiData {
    pub pages: Vec<core_ui_graphs::Page>,
    pub assets: Vec<core_ui_graphs::Asset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditorDocumentRef {
    PageUi { page_id: Uuid },
    PageBlueprint { document_id: Uuid },
    ServerBlueprint { document_id: Uuid },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceData {
    #[serde(default)]
    pub open_documents: Vec<EditorDocumentRef>,
    #[serde(default)]
    pub active_document: Option<EditorDocumentRef>,
}
