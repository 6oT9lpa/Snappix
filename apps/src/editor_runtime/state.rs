use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::app::project::ProjectManager;
use core_blueprint::BlueprintDiagnostic;
use std::sync::MutexGuard;

use super::{clipboard::ClipboardBuffer, history::HistoryManager};

#[derive(Clone)]
pub struct TransformPreviewState {
    pub element_id: Uuid,
    pub geometries: HashMap<Uuid, (f32, f32, f32, f32, f32)>,
}

#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct BlueprintEditorState {
    pub selected_graph_id: Rc<RefCell<Option<Uuid>>>,
    pub selected_nodes: Rc<RefCell<Vec<Uuid>>>,
    pub camera_position: Rc<RefCell<(f32, f32)>>,
    pub open_functions: Rc<RefCell<Vec<Uuid>>>,
    pub palette_page_id: Rc<RefCell<Option<Uuid>>>,
    pub compile_diagnostics: Rc<RefCell<Vec<BlueprintDiagnostic>>>,
}

#[derive(Clone)]
pub struct EditorState {
    pub project_manager: Arc<Mutex<ProjectManager>>,
    pub selected_elements: Rc<RefCell<Vec<Uuid>>>,
    pub hidden_elements: Rc<RefCell<HashSet<Uuid>>>,
    pub collapsed_outline_nodes: Rc<RefCell<HashSet<Uuid>>>,
    pub transform_preview: Rc<RefCell<Option<TransformPreviewState>>>,
    #[allow(dead_code)]
    pub blueprint: BlueprintEditorState,
    pub history: Rc<RefCell<HistoryManager>>,
    pub clipboard: Rc<RefCell<ClipboardBuffer>>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            project_manager: Arc::new(Mutex::new(ProjectManager::new())),
            selected_elements: Rc::new(RefCell::new(Vec::new())),
            hidden_elements: Rc::new(RefCell::new(HashSet::new())),
            collapsed_outline_nodes: Rc::new(RefCell::new(HashSet::new())),
            transform_preview: Rc::new(RefCell::new(None)),
            blueprint: BlueprintEditorState::default(),
            history: Rc::new(RefCell::new(HistoryManager::default())),
            clipboard: Rc::new(RefCell::new(ClipboardBuffer::default())),
        }
    }
}

pub trait ProjectManagerHandleExt {
    fn borrow_mut(&self) -> MutexGuard<'_, ProjectManager>;
}

impl ProjectManagerHandleExt for Arc<Mutex<ProjectManager>> {
    fn borrow_mut(&self) -> MutexGuard<'_, ProjectManager> {
        self.lock().unwrap()
    }
}
