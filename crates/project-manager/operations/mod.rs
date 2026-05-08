//! Операции над проектами: создание, сохранение, загрузка, экспорт и т.д.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use core_ui_graphs::{Page, ProjectManifest, ProjectMode};
use shared::Result;

use crate::format::ProjectFile;
use crate::storage::{extract_assets, load_history, save_history, ProjectStorage};

/// Создание нового проекта с заданным именем.
pub fn create_project(name: &str) -> Result<ProjectFile> {
    let manifest = ProjectManifest {
        project_name: name.to_string(),
        ..ProjectManifest::default()
    };

    // Создание с базовой страницей
    let default_page = Page::new("Main".to_string());

    let mut project = ProjectFile::new(manifest);
    project.ui_data.pages.push(default_page);

    Ok(project)
}

/// Создание нового проекта из шаблона.
pub fn create_project_from_template(name: &str, template: ProjectTemplate) -> Result<ProjectFile> {
    let mut project = create_project(name)?;

    match template {
        ProjectTemplate::Blank => {
            // Just the default empty page
        }
        ProjectTemplate::DesktopApp => {
            // Добавляем типичные страницы для десктопного приложения
            project.ui_data.pages.push(Page::new("Main".to_string()));
            project
                .ui_data
                .pages
                .push(Page::new("Settings".to_string()));
            project.manifest.mode = ProjectMode::Hybrid;
        }
        ProjectTemplate::MobileApp => {
            // Добавляем типичные страницы для мобильного приложения
            project.ui_data.pages.push(Page::new("Home".to_string()));
            project.ui_data.pages.push(Page::new("Profile".to_string()));
            project
                .ui_data
                .pages
                .push(Page::new("Settings".to_string()));
        }
        ProjectTemplate::WebApp => {
            // Добавляем типичные страницы для веб-приложения
            project.ui_data.pages.push(Page::new("Index".to_string()));
            project.ui_data.pages.push(Page::new("About".to_string()));
        }
    }

    Ok(project)
}

/// Щаблон проекта по умолчанию.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectTemplate {
    Blank,
    DesktopApp,
    MobileApp,
    WebApp,
}

/// Сохранение проекта на диск.
pub fn save_project(project: &ProjectFile, path: &Path) -> Result<()> {
    let storage = ProjectStorage::new(path.parent().unwrap_or(Path::new(".")));
    storage.save(project, path, None)
}

/// РЎРѕС…СЂР°РЅРµРЅРёРµ РїСЂРѕРµРєС‚Р° РЅР° РґРёСЃРє СЃ РёСЃРїРѕР»СЊР·РѕРІР°РЅРёРµРј assets cache.
pub fn save_project_with_assets(
    project: &ProjectFile,
    path: &Path,
    assets_root: &Path,
) -> Result<()> {
    let storage = ProjectStorage::new(path.parent().unwrap_or(Path::new(".")));
    storage.save(project, path, Some(assets_root))
}

/// Загрузка проекта с диска.
pub fn load_project(path: &Path) -> Result<ProjectFile> {
    let storage = ProjectStorage::new(path.parent().unwrap_or(Path::new(".")));
    storage.load(path)
}

/// Р—Р°РіСЂСѓР·РєР° assets РёР· Р°СЂС…РёРІР° РІ temp-кэш.
pub fn extract_project_assets(path: &Path, assets_root: &Path) -> Result<()> {
    extract_assets(path, assets_root)
}

/// Загрузка истории проекта из архива.
pub fn load_project_history(path: &Path) -> Result<Option<Vec<u8>>> {
    load_history(path)
}

/// Сохранение истории проекта в архив.
pub fn save_project_history(path: &Path, bytes: &[u8]) -> Result<()> {
    save_history(path, bytes)
}

pub fn relocate_project_file(
    source: &Path,
    destination_folder: &Path,
    project_name: &str,
) -> std::io::Result<PathBuf> {
    let file_name = source
        .file_name()
        .map(|value| value.to_owned())
        .unwrap_or_else(|| {
            OsString::from(format!("{}.spx", sanitize_project_file_stem(project_name)))
        });
    let destination = destination_folder.join(file_name);

    if source == destination {
        return Ok(source.to_path_buf());
    }

    std::fs::rename(source, &destination)?;
    Ok(destination)
}

pub fn delete_project_path(path: &Path, project_name: &str) -> std::io::Result<bool> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(false);
    }

    if path.is_file() {
        std::fs::remove_file(path)?;
        return Ok(true);
    }

    if !path.is_dir() {
        return Ok(false);
    }

    let mut removed_any = false;

    if !project_name.trim().is_empty() {
        let named_candidate = path.join(format!("{}.spx", project_name.trim()));
        if named_candidate.exists() && named_candidate.is_file() {
            std::fs::remove_file(named_candidate)?;
            removed_any = true;
        }
    }

    if !removed_any {
        let spx_files = list_spx_files(path)?;
        if spx_files.len() == 1 {
            std::fs::remove_file(&spx_files[0])?;
            removed_any = true;
        }
    }

    if removed_any && path.read_dir()?.next().is_none() {
        let _ = std::fs::remove_dir(path);
    }

    Ok(removed_any)
}

pub fn sanitize_project_file_stem(name: &str) -> String {
    let mut sanitized = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

fn list_spx_files(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut spx_files = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let is_spx = entry_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("spx"))
            .unwrap_or(false);
        if entry_path.is_file() && is_spx {
            spx_files.push(entry_path);
        }
    }
    Ok(spx_files)
}

/// Экспорт проекта в код для целевой платформы.
pub fn export_project(_project: &ProjectFile, output_dir: &Path) -> Result<ExportResult> {
    use std::fs;

    fs::create_dir_all(output_dir)?;

    let mut result = ExportResult::default();

    // Export UI to Slint code
    let ui_dir = output_dir.join("ui");
    fs::create_dir_all(&ui_dir)?;

    result.success = true;
    Ok(result)
}

/// Result of project export.
#[derive(Debug, Default)]
pub struct ExportResult {
    pub success: bool,
    pub ui_files: Vec<std::path::PathBuf>,
    pub logic_files: Vec<std::path::PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn sanitize_project_file_stem_keeps_safe_name() {
        assert_eq!(sanitize_project_file_stem("My Project_01"), "My Project_01");
    }

    #[test]
    fn sanitize_project_file_stem_replaces_unsafe_chars() {
        assert_eq!(sanitize_project_file_stem("bad/name:*"), "bad_name__");
    }

    #[test]
    fn sanitize_project_file_stem_falls_back_for_blank_name() {
        assert_eq!(sanitize_project_file_stem("   "), "project");
    }

    #[test]
    fn delete_project_path_removes_direct_file() {
        let temp = tempdir().expect("temp dir");
        let file = temp.path().join("app.spx");
        fs::write(&file, b"project").expect("write project");

        let removed = delete_project_path(&file, "app").expect("delete project");

        assert!(removed);
        assert!(!file.exists());
    }

    #[test]
    fn delete_project_path_removes_named_file_in_directory() {
        let temp = tempdir().expect("temp dir");
        let file = temp.path().join("Named.spx");
        fs::write(&file, b"project").expect("write project");

        let removed = delete_project_path(temp.path(), "Named").expect("delete project");

        assert!(removed);
        assert!(!file.exists());
    }

    #[test]
    fn delete_project_path_removes_single_spx_when_name_does_not_match() {
        let temp = tempdir().expect("temp dir");
        let file = temp.path().join("Only.spx");
        fs::write(&file, b"project").expect("write project");

        let removed = delete_project_path(temp.path(), "Other").expect("delete project");

        assert!(removed);
        assert!(!file.exists());
    }

    #[test]
    fn delete_project_path_does_not_delete_ambiguous_directory() {
        let temp = tempdir().expect("temp dir");
        let one = temp.path().join("one.spx");
        let two = temp.path().join("two.spx");
        fs::write(&one, b"one").expect("write one");
        fs::write(&two, b"two").expect("write two");

        let removed = delete_project_path(temp.path(), "Other").expect("delete project");

        assert!(!removed);
        assert!(one.exists());
        assert!(two.exists());
    }

    #[test]
    fn delete_project_path_returns_false_for_missing_path() {
        let temp = tempdir().expect("temp dir");
        let missing = temp.path().join("missing.spx");

        let removed = delete_project_path(&missing, "missing").expect("delete project");

        assert!(!removed);
    }

    #[test]
    fn relocate_project_file_moves_file_to_folder() {
        let source_dir = tempdir().expect("source dir");
        let dest_dir = tempdir().expect("dest dir");
        let source = source_dir.path().join("app.spx");
        fs::write(&source, b"project").expect("write project");

        let destination =
            relocate_project_file(&source, dest_dir.path(), "app").expect("relocate project");

        assert_eq!(destination, dest_dir.path().join("app.spx"));
        assert!(!source.exists());
        assert!(destination.exists());
    }

    #[test]
    fn relocate_project_file_returns_same_path_when_destination_matches() {
        let temp = tempdir().expect("temp dir");
        let source = temp.path().join("app.spx");
        fs::write(&source, b"project").expect("write project");

        let destination = relocate_project_file(&source, temp.path(), "app").expect("relocate");

        assert_eq!(destination, source);
        assert!(destination.exists());
    }
}

/// Add a page to a project.
pub fn add_page(project: &mut ProjectFile, page: Page) {
    project.ui_data.pages.push(page);
}

/// Remove a page from a project.
pub fn remove_page(project: &mut ProjectFile, page_name: &str) -> Option<Page> {
    let idx = project
        .ui_data
        .pages
        .iter()
        .position(|p| p.name == page_name)?;
    Some(project.ui_data.pages.remove(idx))
}

/// Get a page by name.
pub fn get_page<'a>(project: &'a ProjectFile, page_name: &str) -> Option<&'a Page> {
    project.ui_data.pages.iter().find(|p| p.name == page_name)
}

/// Get a mutable page by name.
pub fn get_page_mut<'a>(project: &'a mut ProjectFile, page_name: &str) -> Option<&'a mut Page> {
    project
        .ui_data
        .pages
        .iter_mut()
        .find(|p| p.name == page_name)
}

/// Validate a project.
pub fn validate_project(project: &ProjectFile) -> Result<Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check manifest
    if project.manifest.project_name.is_empty() {
        errors.push(ValidationError {
            field: "manifest.project_name".to_string(),
            message: "Project name cannot be empty".to_string(),
        });
    }

    // Check pages
    if project.ui_data.pages.is_empty() {
        errors.push(ValidationError {
            field: "ui_data.pages".to_string(),
            message: "Project must have at least one page".to_string(),
        });
    }

    // Check for duplicate page names
    let mut page_names = std::collections::HashSet::new();
    for page in &project.ui_data.pages {
        if !page_names.insert(&page.name) {
            errors.push(ValidationError {
                field: format!("ui_data.pages[{}]", page.name),
                message: format!("Duplicate page name: {}", page.name),
            });
        }
    }

    Ok(errors)
}

/// Validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Error message.
    pub message: String,
}
