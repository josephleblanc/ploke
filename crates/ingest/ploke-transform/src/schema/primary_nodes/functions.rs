use super::super::*;
pub struct FunctionNodeSchema {
    id: CozoField,
    name: CozoField,
    docstring: CozoField,
    span: CozoField,
    tracking_hash: CozoField,
    cfgs: CozoField,
    return_type_id: CozoField,
    body: CozoField,
    vis_kind: CozoField,
    vis_path: CozoField,
    module_id: CozoField,
}

pub static FUNCTION_NODE_SCHEMA: FunctionNodeSchema = FunctionNodeSchema {
    id: CozoField {
        st: "id",
        dv: "Uuid",
    },
    name: CozoField {
        st: "name",
        dv: "String",
    },
    docstring: CozoField {
        st: "docstring",
        dv: "String?",
    },
    span: CozoField {
        st: "span",
        dv: "[Int]",
    },
    tracking_hash: CozoField {
        st: "tracking_hash",
        dv: "Uuid",
    },
    cfgs: CozoField {
        st: "cfgs",
        dv: "[String]?",
    },
    return_type_id: CozoField {
        st: "return_type",
        dv: "Uuid?",
    },
    body: CozoField {
        st: "body",
        dv: "String?",
    },
    vis_kind: CozoField {
        st: "vis_kind",
        dv: "String",
    },
    vis_path: CozoField {
        st: "vis_path",
        dv: "[String]?",
    },
    module_id: CozoField {
        st: "module_id",
        dv: "Uuid",
    },
};

impl FunctionNodeSchema {
    pub fn id(&self) -> &str {
        self.id.st()
    }
    pub fn name(&self) -> &str {
        self.name.st()
    }
    pub fn id_schema(&self) -> impl Iterator<Item = char> {
        self.id.schema_str()
    }
    pub fn name_schema(&self) -> impl Iterator<Item = char> {
        self.name.schema_str()
    }
    pub fn docstring(&self) -> &str {
        self.docstring.st()
    }
    pub fn docstring_schema(&self) -> impl Iterator<Item = char> {
        self.docstring.schema_str()
    }
    pub fn span(&self) -> &str {
        self.span.st()
    }
    pub fn span_schema(&self) -> impl Iterator<Item = char> {
        self.span.schema_str()
    }
    pub fn tracking_hash(&self) -> &str {
        self.tracking_hash.st()
    }
    pub fn tracking_hash_schema(&self) -> impl Iterator<Item = char> {
        self.tracking_hash.schema_str()
    }
    pub fn cfgs(&self) -> &str {
        self.cfgs.st()
    }
    pub fn cfgs_schema(&self) -> impl Iterator<Item = char> {
        self.cfgs.schema_str()
    }
    pub fn return_type_id(&self) -> &str {
        self.return_type_id.st()
    }
    pub fn return_type_id_schema(&self) -> impl Iterator<Item = char> {
        self.return_type_id.schema_str()
    }
    pub fn body(&self) -> &str {
        self.body.st()
    }
    pub fn body_schema(&self) -> impl Iterator<Item = char> {
        self.body.schema_str()
    }
    pub fn vis_kind(&self) -> &str {
        self.vis_kind.st()
    }
    pub fn vis_kind_schema(&self) -> impl Iterator<Item = char> {
        self.vis_kind.schema_str()
    }
    pub fn vis_path(&self) -> &str {
        self.vis_path.st()
    }
    pub fn vis_path_schema(&self) -> impl Iterator<Item = char> {
        self.vis_path.schema_str()
    }
    pub fn module_id(&self) -> &str {
        self.module_id.st()
    }
    pub fn module_id_schema(&self) -> impl Iterator<Item = char> {
        self.module_id.schema_str()
    }
}

impl FunctionNodeSchema {
    /// Creates the relation schema ready to be registered in the cozo::Db using ":create
    /// <relation> { .. }"
    pub fn schema_create(&self, db: &cozo::Db<MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
        let create = self.schema_string();
        db.run_script(&create, BTreeMap::new(), ScriptMutability::Mutable)
    }
    pub fn schema_string(&self) -> String {
        let fields = [
            format!("{}: {}", self.id(), self.id.dv()),
            format!("{}: {}", self.name(), self.name.dv()),
            format!("{}: {}", self.docstring(), self.docstring.dv()),
            format!("{}: {}", self.span(), self.span.dv()),
            format!("{}: {}", self.tracking_hash(), self.tracking_hash.dv()),
            format!("{}: {}", self.cfgs(), self.cfgs.dv()),
            format!("{}: {}", self.return_type_id(), self.return_type_id.dv()),
            format!("{}: {}", self.body(), self.body.dv()),
            format!("{}: {}", self.vis_kind(), self.vis_kind.dv()),
            format!("{}: {}", self.vis_path(), self.vis_path.dv()),
            format!("{}: {}", self.module_id(), self.module_id.dv()),
        ];

        format!(":create function {{ {} }}", fields.join(", "))
    }
}
