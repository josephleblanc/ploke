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
    BorrowedLocalBinding {
        name: String,
    },
    BorrowedTypedLocalBinding {
        name: String,
        type_path: Vec<String>,
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
    TryResult,
    TryPathCallResult {
        path: Vec<String>,
    },
    Literal,
}
