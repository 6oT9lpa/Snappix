use uuid::Uuid;

use crate::model::{BlueprintExport, BlueprintFunctionParameter, BlueprintPinType};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlueprintProjectApi {
    pub pages: Vec<PageApiDescriptor>,
    pub server: ServerApiDescriptor,
}

impl BlueprintProjectApi {
    pub fn page(&self, page_id: Uuid) -> Option<&PageApiDescriptor> {
        self.pages.iter().find(|page| page.page_id == page_id)
    }

    pub fn page_export(&self, page_id: Uuid, function_name: &str) -> Option<&BlueprintExport> {
        self.page(page_id)?
            .exported_functions
            .iter()
            .find(|export| export.signature.name == function_name)
    }

    pub fn server_export(&self, function_name: &str) -> Option<&BlueprintExport> {
        self.server
            .exported_functions
            .iter()
            .find(|export| export.signature.name == function_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageApiDescriptor {
    pub page_id: Uuid,
    pub page_name: String,
    pub elements: Vec<UiElementApiDescriptor>,
    pub exported_functions: Vec<BlueprintExport>,
}

impl PageApiDescriptor {
    pub fn element(&self, element_id: Uuid) -> Option<&UiElementApiDescriptor> {
        self.elements.iter().find(|element| element.element_id == element_id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerApiDescriptor {
    pub exported_functions: Vec<BlueprintExport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiElementApiDescriptor {
    pub element_id: Uuid,
    pub display_name: String,
    pub element_type: String,
    pub events: Vec<UiEventDescriptor>,
    pub actions: Vec<UiActionDescriptor>,
}

impl UiElementApiDescriptor {
    pub fn event(&self, event_name: &str) -> Option<&UiEventDescriptor> {
        self.events.iter().find(|event| event.name == event_name)
    }

    pub fn action(&self, action_name: &str) -> Option<&UiActionDescriptor> {
        self.actions.iter().find(|action| action.name == action_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiEventDescriptor {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiActionDescriptor {
    pub name: String,
    pub display_name: String,
    pub parameters: Vec<BlueprintFunctionParameter>,
    pub return_type: BlueprintPinType,
}
