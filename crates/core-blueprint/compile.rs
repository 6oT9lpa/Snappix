use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::api::BlueprintProjectApi;
use crate::codegen::{generate_project, BlueprintSourceMap, GeneratedFile, GeneratedNodeSpan};
use crate::lowering::lower_project;
use crate::model::BlueprintDocument;
use crate::validate::{BlueprintDiagnostic, BlueprintDiagnosticSeverity};

#[derive(Debug, Clone)]
pub struct BlueprintCompilationResult {
    pub success: bool,
    pub output_dir: PathBuf,
    pub generated_files: Vec<GeneratedFile>,
    pub source_map: BlueprintSourceMap,
    pub diagnostics: Vec<BlueprintDiagnostic>,
}

pub fn compile_project(
    project_name: &str,
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
    output_dir: impl AsRef<Path>,
) -> BlueprintCompilationResult {
    let output_dir = output_dir.as_ref().to_path_buf();
    let ir = match lower_project(documents, api) {
        Ok(ir) => ir,
        Err(diagnostics) => {
            return BlueprintCompilationResult {
                success: false,
                output_dir,
                generated_files: Vec::new(),
                source_map: BlueprintSourceMap::default(),
                diagnostics,
            };
        }
    };

    let generated = generate_project(project_name, &ir);
    if let Err(err) = write_generated_project(&output_dir, &generated.files) {
        return BlueprintCompilationResult {
            success: false,
            output_dir,
            generated_files: Vec::new(),
            source_map: BlueprintSourceMap::default(),
            diagnostics: vec![BlueprintDiagnostic {
                severity: BlueprintDiagnosticSeverity::Error,
                code: "write_failed".to_string(),
                message: format!("Failed to write generated project files: {err}"),
                document_id: None,
                graph_id: None,
                node_id: None,
                pin_id: None,
                file: None,
                line: None,
                column: None,
            }],
        };
    }
    let diagnostics = cargo_check_diagnostics(&output_dir, &generated.source_map);
    let success = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != BlueprintDiagnosticSeverity::Error);

    BlueprintCompilationResult {
        success,
        output_dir,
        generated_files: generated.files,
        source_map: generated.source_map,
        diagnostics,
    }
}

fn write_generated_project(
    output_dir: &Path,
    files: &[GeneratedFile],
) -> Result<(), std::io::Error> {
    if output_dir.exists() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(output_dir)?;

    for file in files {
        let path = output_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &file.contents)?;
    }

    Ok(())
}

fn cargo_check_diagnostics(
    output_dir: &Path,
    source_map: &BlueprintSourceMap,
) -> Vec<BlueprintDiagnostic> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--message-format=json")
        .current_dir(output_dir)
        .output();

    let Ok(output) = output else {
        return vec![BlueprintDiagnostic {
            severity: BlueprintDiagnosticSeverity::Error,
            code: "cargo_unavailable".to_string(),
            message: "Failed to launch cargo check for generated blueprint project.".to_string(),
            document_id: None,
            graph_id: None,
            node_id: None,
            pin_id: None,
            file: None,
            line: None,
            column: None,
        }];
    };

    let mut diagnostics = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let span_lookup = build_source_lookup(source_map);

    for line in stdout.lines().chain(stderr.lines()) {
        let Ok(parsed) = serde_json::from_str::<CargoMessage>(line) else {
            continue;
        };
        if parsed.reason != "compiler-message" {
            continue;
        }
        let Some(message) = parsed.message else {
            continue;
        };

        let severity = match message.level.as_str() {
            "error" => BlueprintDiagnosticSeverity::Error,
            "warning" => BlueprintDiagnosticSeverity::Warning,
            _ => continue,
        };

        let primary_span = message.spans.iter().find(|span| span.is_primary);
        let normalized_file = primary_span
            .map(|span| normalize_path(span.file_name.as_str()))
            .unwrap_or_default();
        let mapped = primary_span.and_then(|span| {
            span_lookup.get(&(normalize_path(span.file_name.as_str()), span.line_start))
        });

        diagnostics.push(BlueprintDiagnostic {
            severity,
            code: message.code.and_then(|code| code.code).unwrap_or_default(),
            message: message.rendered.unwrap_or(message.message),
            document_id: mapped.map(|span| span.document_id),
            graph_id: mapped.map(|span| span.graph_id),
            node_id: mapped.map(|span| span.node_id),
            pin_id: mapped.and_then(|span| span.pin_id),
            file: if normalized_file.is_empty() {
                None
            } else {
                Some(normalized_file)
            },
            line: primary_span.map(|span| span.line_start),
            column: primary_span.map(|span| span.column_start),
        });
    }

    if !output.status.success() {
        diagnostics.push(BlueprintDiagnostic {
            severity: BlueprintDiagnosticSeverity::Error,
            code: "cargo_check_failed".to_string(),
            message: "cargo check failed for generated blueprint project.".to_string(),
            document_id: None,
            graph_id: None,
            node_id: None,
            pin_id: None,
            file: None,
            line: None,
            column: None,
        });
    }

    diagnostics
}

fn build_source_lookup(source_map: &BlueprintSourceMap) -> HashMap<(String, usize), &GeneratedNodeSpan> {
    let mut lookup = HashMap::new();
    for span in &source_map.spans {
        lookup.insert((normalize_path(&span.file), span.line), span);
    }
    lookup
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    message: Option<CargoCompilerMessage>,
}

#[derive(Debug, Deserialize)]
struct CargoCompilerMessage {
    code: Option<CargoDiagnosticCode>,
    level: String,
    message: String,
    rendered: Option<String>,
    #[serde(default)]
    spans: Vec<CargoMessageSpan>,
}

#[derive(Debug, Deserialize)]
struct CargoDiagnosticCode {
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMessageSpan {
    file_name: String,
    line_start: usize,
    column_start: usize,
    is_primary: bool,
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use uuid::Uuid;

    use crate::api::{
        BlueprintProjectApi, PageApiDescriptor, ServerApiDescriptor, UiActionDescriptor,
        UiElementApiDescriptor, UiEventDescriptor,
    };
    use crate::model::{
        BlueprintDocument, BlueprintFunctionParameter, BlueprintFunctionSignature,
        BlueprintFunctionTarget, BlueprintGraph, BlueprintGraphKind, BlueprintLink, BlueprintNode,
        BlueprintPinType,
    };

    use super::compile_project;

    fn page_api(
        page_id: Uuid,
        elements: Vec<UiElementApiDescriptor>,
        server_exports: Vec<crate::model::BlueprintExport>,
    ) -> BlueprintProjectApi {
        BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements,
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor {
                exported_functions: server_exports,
            },
        }
    }

    #[test]
    fn compiles_button_click_set_text_graph() {
        let page_id = Uuid::new_v4();
        let button_id = Uuid::new_v4();
        let label_id = Uuid::new_v4();

        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let event = BlueprintNode::ui_event(button_id, "clicked");
        let set_text = BlueprintNode::set_element_text(label_id);
        let literal = BlueprintNode::literal_string("Hello from blueprint");

        document.graphs[0].entrypoints = vec![event.id];
        document.graphs[0].nodes = vec![event.clone(), set_text.clone(), literal.clone()];
        document.graphs[0].links = vec![
            BlueprintLink::new(
                event.id,
                event.pin_named("then").expect("event exec").id,
                set_text.id,
                set_text.pin_named("in").expect("set text exec").id,
            ),
            BlueprintLink::new(
                literal.id,
                literal.pin_named("value").expect("literal value").id,
                set_text.id,
                set_text.pin_named("text").expect("set text value").id,
            ),
        ];

        let api = page_api(
            page_id,
            vec![
                UiElementApiDescriptor {
                    element_id: button_id,
                    display_name: "button".to_string(),
                    element_type: "button".to_string(),
                    events: vec![UiEventDescriptor {
                        name: "clicked".to_string(),
                        display_name: "Clicked".to_string(),
                    }],
                    actions: Vec::new(),
                },
                UiElementApiDescriptor {
                    element_id: label_id,
                    display_name: "label".to_string(),
                    element_type: "label".to_string(),
                    events: Vec::new(),
                    actions: vec![UiActionDescriptor {
                        name: "set_text".to_string(),
                        display_name: "Set Text".to_string(),
                        parameters: vec![BlueprintFunctionParameter {
                            name: "text".to_string(),
                            data_type: BlueprintPinType::String,
                        }],
                        return_type: BlueprintPinType::Void,
                    }],
                },
            ],
            Vec::new(),
        );

        let output_dir = tempdir().expect("temp dir");
        let result = compile_project(
            "snappix_blueprint_test",
            &[document, BlueprintDocument::new_server()],
            &api,
            output_dir.path(),
        );

        assert!(
            result.success,
            "expected compile success, diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result
            .source_map
            .spans
            .iter()
            .any(|span| span.node_id == set_text.id));
    }

    #[test]
    fn page_blueprint_can_call_server_export() {
        let page_id = Uuid::new_v4();
        let button_id = Uuid::new_v4();
        let signature = BlueprintFunctionSignature {
            name: "notify_server".to_string(),
            parameters: vec![BlueprintFunctionParameter {
                name: "text".to_string(),
                data_type: BlueprintPinType::String,
            }],
            return_type: BlueprintPinType::Void,
            is_public: true,
        };

        let mut page_document = BlueprintDocument::new_page(page_id, "Main");
        let event = BlueprintNode::ui_event(button_id, "clicked");
        let literal = BlueprintNode::literal_string("Ping");
        let call = BlueprintNode::call_document_function(
            BlueprintFunctionTarget::Server,
            signature.clone(),
        );
        page_document.graphs[0].entrypoints = vec![event.id];
        page_document.graphs[0].nodes = vec![event.clone(), literal.clone(), call.clone()];
        page_document.graphs[0].links = vec![
            BlueprintLink::new(
                event.id,
                event.pin_named("then").expect("event exec").id,
                call.id,
                call.pin_named("in").expect("call exec").id,
            ),
            BlueprintLink::new(
                literal.id,
                literal.pin_named("value").expect("literal value").id,
                call.id,
                call.pin_named("text").expect("call arg").id,
            ),
        ];

        let mut server_document = BlueprintDocument::new_server();
        let entry = BlueprintNode::function_entry(signature.clone());
        let result = BlueprintNode::function_result(BlueprintPinType::Void);
        let mut graph = BlueprintGraph::new("notify_server", BlueprintGraphKind::FunctionGraph);
        graph.nodes = vec![entry.clone(), result.clone()];
        graph.links = vec![BlueprintLink::new(
            entry.id,
            entry.pin_named("then").expect("entry exec").id,
            result.id,
            result.pin_named("in").expect("result exec").id,
        )];
        server_document.graphs.push(graph);
        server_document.sync_exports();

        let api = page_api(
            page_id,
            vec![UiElementApiDescriptor {
                element_id: button_id,
                display_name: "button".to_string(),
                element_type: "button".to_string(),
                events: vec![UiEventDescriptor {
                    name: "clicked".to_string(),
                    display_name: "Clicked".to_string(),
                }],
                actions: Vec::new(),
            }],
            server_document.exports.clone(),
        );

        let output_dir = tempdir().expect("temp dir");
        let result = compile_project(
            "snappix_server_call_test",
            &[page_document, server_document],
            &api,
            output_dir.path(),
        );

        assert!(
            result.success,
            "expected compile success, diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result
            .generated_files
            .iter()
            .any(|file| file.path.ends_with("runtime.rs")));
    }
}
