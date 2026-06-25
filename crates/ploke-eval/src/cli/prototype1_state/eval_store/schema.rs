use std::collections::BTreeMap;

use cozo::DataValue;

use super::{cozo_schema::eval_relation_exists, cozo_store::EvalDb, error::EvalStoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvalSchemaField {
    name: &'static str,
    ty: &'static str,
}

impl EvalSchemaField {
    pub(crate) const fn new(name: &'static str, ty: &'static str) -> Self {
        Self { name, ty }
    }

    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    const fn ty(self) -> &'static str {
        self.ty
    }
}

pub(crate) trait EvalRelationSchema {
    fn relation(&self) -> &'static str;
    fn key_fields(&self) -> &'static [EvalSchemaField];
    fn value_fields(&self) -> &'static [EvalSchemaField];

    fn all_fields(&self) -> Vec<EvalSchemaField> {
        self.key_fields()
            .iter()
            .chain(self.value_fields().iter())
            .copied()
            .collect()
    }

    fn script_create(&self) -> String {
        format!(
            ":create {} {{ {} => {} }}",
            self.relation(),
            field_defs(self.key_fields()),
            field_defs(self.value_fields())
        )
    }

    fn script_put(&self, params: &BTreeMap<String, DataValue>) -> String {
        let fields = self.all_fields();
        let missing: Vec<_> = fields
            .iter()
            .map(|field| field.name())
            .filter(|field| !params.contains_key(*field))
            .collect();
        debug_assert!(
            missing.is_empty(),
            "eval-store params are missing fields for relation {}: {:?}",
            self.relation(),
            missing
        );
        format!(
            "?[{}] <- [[{}]] :put {} {{ {} => {} }}",
            join_names(&fields),
            param_refs(&fields),
            self.relation(),
            join_names(self.key_fields()),
            join_names(self.value_fields())
        )
    }

    fn log_create_script(&self, script: &str) {
        tracing::trace!(
            target: "eval_store",
            relation = self.relation(),
            script,
            "creating eval relation"
        );
    }

    fn log_put_script(&self, script: &str) {
        tracing::trace!(
            target: "eval_store",
            relation = self.relation(),
            script,
            "putting eval row"
        );
    }

    fn ensure_installed<D: EvalDb + ?Sized>(
        &self,
        db: &D,
        phase: &'static str,
    ) -> Result<(), EvalStoreError> {
        if !eval_relation_exists(db, self.relation())? {
            let script = self.script_create();
            self.log_create_script(&script);
            db.eval_query_mut_params(&script, BTreeMap::new())
                .map_err(|source| EvalStoreError::Db { phase, source })?;
        }
        Ok(())
    }
}

pub(crate) trait EvalRow {
    type Schema: EvalRelationSchema + 'static;

    const PHASE: &'static str;

    fn schema() -> &'static Self::Schema;
    fn params(&self) -> BTreeMap<String, DataValue>;
}

pub(crate) fn put_eval_row<D, R>(db: &D, row: &R) -> Result<(), EvalStoreError>
where
    D: EvalDb + ?Sized,
    R: EvalRow,
{
    let schema = R::schema();
    let params = row.params();
    let script = schema.script_put(&params);
    schema.log_put_script(&script);
    db.eval_query_mut_params(&script, params)
        .map_err(|source| EvalStoreError::Db {
            phase: R::PHASE,
            source,
        })?;
    Ok(())
}

fn field_defs(fields: &[EvalSchemaField]) -> String {
    fields
        .iter()
        .map(|field| format!("{}: {}", field.name(), field.ty()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_names(fields: &[EvalSchemaField]) -> String {
    fields
        .iter()
        .map(|field| field.name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn param_refs(fields: &[EvalSchemaField]) -> String {
    fields
        .iter()
        .map(|field| format!("${}", field.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

macro_rules! define_eval_schema {
    ($schema_name:ident {
        $relation:literal,
        $($key_name:ident: $key_ty:literal),+ $(,)? =>
        $($value_name:ident: $value_ty:literal),+ $(,)?
    }) => {
        #[derive(Debug, Clone, Copy)]
        pub(super) struct $schema_name {
            relation: &'static str,
            $($key_name: $crate::cli::prototype1_state::eval_store::schema::EvalSchemaField,)+
            $($value_name: $crate::cli::prototype1_state::eval_store::schema::EvalSchemaField,)+
        }

        impl $schema_name {
            pub(super) const RELATION: &'static str = $relation;
            pub(super) const KEY_FIELDS: &'static [$crate::cli::prototype1_state::eval_store::schema::EvalSchemaField] = &[
                $($crate::cli::prototype1_state::eval_store::schema::EvalSchemaField::new(stringify!($key_name), $key_ty)),+
            ];
            pub(super) const VALUE_FIELDS: &'static [$crate::cli::prototype1_state::eval_store::schema::EvalSchemaField] = &[
                $($crate::cli::prototype1_state::eval_store::schema::EvalSchemaField::new(stringify!($value_name), $value_ty)),+
            ];
            pub(super) const SCHEMA: Self = Self {
                relation: $relation,
                $($key_name: $crate::cli::prototype1_state::eval_store::schema::EvalSchemaField::new(stringify!($key_name), $key_ty),)+
                $($value_name: $crate::cli::prototype1_state::eval_store::schema::EvalSchemaField::new(stringify!($value_name), $value_ty),)+
            };

            $(pub(super) fn $key_name(&self) -> &'static str {
                self.$key_name.name()
            })+

            $(pub(super) fn $value_name(&self) -> &'static str {
                self.$value_name.name()
            })+
        }

        impl $crate::cli::prototype1_state::eval_store::schema::EvalRelationSchema for $schema_name {
            fn relation(&self) -> &'static str {
                self.relation
            }

            fn key_fields(&self) -> &'static [$crate::cli::prototype1_state::eval_store::schema::EvalSchemaField] {
                Self::KEY_FIELDS
            }

            fn value_fields(&self) -> &'static [$crate::cli::prototype1_state::eval_store::schema::EvalSchemaField] {
                Self::VALUE_FIELDS
            }
        }
    };
}

pub(crate) use define_eval_schema;
