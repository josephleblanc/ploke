use std::collections::BTreeMap;

use cozo::{Db, MemStorage};
use syn_parser::utils::LogStyle;

use crate::schema::{
    assoc_nodes::MethodNodeSchema,
    primary_nodes::{
        ConstNodeSchema, ImplNodeSchema, StaticNodeSchema, StructNodeSchema, TraitNodeSchema,
    },
    secondary_nodes::{
        AttributeNodeSchema, FieldNodeSchema, GenericConstNodeSchema, GenericLifetimeNodeSchema,
        GenericTypeNodeSchema, VariantNodeSchema,
    },
};

pub(crate) fn log_db_result(db_result: cozo::NamedRows) {
    log::info!(target: "db",
        "{} {:#?}",
        "  Db return: ".log_step(),
        db_result,
    );
}

pub(crate) fn create_field_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let field_schema = FieldNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &field_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_attribute_schema(
    db: &Db<MemStorage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let attribute_schema = AttributeNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &attribute_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_generic_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    create_generic_const_schema(db)?;
    create_generic_lifetime_schema(db)?;
    create_generic_type_schema(db)?;
    Ok(())
}

pub(crate) fn create_generic_type_schema(
    db: &Db<MemStorage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let generic_type_schema = GenericTypeNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &generic_type_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_generic_lifetime_schema(
    db: &Db<MemStorage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let generic_lifetime_schema = GenericLifetimeNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &generic_lifetime_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_generic_const_schema(
    db: &Db<MemStorage>,
) -> Result<(), Box<dyn std::error::Error>> {
    let generic_const_schema = GenericConstNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &generic_const_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_struct_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let struct_schema = StructNodeSchema::SCHEMA;
    let script_create = struct_schema.script_create();
    struct_schema.log_create_script();
    let db_result = db.run_script(
        &script_create,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_variant_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let variant_schema = VariantNodeSchema::SCHEMA;
    let script_create = variant_schema.script_create();
    variant_schema.log_create_script();
    let db_result = db.run_script(
        &script_create,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_impl_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let impl_schema = ImplNodeSchema::SCHEMA;
    let script_create = impl_schema.script_create();
    impl_schema.log_create_script();
    let db_result = db.run_script(
        &script_create,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_method_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let method_schema = MethodNodeSchema::SCHEMA;
    let script_create = method_schema.script_create();
    method_schema.log_create_script();
    let db_result = db.run_script(
        &script_create,
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_const_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let const_schema = ConstNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &const_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_static_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let static_schema = StaticNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &static_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}

pub(crate) fn create_trait_schema(db: &Db<MemStorage>) -> Result<(), Box<dyn std::error::Error>> {
    let trait_schema = TraitNodeSchema::SCHEMA;
    let db_result = db.run_script(
        &trait_schema.script_create(),
        BTreeMap::new(),
        cozo::ScriptMutability::Mutable,
    )?;
    log_db_result(db_result);
    Ok(())
}
