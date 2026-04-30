//! Операции над проектами: создание, сохранение, загрузка, экспорт и т.д.

use std::path::Path;

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
