use std::collections::BTreeMap;

use crate::database::Database;
use crate::error::DbError;
use crate::multi_embedding::schema::metadata::ExperimentalRelationSchemaDbExt;
use crate::multi_embedding::schema::node_specs::{
    ExperimentalNodeRelationSpec, EXPERIMENTAL_NODE_RELATION_SPECS,
};
use crate::multi_embedding::schema::vector_dims::{
    embedding_entry, vector_dimension_specs, VectorDimensionSpec,
};
use crate::multi_embedding::vectors::ExperimentalVectorRelation;
use cozo::{DataValue, Db, MemStorage, NamedRows, Num, ScriptMutability, UuidWrapper};
use uuid::Uuid;

pub(crate) struct SampleNodeData {
    pub(crate) node_id: Uuid,
    pub(crate) params: BTreeMap<String, DataValue>,
}

pub(crate) struct ExperimentalNodeSpec {
    pub(crate) base: &'static ExperimentalNodeRelationSpec,
    pub(crate) sample_builder: fn() -> SampleNodeData,
}

pub(crate) const EXPERIMENTAL_NODE_SPECS: &[ExperimentalNodeSpec] = &[
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[0],
        sample_builder: sample_function_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[1],
        sample_builder: sample_const_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[2],
        sample_builder: sample_enum_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[3],
        sample_builder: sample_impl_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[4],
        sample_builder: sample_import_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[5],
        sample_builder: sample_macro_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[6],
        sample_builder: sample_module_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[7],
        sample_builder: sample_static_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[8],
        sample_builder: sample_struct_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[9],
        sample_builder: sample_trait_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[10],
        sample_builder: sample_type_alias_params,
    },
    ExperimentalNodeSpec {
        base: &EXPERIMENTAL_NODE_RELATION_SPECS[11],
        sample_builder: sample_union_params,
    },
];

pub(crate) fn seed_metadata_relation(
    spec: &ExperimentalNodeSpec,
) -> Result<(Database, SampleNodeData), DbError> {
    let db = init_db();
    spec.base.metadata_schema.ensure_registered(&db)?;
    let sample = (spec.sample_builder)();
    insert_metadata_sample(&db, spec, &sample)?;
    Ok((db, sample))
}

pub(crate) fn seed_vector_relation_for_node<'a>(
    db: &'a Database,
    spec: &'a ExperimentalNodeSpec,
    node_id: Uuid,
    dim_spec: &'a VectorDimensionSpec,
) -> Result<ExperimentalVectorRelation<'a>, DbError> {
    let vector_relation =
        ExperimentalVectorRelation::new(dim_spec.dims(), spec.base.vector_relation_base);
    vector_relation.ensure_registered(db)?;
    vector_relation.insert_row(db, node_id, dim_spec)?;
    Ok(vector_relation)
}

pub(crate) fn insert_metadata_sample(
    db: &Database,
    spec: &ExperimentalNodeSpec,
    sample: &SampleNodeData,
) -> Result<(), DbError> {
    let insert_script = spec.base.metadata_schema.script_put(&sample.params);
    db.run_script(
        &insert_script,
        sample.params.clone(),
        ScriptMutability::Mutable,
    )
    .map_err(|err| DbError::ExperimentalScriptFailure {
        action: "metadata_insert",
        relation: spec.base.metadata_schema.relation().to_string(),
        details: err.to_string(),
    })
    .map(|_| ())
}

pub(crate) fn init_db() -> Database {
    let db = Db::new(MemStorage::default()).expect("create db");
    db.initialize().expect("init db");
    Database::new(db)
}

fn metadata_embeddings() -> DataValue {
    DataValue::List(
        vector_dimension_specs()
            .iter()
            .map(|spec| embedding_entry(spec.embedding_model(), spec.dims()))
            .collect(),
    )
}

fn sample_span() -> DataValue {
    DataValue::List(vec![
        DataValue::Num(Num::Int(0)),
        DataValue::Num(Num::Int(42)),
    ])
}

fn cfg_list(values: &[&str]) -> DataValue {
    DataValue::List(values.iter().map(|v| DataValue::Str((*v).into())).collect())
}

fn string_list(values: &[&str]) -> DataValue {
    DataValue::List(values.iter().map(|v| DataValue::Str((*v).into())).collect())
}

fn sample_function_params() -> SampleNodeData {
    let function_id = Uuid::new_v4();
    let module_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(function_id)));
    params.insert(
        "name".into(),
        DataValue::Str("experimental_function".into()),
    );
    params.insert("docstring".into(), DataValue::Null);
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("span".into(), sample_span());
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("return_type_id".into(), DataValue::Null);
    params.insert("body".into(), DataValue::Null);
    params.insert("module_id".into(), DataValue::Uuid(UuidWrapper(module_id)));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: function_id,
        params,
    }
}

fn sample_const_params() -> SampleNodeData {
    let const_id = Uuid::new_v4();
    let ty_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(const_id)));
    params.insert("name".into(), DataValue::Str("CONST_VALUE".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("ty_id".into(), DataValue::Uuid(UuidWrapper(ty_id)));
    params.insert("value".into(), DataValue::Str("42".into()));
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: const_id,
        params,
    }
}

fn sample_enum_params() -> SampleNodeData {
    let enum_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(enum_id)));
    params.insert("name".into(), DataValue::Str("ExampleEnum".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), DataValue::Null);
    params.insert(
        "variants".into(),
        DataValue::List(vec![DataValue::Uuid(UuidWrapper(Uuid::new_v4()))]),
    );
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: enum_id,
        params,
    }
}

fn sample_impl_params() -> SampleNodeData {
    let impl_id = Uuid::new_v4();
    let self_type = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(impl_id)));
    params.insert("self_type".into(), DataValue::Uuid(UuidWrapper(self_type)));
    params.insert("span".into(), sample_span());
    params.insert("trait_type".into(), DataValue::Null);
    params.insert("methods".into(), DataValue::Null);
    params.insert("cfgs".into(), cfg_list(&["default", "test"]));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: impl_id,
        params,
    }
}

fn sample_import_params() -> SampleNodeData {
    let import_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(import_id)));
    params.insert("name".into(), DataValue::Str("some::module".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Null);
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert(
        "source_path".into(),
        string_list(&["crate", "module", "Item"]),
    );
    params.insert("visible_name".into(), DataValue::Str("Item".into()));
    params.insert("original_name".into(), DataValue::Null);
    params.insert("is_glob".into(), DataValue::Bool(false));
    params.insert("is_self_import".into(), DataValue::Bool(false));
    params.insert("import_kind".into(), DataValue::Str("use".into()));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: import_id,
        params,
    }
}

fn sample_macro_params() -> SampleNodeData {
    let macro_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(macro_id)));
    params.insert("name".into(), DataValue::Str("macro_rules!".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert("body".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("kind".into(), DataValue::Str("macro_rules".into()));
    params.insert("proc_kind".into(), DataValue::Null);
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: macro_id,
        params,
    }
}

fn sample_module_params() -> SampleNodeData {
    let module_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(module_id)));
    params.insert("name".into(), DataValue::Str("example".into()));
    params.insert(
        "path".into(),
        DataValue::List(vec![DataValue::Str("example".into())]),
    );
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert("span".into(), sample_span());
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("module_kind".into(), DataValue::Str("mod".into()));
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: module_id,
        params,
    }
}

fn sample_static_params() -> SampleNodeData {
    let static_id = Uuid::new_v4();
    let ty_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(static_id)));
    params.insert("name".into(), DataValue::Str("GLOBAL".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("ty_id".into(), DataValue::Uuid(UuidWrapper(ty_id)));
    params.insert("is_mutable".into(), DataValue::Bool(false));
    params.insert("value".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: static_id,
        params,
    }
}

fn sample_struct_params() -> SampleNodeData {
    let struct_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(struct_id)));
    params.insert("name".into(), DataValue::Str("ExampleStruct".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: struct_id,
        params,
    }
}

fn sample_trait_params() -> SampleNodeData {
    let trait_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(trait_id)));
    params.insert("name".into(), DataValue::Str("ExampleTrait".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("methods".into(), DataValue::Null);
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: trait_id,
        params,
    }
}

fn sample_type_alias_params() -> SampleNodeData {
    let alias_id = Uuid::new_v4();
    let ty_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(alias_id)));
    params.insert("name".into(), DataValue::Str("Alias".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("ty_id".into(), DataValue::Uuid(UuidWrapper(ty_id)));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: alias_id,
        params,
    }
}

fn sample_union_params() -> SampleNodeData {
    let union_id = Uuid::new_v4();
    let mut params = BTreeMap::new();
    params.insert("id".into(), DataValue::Uuid(UuidWrapper(union_id)));
    params.insert("name".into(), DataValue::Str("ExampleUnion".into()));
    params.insert("span".into(), sample_span());
    params.insert("vis_kind".into(), DataValue::Str("public".into()));
    params.insert("vis_path".into(), DataValue::Null);
    params.insert("docstring".into(), DataValue::Null);
    params.insert(
        "tracking_hash".into(),
        DataValue::Uuid(UuidWrapper(Uuid::new_v4())),
    );
    params.insert("cfgs".into(), cfg_list(&["default"]));
    params.insert("embeddings".into(), metadata_embeddings());

    SampleNodeData {
        node_id: union_id,
        params,
    }
}
