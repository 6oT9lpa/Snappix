pub mod api;
pub mod catalog;
pub mod codegen;
pub mod compile;
pub mod lowering;
pub mod model;
pub mod validate;

pub use api::{
    BlueprintProjectApi, PageApiDescriptor, ServerApiDescriptor, UiActionDescriptor,
    UiElementApiDescriptor, UiEventDescriptor,
};
pub use catalog::{
    builtin_node_catalog, builtin_node_descriptor, is_builtin_node_descriptor,
    BlueprintNodeContext, BlueprintNodeDescriptor, BlueprintPinDescriptor,
};
pub use codegen::{
    generate_project, BlueprintSourceMap, GeneratedBlueprintProject, GeneratedFile,
    GeneratedNodeSpan,
};
pub use compile::{compile_project, BlueprintCompilationResult};
pub use lowering::{
    lower_project, BlueprintIrDocument, BlueprintIrFunction, BlueprintIrFunctionTrigger,
    BlueprintIrProject, BlueprintIrStatement, BlueprintIrValue,
};
pub use model::{
    blueprint_name_for_page, BlueprintDocument, BlueprintDocumentKind, BlueprintExport,
    BlueprintFunctionParameter, BlueprintFunctionSignature, BlueprintFunctionTarget,
    BlueprintGraph, BlueprintGraphKind, BlueprintLink, BlueprintLocalVariable, BlueprintNode,
    BlueprintNodeKind, BlueprintNumericPolicy, BlueprintNumericPromotion, BlueprintNumericWidth,
    BlueprintOwner, BlueprintPin, BlueprintPinDirection, BlueprintPinKind, BlueprintPinType,
    BlueprintPoint, LogicData,
};
pub use validate::{validate_project, BlueprintDiagnostic, BlueprintDiagnosticSeverity};
