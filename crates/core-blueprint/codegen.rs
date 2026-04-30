use std::collections::HashMap;

use uuid::Uuid;

use crate::lowering::{
    BlueprintIrDocument, BlueprintIrFunction, BlueprintIrFunctionTrigger, BlueprintIrProject,
    BlueprintIrStatement, BlueprintIrValue,
};
use crate::model::{BlueprintFunctionTarget, BlueprintPinType};

#[derive(Debug, Clone)]
pub struct GeneratedBlueprintProject {
    pub files: Vec<GeneratedFile>,
    pub source_map: BlueprintSourceMap,
}

#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone, Default)]
pub struct BlueprintSourceMap {
    pub spans: Vec<GeneratedNodeSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedNodeSpan {
    pub document_id: Uuid,
    pub graph_id: Uuid,
    pub node_id: Uuid,
    pub pin_id: Option<Uuid>,
    pub file: String,
    pub line: usize,
    pub column: usize,
}

pub fn generate_project(project_name: &str, ir: &BlueprintIrProject) -> GeneratedBlueprintProject {
    let mut files = Vec::new();
    let mut source_map = BlueprintSourceMap::default();

    files.push(GeneratedFile {
        path: "Cargo.toml".to_string(),
        contents: workspace_manifest(),
    });

    files.push(GeneratedFile {
        path: "generated/Cargo.toml".to_string(),
        contents: crate_manifest(project_name),
    });

    files.push(GeneratedFile {
        path: "generated/src/runtime.rs".to_string(),
        contents: runtime_module(),
    });

    let module_names = unique_document_module_names(&ir.documents);
    let lib_rs = build_lib_rs(&module_names);
    files.push(GeneratedFile {
        path: "generated/src/lib.rs".to_string(),
        contents: lib_rs,
    });

    for document in &ir.documents {
        let module_name = module_names
            .get(&document.id)
            .cloned()
            .unwrap_or_else(|| sanitize_ident(document.name.clone()));
        let path = format!("generated/src/{module_name}.rs");
        let (contents, spans) = build_document_module(document, &path);
        source_map.spans.extend(spans);
        files.push(GeneratedFile { path, contents });
    }

    GeneratedBlueprintProject { files, source_map }
}

fn workspace_manifest() -> String {
    "[workspace]\nresolver = \"2\"\nmembers = [\"generated\"]\n".to_string()
}

fn crate_manifest(project_name: &str) -> String {
    format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
        sanitize_package_name(project_name)
    )
}

fn runtime_module() -> String {
    r#"#[derive(Clone, Debug, PartialEq)]
pub enum BlueprintValue {
    Void,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(String),
    UiElementRef(String),
    PageRef(String),
    ApiRef(String),
}

pub trait BlueprintRuntime {
    fn set_element_text(&mut self, page_id: &str, element_id: &str, value: String);
    fn get_blueprint_variable(
        &mut self,
        variable_id: &str,
        variable_name: &str,
    ) -> BlueprintValue {
        let _ = (variable_id, variable_name);
        BlueprintValue::Void
    }
    fn set_blueprint_variable(
        &mut self,
        variable_id: &str,
        variable_name: &str,
        value: BlueprintValue,
    ) {
        let _ = (variable_id, variable_name, value);
    }
    fn call_server_function(
        &mut self,
        function_name: &str,
        arguments: &[BlueprintValue],
    ) -> BlueprintValue;
    fn call_page_function(
        &mut self,
        page_id: &str,
        function_name: &str,
        arguments: &[BlueprintValue],
    ) -> BlueprintValue;
    fn execute_functional_node(
        &mut self,
        node_id: &str,
        arguments: &[BlueprintValue],
    ) -> BlueprintValue {
        let _ = (node_id, arguments);
        BlueprintValue::Void
    }
}

pub fn value_to_string(value: &BlueprintValue) -> String {
    match value {
        BlueprintValue::Void => String::new(),
        BlueprintValue::Bool(value) => value.to_string(),
        BlueprintValue::Int(value) => value.to_string(),
        BlueprintValue::Float(value) => value.to_string(),
        BlueprintValue::String(value) => value.clone(),
        BlueprintValue::Color(value) => value.clone(),
        BlueprintValue::UiElementRef(value) => value.clone(),
        BlueprintValue::PageRef(value) => value.clone(),
        BlueprintValue::ApiRef(value) => value.clone(),
    }
}

pub fn value_to_bool(value: &BlueprintValue) -> bool {
    match value {
        BlueprintValue::Bool(value) => *value,
        BlueprintValue::Int(value) => *value != 0,
        BlueprintValue::Float(value) => *value != 0.0,
        BlueprintValue::String(value) => !value.is_empty(),
        BlueprintValue::Color(value)
        | BlueprintValue::UiElementRef(value)
        | BlueprintValue::PageRef(value)
        | BlueprintValue::ApiRef(value) => !value.is_empty(),
        BlueprintValue::Void => false,
    }
}
"#
    .to_string()
}

fn build_lib_rs(module_names: &HashMap<Uuid, String>) -> String {
    let mut lines = vec!["pub mod runtime;".to_string()];
    let mut entries: Vec<_> = module_names.values().cloned().collect();
    entries.sort();
    for name in entries {
        lines.push(format!("pub mod {name};"));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn unique_document_module_names(documents: &[BlueprintIrDocument]) -> HashMap<Uuid, String> {
    let mut used = HashMap::<String, usize>::new();
    let mut names = HashMap::new();

    for document in documents {
        let base = sanitize_ident(document.name.replace('.', "_"));
        let next_index = used.entry(base.clone()).or_insert(0);
        let module_name = if *next_index == 0 {
            base.clone()
        } else {
            format!("{base}_{next_index}")
        };
        *next_index += 1;
        names.insert(document.id, module_name);
    }

    names
}

fn build_document_module(
    document: &BlueprintIrDocument,
    path: &str,
) -> (String, Vec<GeneratedNodeSpan>) {
    let mut writer = RustWriter::new();
    let mut spans = Vec::new();

    let function_name_map: HashMap<&str, &str> = document
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function.rust_name.as_str()))
        .collect();

    writer.push_line(
        "use crate::runtime::{value_to_bool, value_to_string, BlueprintRuntime, BlueprintValue};",
    );
    writer.push_line("");
    writer.push_line(format!(
        "pub const DOCUMENT_NAME: &str = {:?};",
        document.name
    ));
    writer.push_line("");

    for function in &document.functions {
        build_function(
            &mut writer,
            document,
            function,
            path,
            &function_name_map,
            &mut spans,
        );
        writer.push_line("");
    }

    (writer.finish(), spans)
}

fn build_function(
    writer: &mut RustWriter,
    document: &BlueprintIrDocument,
    function: &BlueprintIrFunction,
    path: &str,
    function_name_map: &HashMap<&str, &str>,
    spans: &mut Vec<GeneratedNodeSpan>,
) {
    let mut parameters = vec!["runtime: &mut dyn BlueprintRuntime".to_string()];
    parameters.extend(
        function
            .signature
            .parameters
            .iter()
            .map(|parameter| format!("{}: BlueprintValue", sanitize_ident(parameter.name.clone()))),
    );

    writer.push_line(format!(
        "pub fn {}({}) -> BlueprintValue {{",
        function.rust_name,
        parameters.join(", ")
    ));
    writer.indent += 1;

    match &function.trigger {
        BlueprintIrFunctionTrigger::Event {
            element_id,
            event_name,
        } => writer.push_line(format!("// Event trigger: {event_name} on {element_id}")),
        BlueprintIrFunctionTrigger::Function => writer.push_line("// User function entry"),
    }

    let mut temp_index = 0usize;
    for statement in &function.statements {
        build_statement(
            writer,
            document,
            function,
            statement,
            path,
            function_name_map,
            spans,
            &mut temp_index,
        );
    }

    writer.push_line("BlueprintValue::Void");
    writer.indent -= 1;
    writer.push_line("}");
}

fn build_statement(
    writer: &mut RustWriter,
    document: &BlueprintIrDocument,
    function: &BlueprintIrFunction,
    statement: &BlueprintIrStatement,
    path: &str,
    function_name_map: &HashMap<&str, &str>,
    spans: &mut Vec<GeneratedNodeSpan>,
    temp_index: &mut usize,
) {
    let line_before = writer.current_line();
    match statement {
        BlueprintIrStatement::SetElementText {
            node_id,
            value_pin_id,
            element_id,
            page_id,
            value,
        } => {
            let value_expr = value_expr(value);
            writer.push_line(format!(
                "runtime.set_element_text({:?}, {:?}, value_to_string(&{}));",
                page_id.to_string(),
                element_id.to_string(),
                value_expr
            ));
            spans.push(GeneratedNodeSpan {
                document_id: document.id,
                graph_id: function.graph_id,
                node_id: *node_id,
                pin_id: *value_pin_id,
                file: path.to_string(),
                line: line_before,
                column: 1,
            });
        }
        BlueprintIrStatement::SetVariable {
            node_id,
            variable_id,
            variable_name,
            value,
        } => {
            let temp_name = format!("variable_value_{}", *temp_index);
            *temp_index += 1;
            writer.push_line(format!("let {} = {};", temp_name, value_expr(value)));
            writer.push_line(format!(
                "runtime.set_blueprint_variable({:?}, {:?}, {});",
                variable_id.to_string(),
                variable_name,
                temp_name
            ));
            spans.push(GeneratedNodeSpan {
                document_id: document.id,
                graph_id: function.graph_id,
                node_id: *node_id,
                pin_id: None,
                file: path.to_string(),
                line: line_before,
                column: 1,
            });
        }
        BlueprintIrStatement::Branch {
            node_id,
            condition_pin_id,
            condition,
            true_statements,
            false_statements,
        } => {
            let condition_expr = value_expr(condition);
            writer.push_line(format!("if value_to_bool(&{}) {{", condition_expr));
            spans.push(GeneratedNodeSpan {
                document_id: document.id,
                graph_id: function.graph_id,
                node_id: *node_id,
                pin_id: *condition_pin_id,
                file: path.to_string(),
                line: line_before,
                column: 1,
            });
            writer.indent += 1;
            for statement in true_statements {
                build_statement(
                    writer,
                    document,
                    function,
                    statement,
                    path,
                    function_name_map,
                    spans,
                    temp_index,
                );
            }
            writer.indent -= 1;
            writer.push_line("} else {");
            writer.indent += 1;
            for statement in false_statements {
                build_statement(
                    writer,
                    document,
                    function,
                    statement,
                    path,
                    function_name_map,
                    spans,
                    temp_index,
                );
            }
            writer.indent -= 1;
            writer.push_line("}");
        }
        BlueprintIrStatement::CallDocumentFunction {
            node_id,
            target,
            function_name,
            arguments,
        } => {
            let args_expr = format!(
                "&[{}]",
                arguments
                    .iter()
                    .map(value_expr)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let temp_name = format!("call_result_{}", *temp_index);
            *temp_index += 1;
            let statement = match target {
                BlueprintFunctionTarget::ThisDocument => {
                    let target_name = function_name_map
                        .get(function_name.as_str())
                        .copied()
                        .unwrap_or(function_name.as_str());
                    let direct_args = arguments
                        .iter()
                        .map(value_expr)
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "let _{} = {}(runtime{}{});",
                        temp_name,
                        target_name,
                        if direct_args.is_empty() { "" } else { ", " },
                        direct_args
                    )
                }
                BlueprintFunctionTarget::Server => format!(
                    "let _{} = runtime.call_server_function({:?}, {});",
                    temp_name, function_name, args_expr
                ),
                BlueprintFunctionTarget::Page { page_id } => format!(
                    "let _{} = runtime.call_page_function({:?}, {:?}, {});",
                    temp_name,
                    page_id.to_string(),
                    function_name,
                    args_expr
                ),
            };
            writer.push_line(statement);
            spans.push(GeneratedNodeSpan {
                document_id: document.id,
                graph_id: function.graph_id,
                node_id: *node_id,
                pin_id: None,
                file: path.to_string(),
                line: line_before,
                column: 1,
            });
        }
        BlueprintIrStatement::FunctionalNode {
            node_id,
            functional_node_id,
            arguments,
        } => {
            let args_expr = format!(
                "&[{}]",
                arguments
                    .iter()
                    .map(value_expr)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let temp_name = format!("functional_result_{}", *temp_index);
            *temp_index += 1;
            writer.push_line(format!(
                "let _{} = runtime.execute_functional_node({:?}, {});",
                temp_name, functional_node_id, args_expr
            ));
            spans.push(GeneratedNodeSpan {
                document_id: document.id,
                graph_id: function.graph_id,
                node_id: *node_id,
                pin_id: None,
                file: path.to_string(),
                line: line_before,
                column: 1,
            });
        }
        BlueprintIrStatement::Return { node_id, value } => {
            let expr = value
                .as_ref()
                .map(value_expr)
                .unwrap_or_else(|| "BlueprintValue::Void".to_string());
            writer.push_line(format!("return {expr};"));
            spans.push(GeneratedNodeSpan {
                document_id: document.id,
                graph_id: function.graph_id,
                node_id: *node_id,
                pin_id: None,
                file: path.to_string(),
                line: line_before,
                column: 1,
            });
        }
    }
}

fn value_expr(value: &BlueprintIrValue) -> String {
    match value {
        BlueprintIrValue::StringLiteral { value, .. } => {
            format!("BlueprintValue::String({value:?}.to_string())")
        }
        BlueprintIrValue::Parameter { name, .. } => sanitize_ident(name.clone()),
        BlueprintIrValue::Variable {
            variable_id,
            variable_name,
            ..
        } => format!(
            "runtime.get_blueprint_variable({:?}, {:?})",
            variable_id.to_string(),
            variable_name
        ),
        BlueprintIrValue::Default(pin_type) => default_value_expr(*pin_type),
    }
}

fn default_value_expr(pin_type: BlueprintPinType) -> String {
    match pin_type {
        BlueprintPinType::Exec | BlueprintPinType::Void => "BlueprintValue::Void".to_string(),
        BlueprintPinType::Any => "BlueprintValue::Void".to_string(),
        BlueprintPinType::Bool => "BlueprintValue::Bool(false)".to_string(),
        BlueprintPinType::Int => "BlueprintValue::Int(0)".to_string(),
        BlueprintPinType::Float => "BlueprintValue::Float(0.0)".to_string(),
        BlueprintPinType::String => "BlueprintValue::String(String::new())".to_string(),
        BlueprintPinType::Color => "BlueprintValue::Color(String::new())".to_string(),
        BlueprintPinType::Array
        | BlueprintPinType::Vector
        | BlueprintPinType::HashSet
        | BlueprintPinType::HashMap => "BlueprintValue::Void".to_string(),
        BlueprintPinType::UiElementRef => "BlueprintValue::UiElementRef(String::new())".to_string(),
        BlueprintPinType::PageRef => "BlueprintValue::PageRef(String::new())".to_string(),
        BlueprintPinType::ApiRef => "BlueprintValue::ApiRef(String::new())".to_string(),
    }
}

fn sanitize_package_name(raw: &str) -> String {
    let sanitized = sanitize_ident(raw.replace(' ', "_"));
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "snappix_blueprint".to_string()
    } else {
        trimmed.to_string()
    }
}

fn sanitize_ident(raw: impl Into<String>) -> String {
    let raw = raw.into();
    let mut ident = String::new();
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() {
            ident.push(character.to_ascii_lowercase());
        } else {
            ident.push('_');
        }
    }
    let ident = ident.trim_matches('_').to_string();
    if ident.is_empty() {
        "blueprint_item".to_string()
    } else if ident.chars().next().unwrap_or('a').is_ascii_digit() {
        format!("bp_{ident}")
    } else {
        ident
    }
}

struct RustWriter {
    indent: usize,
    lines: Vec<String>,
}

impl RustWriter {
    fn new() -> Self {
        Self {
            indent: 0,
            lines: Vec::new(),
        }
    }

    fn push_line(&mut self, line: impl Into<String>) {
        let line = line.into();
        if line.is_empty() {
            self.lines.push(String::new());
            return;
        }

        self.lines
            .push(format!("{}{}", "    ".repeat(self.indent), line));
    }

    fn current_line(&self) -> usize {
        self.lines.len() + 1
    }

    fn finish(self) -> String {
        self.lines.join("\n")
    }
}
