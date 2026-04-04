use crate::element::UiElement;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Ошибки при работе с проектом
#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Project file not found: {0}")]
    FileNotFound(String),

    #[error("Invalid project structure: {0}")]
    InvalidStructure(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),
}

/// Платформа проекта.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    MacOS,
    Linux,
    Android,
    IOS,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FormFactor {
    Mobile,
    Desktop,
    Web,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Platform {
    pub os: Os,
    pub form_factor: FormFactor,
}

/// Режим разработки проекта.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProjectMode {
    #[default]
    Blueprint,
    Code,
    Hybrid,
}

/// Метаданные проекта.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MetadataProject {
    pub author: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

impl Default for MetadataProject {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            author: String::new(),
            description: String::new(),
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        }
    }
}

/// Манифест проекта.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectManifest {
    pub project_name: String,
    pub project_id: Uuid,
    pub entry_point: String,
    pub platforms: Vec<Platform>,
    pub mode: ProjectMode,
    pub metadata: MetadataProject,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            project_name: "New Project".to_string(),
            project_id: Uuid::new_v4(),
            entry_point: "main.slint".to_string(),
            platforms: vec![Platform {
                os: Os::Windows,
                form_factor: FormFactor::Desktop,
            }],
            mode: ProjectMode::default(),
            metadata: MetadataProject::default(),
        }
    }
}

/// Страница UI.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Page {
    pub id: Uuid,
    pub name: String,
    pub children: Vec<UiElement>,
    #[serde(default)]
    pub comments: Vec<PageComment>,
}

impl Page {
    /// Создает новую страницу с заданным именем
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            children: Vec::new(),
            comments: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PageCommentImage {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub rgba: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PageComment {
    pub id: Uuid,
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_comment_width")]
    pub width: f32,
    #[serde(default = "default_comment_body_height")]
    pub body_height: f32,
    #[serde(default = "default_comment_title")]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default = "default_comment_title_font_size")]
    pub title_font_size: f32,
    #[serde(default = "default_comment_body_font_size")]
    pub body_font_size: f32,
    #[serde(default)]
    pub image: Option<PageCommentImage>,
}

impl PageComment {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            id: Uuid::new_v4(),
            x,
            y,
            width: default_comment_width(),
            body_height: default_comment_body_height(),
            title: default_comment_title(),
            body: String::new(),
            title_font_size: default_comment_title_font_size(),
            body_font_size: default_comment_body_font_size(),
            image: None,
        }
    }
}

fn default_comment_title() -> String {
    "Comment".to_string()
}

fn default_comment_width() -> f32 {
    500.0
}

fn default_comment_body_height() -> f32 {
    76.0
}

fn default_comment_title_font_size() -> f32 {
    16.0
}

fn default_comment_body_font_size() -> f32 {
    13.0
}

/// Ассеты проекта.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Asset {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub mime_type: String,
}

impl Asset {
    /// Создает новый ассет
    pub fn new(name: String, path: String, mime_type: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            mime_type,
        }
    }
}

/// Данные проекта.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ProjectData {
    pub manifest: ProjectManifest,
    pub pages: Vec<Page>,
    pub assets: Vec<Asset>,
}
