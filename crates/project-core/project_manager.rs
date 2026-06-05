use crate::project::{DevMode, PageSize, Platform, Project};

/// Owns the currently opened project for application sessions.
pub struct ProjectManager {
    current_project: Option<Project>,
}

impl ProjectManager {
    pub fn new() -> Self {
        Self {
            current_project: None,
        }
    }

    /// Create a new project and make it active.
    pub fn create_project(
        &mut self,
        name: &str,
        path: &str,
        platform: Platform,
        dev_mode: DevMode,
        initial_page_size: PageSize,
        custom_width: u32,
        custom_height: u32,
    ) -> &Project {
        let project = Project::new(
            name,
            path,
            platform,
            dev_mode,
            initial_page_size,
            custom_width,
            custom_height,
        );
        self.current_project = Some(project);
        self.current_project.as_ref().expect("project just set")
    }

    /// Load an existing project and make it active.
    pub fn load_project(&mut self, path: &str) -> Result<&Project, Box<dyn std::error::Error>> {
        let project = Project::load(path)?;
        self.current_project = Some(project);
        Ok(self.current_project.as_ref().expect("project just set"))
    }

    pub fn current_project_mut(&mut self) -> Option<&mut Project> {
        self.current_project.as_mut()
    }
}

impl Default for ProjectManager {
    fn default() -> Self {
        Self::new()
    }
}
