//! Хранилище проектов для Snappix.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::format::{
    BlueprintIndex, BlueprintPageIndex, ProjectArchiveHeader, ProjectFile, ARCHIVE_VERSION,
};
use core_blueprint::{BlueprintDocument, BlueprintDocumentKind, BlueprintOwner};
use shared::{from_msgpack, to_msgpack, Result, SnappixError};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const PROJECT_BIN_PATH: &str = "project.bin";
const UI_BIN_PATH: &str = "ui.bin";
const BLUEPRINT_INDEX_PATH: &str = "blueprints/index.bin";
const BLUEPRINT_PAGES_DIR: &str = "blueprints/pages";
const BLUEPRINT_SERVER_PATH: &str = "blueprints/server.bin";
const HISTORY_PATH: &str = "history/timeline.bin";
const ICON_PATH: &str = "meta/icon.png";
const DEFAULT_ICON_PNG: &[u8] = include_bytes!("../../../apps/resources/icons/icon.png");

#[derive(Debug)]
pub struct ProjectStorage {
    base_dir: PathBuf,
}

impl ProjectStorage {
    /// Создание нового хранилища проектов с базовым каталогом.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Установка формата по умолчанию для сохранения проектов.
    /// Получение базового каталога хранилища.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Получение пути к проекту по имени.
    pub fn project_path(&self, project_name: &str) -> PathBuf {
        self.base_dir.join(project_name).with_extension("spx")
    }

    /// Проверка существования проекта по имени.
    pub fn exists(&self, project_name: &str) -> bool {
        self.project_path(project_name).exists()
    }

    /// Сохраение проекта на диск.
    pub fn save(
        &self,
        project: &ProjectFile,
        path: &Path,
        assets_root: Option<&Path>,
    ) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        self.save_archive(project, path, assets_root)
    }

    /// Загрузка проекта с диска.
    pub fn load(&self, path: &Path) -> Result<ProjectFile> {
        if let Ok(project) = self.load_archive(path) {
            return Ok(project);
        }
        let content = fs::read(path)?;

        // Бинарный формат с MessagePack
        if let Ok(project) = from_msgpack(&content) {
            return Ok(project);
        }

        // Json формат
        if let Ok(project) = serde_json::from_slice(&content) {
            return Ok(project);
        }

        Err(SnappixError::Project(
            "Invalid project file format".to_string(),
        ))
    }

    fn save_archive(
        &self,
        project: &ProjectFile,
        path: &Path,
        assets_root: Option<&Path>,
    ) -> Result<()> {
        let file = File::create(path)?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

        let header = ProjectArchiveHeader {
            version: ARCHIVE_VERSION.to_string(),
            manifest: project.manifest.clone(),
            workspace_data: project.workspace_data.clone(),
            icon_path: Some(ICON_PATH.to_string()),
        };
        write_zip_entry(&mut zip, PROJECT_BIN_PATH, &to_msgpack(&header)?, options)?;
        write_zip_entry(
            &mut zip,
            UI_BIN_PATH,
            &to_msgpack(&project.ui_data)?,
            options,
        )?;

        let mut page_entries = Vec::new();
        let mut server_path = None;
        for document in &project.logic_data.documents {
            match (document.kind, &document.owner) {
                (BlueprintDocumentKind::PageBlueprint, BlueprintOwner::Page { page_id }) => {
                    let path = format!("{}/{}.bin", BLUEPRINT_PAGES_DIR, page_id);
                    page_entries.push(BlueprintPageIndex {
                        page_id: *page_id,
                        path: path.clone(),
                    });
                    write_zip_entry(&mut zip, &path, &to_msgpack(document)?, options)?;
                }
                (BlueprintDocumentKind::ServerBlueprint, BlueprintOwner::Project) => {
                    if server_path.is_none() {
                        server_path = Some(BLUEPRINT_SERVER_PATH.to_string());
                    }
                    write_zip_entry(
                        &mut zip,
                        BLUEPRINT_SERVER_PATH,
                        &to_msgpack(document)?,
                        options,
                    )?;
                }
                _ => {}
            }
        }

        let index = BlueprintIndex {
            version: ARCHIVE_VERSION.to_string(),
            pages: page_entries,
            server: server_path,
        };
        write_zip_entry(
            &mut zip,
            BLUEPRINT_INDEX_PATH,
            &to_msgpack(&index)?,
            options,
        )?;

        let root_dir = assets_root.unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")));
        add_assets_to_archive(&mut zip, root_dir, options)?;

        if let Ok(Some(history_bytes)) = load_history(path) {
            write_zip_entry(&mut zip, HISTORY_PATH, &history_bytes, options)?;
        }

        write_zip_entry(&mut zip, ICON_PATH, DEFAULT_ICON_PNG, options)?;

        zip.finish()
            .map_err(|err| SnappixError::Project(format!("Zip error: {err}")))?;
        Ok(())
    }

    fn load_archive(&self, path: &Path) -> Result<ProjectFile> {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|err| SnappixError::Project(format!("Invalid archive: {err}")))?;

        let header: ProjectArchiveHeader = {
            let bytes = read_zip_entry(&mut archive, PROJECT_BIN_PATH)?;
            from_msgpack(&bytes)?
        };
        let ui_data = {
            let bytes = read_zip_entry(&mut archive, UI_BIN_PATH)?;
            from_msgpack(&bytes)?
        };

        let index: BlueprintIndex = {
            let bytes = read_zip_entry(&mut archive, BLUEPRINT_INDEX_PATH)?;
            from_msgpack(&bytes)?
        };

        let mut documents: Vec<BlueprintDocument> = Vec::new();
        for page in index.pages {
            let bytes = read_zip_entry(&mut archive, &page.path)?;
            let doc: BlueprintDocument = from_msgpack(&bytes)?;
            documents.push(doc);
        }
        if let Some(server_path) = index.server {
            let bytes = read_zip_entry(&mut archive, &server_path)?;
            let doc: BlueprintDocument = from_msgpack(&bytes)?;
            documents.push(doc);
        }

        Ok(ProjectFile {
            version: header.version,
            manifest: header.manifest,
            ui_data,
            logic_data: core_blueprint::LogicData { documents },
            workspace_data: header.workspace_data,
        })
    }

    /// Листинг всех проектов в хранилище.
    pub fn list_projects(&self) -> Result<Vec<String>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "spx") {
                if let Some(name) = path.file_stem() {
                    projects.push(name.to_string_lossy().to_string());
                }
            }
        }

        Ok(projects)
    }

    /// Удаление проекта по имени.
    pub fn delete(&self, project_name: &str) -> Result<()> {
        let path = self.project_path(project_name);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Переименование проекта.
    pub fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        let old_path = self.project_path(old_name);
        let new_path = self.project_path(new_name);

        if old_path.exists() {
            fs::rename(old_path, new_path)?;
        }
        Ok(())
    }

    /// Создание резервной копии проекта.
    pub fn backup(&self, project_name: &str) -> Result<PathBuf> {
        let path = self.project_path(project_name);
        if !path.exists() {
            return Err(SnappixError::Project(format!(
                "Project not found: {}",
                project_name
            )));
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("{}_backup_{}.spx", project_name, timestamp);
        let backup_path = self.base_dir.join("backups").join(backup_name);

        fs::create_dir_all(backup_path.parent().unwrap())?;
        fs::copy(&path, &backup_path)?;

        Ok(backup_path)
    }
}

pub fn load_history(path: &Path) -> Result<Option<Vec<u8>>> {
    if !path.exists() {
        return Ok(None);
    }
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| SnappixError::Project(format!("Invalid archive: {err}")))?;

    if let Ok(mut entry) = archive.by_name(HISTORY_PATH) {
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        return Ok(Some(bytes));
    }
    Ok(None)
}

pub fn save_history(path: &Path, history_bytes: &[u8]) -> Result<()> {
    if !path.exists() {
        return Err(SnappixError::Project(
            "Project archive not found".to_string(),
        ));
    }

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| SnappixError::Project(format!("Invalid archive: {err}")))?;

    let temp_path = std::env::temp_dir().join(format!("snappix-{}.spx.tmp", uuid::Uuid::new_v4()));
    let temp_file = File::create(&temp_path)?;
    let mut zip = ZipWriter::new(temp_file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| SnappixError::Project(format!("Zip error: {err}")))?;
        let name = entry.name().to_string();
        if entry.is_dir() || name == HISTORY_PATH {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        write_zip_entry(&mut zip, &name, &bytes, options)?;
    }

    write_zip_entry(&mut zip, HISTORY_PATH, history_bytes, options)?;
    if let Err(err) = zip.finish() {
        let _ = fs::remove_file(&temp_path);
        return Err(SnappixError::Project(format!("Zip error: {err}")));
    }

    let backup_path = path.with_extension(format!("spx.bak.{}", uuid::Uuid::new_v4()));
    if let Err(err) = fs::rename(path, &backup_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(SnappixError::Project(format!(
            "Failed to create backup for history update: {err}"
        )));
    }

    if let Err(err) = fs::copy(&temp_path, path) {
        let _ = fs::rename(&backup_path, path);
        let _ = fs::remove_file(&temp_path);
        return Err(SnappixError::Project(format!(
            "Failed to replace project archive: {err}"
        )));
    }

    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_file(&backup_path);
    Ok(())
}

pub fn extract_assets(path: &Path, root_dir: &Path) -> Result<()> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| SnappixError::Project(format!("Invalid archive: {err}")))?;
    extract_assets_from_archive(&mut archive, root_dir)
}

fn write_zip_entry(
    zip: &mut ZipWriter<File>,
    name: &str,
    bytes: &[u8],
    options: FileOptions,
) -> Result<()> {
    zip.start_file(name, options)
        .map_err(|err| SnappixError::Project(format!("Zip error: {err}")))?;
    zip.write_all(bytes)?;
    Ok(())
}

fn read_zip_entry(archive: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| SnappixError::Project(format!("Missing archive entry: {name}")))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn add_assets_to_archive(
    zip: &mut ZipWriter<File>,
    root_dir: &Path,
    options: FileOptions,
) -> Result<()> {
    let assets_dir = root_dir.join("assets");
    if !assets_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(&assets_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root_dir).unwrap_or(entry.path());
        let name = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(entry.path())?;
        write_zip_entry(zip, &name, &bytes, options)?;
    }
    Ok(())
}

pub fn extract_assets_from_archive(archive: &mut ZipArchive<File>, root_dir: &Path) -> Result<()> {
    let assets_dir = root_dir.join("assets");
    if assets_dir.exists() {
        fs::remove_dir_all(&assets_dir)?;
    }

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| SnappixError::Project(format!("Zip error: {err}")))?;
        let name = entry.name().to_string();
        if !name.starts_with("assets/") || entry.is_dir() {
            continue;
        }
        let Some(output_path) = safe_archive_asset_path(root_dir, &name) else {
            continue;
        };
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output_file = File::create(output_path)?;
        std::io::copy(&mut entry, &mut output_file)?;
    }
    Ok(())
}

/// Ошибки хранилища проектов
fn safe_archive_asset_path(root_dir: &Path, name: &str) -> Option<PathBuf> {
    let mut components = Path::new(name).components();
    match components.next()? {
        Component::Normal(first) if first == OsStr::new("assets") => {}
        _ => return None,
    }

    let mut output_path = root_dir.join("assets");
    for component in components {
        match component {
            Component::Normal(part) => output_path.push(part),
            _ => return None,
        }
    }
    Some(output_path)
}

#[cfg(test)]
mod tests {
    use super::safe_archive_asset_path;
    use std::path::Path;

    #[test]
    fn safe_archive_asset_path_accepts_nested_assets() {
        let root = Path::new("root");

        assert_eq!(
            safe_archive_asset_path(root, "assets/images/icon.png"),
            Some(root.join("assets").join("images").join("icon.png"))
        );
    }

    #[test]
    fn safe_archive_asset_path_rejects_traversal() {
        let root = Path::new("root");

        assert!(safe_archive_asset_path(root, "assets/../../outside.txt").is_none());
        assert!(safe_archive_asset_path(root, "../assets/outside.txt").is_none());
        assert!(safe_archive_asset_path(root, "/assets/outside.txt").is_none());
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("Project not found: {0}")]
    NotFound(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}
