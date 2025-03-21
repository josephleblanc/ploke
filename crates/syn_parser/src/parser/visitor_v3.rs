use crate::parser::TypeId;
use cozo::DataValue;
use cozo::Db;
use cozo::MemStorage;
use cozo::UuidWrapper;
use quote::ToTokens;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use syn::{
    visit::{self, Visit},
    ItemFn, ReturnType,
};
use uuid::Uuid;

use super::visitor::VisitorState;

// AI NOTE: I'm including this just to show what the VisitorState looks like, but I'm not making it a
// `VisitorStateV2` because I want to use all the methods from the original without replicating
// them here.
// pub(crate) struct VisitorState {
//     pub(crate) code_graph: CodeGraph,
//     next_node_id: NodeId,
//     next_type_id: TypeId,
//     // Maps existing types to their IDs to avoid duplication
//     type_map: HashMap<String, TypeId>,
// }

// Represents a generic parameter
#[derive(Debug, Serialize, Deserialize)]
pub struct GenericParamNode {
    pub id: NodeId,
    pub kind: GenericParamKind,
}

// Different kinds of generic parameters
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum GenericParamKind {
    Type {
        name: String,
        bounds: Vec<TypeId>,
        default: Option<TypeId>,
    },
    Lifetime {
        name: String,
        bounds: Vec<String>,
    },
    Const {
        name: String,
        type_id: TypeId,
    },
}

// Represents a parameter in a function
#[derive(Debug, Serialize, Deserialize)]
pub struct ParameterNode {
    pub id: NodeId,
    pub name: Option<String>,
    pub type_id: TypeId,
    pub is_mutable: bool,
    pub is_self: bool,
}
// Represents a macro rule
#[derive(Debug, Serialize, Deserialize)]
pub struct MacroRuleNode {
    pub id: NodeId,
    pub pattern: String,
    pub expansion: String,
}
// Represent an attribute
#[derive(Debug, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,          // e.g., "derive", "cfg", "serde"
    pub args: Vec<String>,     // Arguments or parameters of the attribute
    pub value: Option<String>, // Optional value (e.g., for `#[attr = "value"]`)
}

pub type NodeId = usize;

// Different kinds of visibility
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VisibilityKind {
    Public,
    Crate,
    Restricted(Vec<String>), // Path components of restricted visibility
    Inherited,               // Default visibility
}
// Different kinds of relations
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationKind {
    FunctionParameter,
    FunctionReturn,
    StructField,
    EnumVariant,
    ImplementsFor,
    ImplementsTrait,
    Inherits,
    References,
    Contains,
    Uses,
    ValueType,
    MacroUse,
    // MacroExpansion,
    // This is outside the scope of this project right now, but if it were to be implemented, it
    // would probably go here.
}
// Represents a relation between nodes
#[derive(Debug, Serialize, Deserialize)]
pub struct Relation {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: RelationKind,
}
// Different kinds of procedural macros
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcMacroKind {
    Derive,
    Attribute,
    Function,
}
// Represents a function definition
#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub parameters: Vec<ParameterNode>,
    pub return_type: Option<TypeId>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MacroKind {
    DeclarativeMacro,
    ProcedureMacro { kind: ProcMacroKind },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MacroNode {
    pub id: NodeId,
    pub name: String,
    pub visibility: VisibilityKind,
    pub kind: MacroKind,
    pub rules: Vec<MacroRuleNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
}

struct TypeRegistry {
    type_map: HashMap<String, uuid::Uuid>,
}

impl TypeRegistry {
    fn get_or_create(&mut self, ty: &syn::Type) -> Uuid {
        let type_name = ty.to_token_stream().to_string();
        *self.type_map.entry(type_name).or_insert_with(|| {
            let id = Uuid::new_v4();
            // Store in types table
            id
        })
    }
}

struct CodeVisitorV2<'a> {
    db: &'a Db<MemStorage>,
    // #MaybeSomeday
    // current_scope: Vec<cozo::DataValuee::parser::visitor_v2::DataValueadb::util::Component>, // Module/block stack
    type_registry: TypeRegistry,
    batches: BTreeMap<&'static str, Vec<DataValue>>,
}

impl<'a> Visit<'a> for CodeVisitorV2<'a> {
    fn visit_item_fn(&mut self, item: &'a ItemFn) {
        let fn_id = Uuid::new_v4();
        let return_type = self.process_type(&item.sig.output);

        // Node entry
        self.batch_push(
            "nodes",
            vec![
                fn_id.into(),
                "function".into(),
                item.sig.ident.to_string().into(),
                self.current_vis().into(),
                attrs_to_json(&item.attrs).into(),
                extract_docs(&item.attrs).into(),
                span_location(item).into(),
            ],
        );

        // Type relationships
        if let Some(ret_id) = return_type {
            self.batch_push(
                "ast_edges",
                vec![
                    fn_id.into(),
                    ret_id.into(),
                    "returns".into(),
                    json!({ "optional": matches!(item.sig.output, ReturnType::Default) }).into(),
                ],
            );
        }

        // Process parameters
        for input in &item.sig.inputs {
            if let syn::FnArg::Typed(pat) = input {
                let param_id = Uuid::new_v4();
                let ty_id = self.process_type(&pat.ty);

                self.batch_push(
                    "nodes",
                    vec![
                        param_id.into(),
                        "parameter".into(),
                        pat.pat.to_token_stream().to_string().into(),
                        "".into(), // No visibility for params
                        json!([]).into(),
                        "".into(),
                        span_location(pat).into(),
                    ],
                );

                self.batch_push("ast_edges", vec![
                    fn_id.into(),
                    param_id.into(),
                    "has_param".into(),
                    json!({ "mutable": matches!(pat.pat, Pat::Ident(pi) if pi.mutability.is_some()) }).into()
                ]);
            }
        }
    }
}

impl<'a> CodeAnalyzer<'a> {
    pub fn new(db: &'a Db<MemStorage>) -> Self {
        Self {
            db,
            current_module: vec![],
            buffer: BTreeMap::from([
                ("nodes", Vec::with_capacity(1000)),
                ("relations", Vec::with_capacity(2000)),
                ("types", Vec::with_capacity(500)),
            ]),
            type_cache: HashMap::new(),
        }
    }

    fn flush(&mut self) {
        let queries = self.buffer.iter().map(|(table, rows)| {
            let params = BTreeMap::from([("rows".to_string(), DataValue::List(rows.clone()))]);
            format!(
                "?[id, kind, name, visibility, attributes, docs, span, module_path] <- $rows\n:put {}",
                table
            )
        });

        self.db
            .run_script(
                &queries.collect::<Vec<_>>().join("\n"),
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .expect("Failed to flush buffers");

        self.buffer.values_mut().for_each(|v| v.clear());
    }

    fn track_type(&mut self, ty: &syn::Type) -> String {
        let type_str = ty.to_token_stream().to_string();
        if !self.type_cache.contains_key(&type_str) {
            let type_id = Uuid::new_v5(/* ... */).to_string();
            self.buffer
                .entry("types")
                .or_default()
                .push(DataValue::List(vec![
                    /* ... type fields ... */
                ]));
            self.type_cache.insert(type_str.clone(), type_id);
        }
        self.type_cache[&type_str].clone()
    }
}

impl Visit<'_> for CodeAnalyzer<'_> {
    fn visit_item_fn(&mut self, item: &ItemFn) {
        let fn_id = generate_id(item);
        let module_path = self.current_module.join("::");

        // Capture function metadata
        self.buffer
            .entry("nodes")
            .or_default()
            .push(DataValue::List(vec![
                /* ... node fields ... */
            ]));

        // Track parameters
        for param in &item.sig.inputs {
            let param_id = generate_id(param);
            self.buffer.entry("nodes").or_default().push(/* ... */);
            self.buffer
                .entry("relations")
                .or_default()
                .push(DataValue::List(vec![
                    fn_id.clone().into(),
                    param_id.into(),
                    "has_parameter".into(),
                    DataValue::Null,
                ]));
        }

        if self.buffer.values().map(|v| v.len()).sum::<usize>() > 1000 {
            self.flush();
        }
    }

    // Similar implementations for other item types
}

fn generate_id<T: ToTokens>(item: &T) -> String {
    let hash = sha256(item.to_token_stream().to_string());
    Uuid::new_v5(&UUID_NS, &hash).to_string()
}
impl CodeAnalyzer<'_> {
    pub fn parse(&mut self, syntax: &File) {
        let mut count = 0;
        self.visit_file(syntax);

        // Periodic flushing based on parse progress
        while count % 100 == 0 {
            self.flush();
            count += 1;
        }
    }
}
