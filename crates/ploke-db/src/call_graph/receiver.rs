use serde::{Deserialize, Serialize};

mod decode;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallReceiver {
    SelfValue,
    SelfField {
        path: Vec<String>,
    },
    LocalBinding {
        name: String,
    },
    TypedLocalBinding {
        name: String,
        type_path: Vec<String>,
    },
    InitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    AliasedLocalBinding {
        name: String,
        source_path: Vec<String>,
    },
    TupleReturnBinding {
        name: String,
        path: Vec<String>,
        index: usize,
    },
    TupleMethodReturn {
        name: String,
        method_name: String,
        method_span: (usize, usize),
        index: usize,
    },
    MethodResultLocalBinding {
        name: String,
        method_name: String,
        method_span: (usize, usize),
    },
    BorrowedLocalBinding {
        name: String,
    },
    BorrowedTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
    },
    BorrowedInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    DereferencedLocalBinding {
        name: String,
    },
    DereferencedInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
    },
    FieldLocalBinding {
        name: String,
        field_path: Vec<String>,
    },
    FieldTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
        field_path: Vec<String>,
    },
    FieldInitializedLocalBinding {
        name: String,
        init_path: Vec<String>,
        field_path: Vec<String>,
    },
    PathCallResult {
        path: Vec<String>,
    },
    MethodCallResult {
        method_name: String,
    },
    AwaitResult,
    AwaitPathCallResult {
        path: Vec<String>,
    },
    AwaitMethodCallResult {
        method_name: String,
    },
    TryResult,
    TryPathCallResult {
        path: Vec<String>,
    },
    TryMethodCallResult {
        method_name: String,
    },
    IfBranchPaths {
        paths: Vec<Vec<String>>,
    },
    Literal,
    Unsupported,
}
