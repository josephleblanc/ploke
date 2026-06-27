//! Focused fixture for call-graph syntax forms that are absent from the larger parser fixtures.

pub fn dynamic_calls() -> i32 {
    let closure = || 7;
    let from_binding = (closure)();
    let from_literal = (|| 11)();
    from_binding + from_literal
}

pub fn local_target() -> i32 {
    3
}

pub fn call_crate_local_target() -> i32 {
    crate::local_target()
}

pub mod local_mod {
    pub fn nested_target() -> i32 {
        5
    }

    pub fn call_self_nested_target() -> i32 {
        self::nested_target()
    }
}

pub fn call_unqualified_local_target() -> i32 {
    local_target()
}

pub fn make_fn() -> fn() -> i32 {
    local_target
}

pub fn call_returned_function() -> i32 {
    make_fn()()
}

pub struct LocalAssoc;

impl LocalAssoc {
    pub fn make() -> Self {
        Self
    }

    pub fn call_self_make() -> Self {
        Self::make()
    }
}

pub fn call_local_assoc_make() -> LocalAssoc {
    LocalAssoc::make()
}

pub fn call_qualified_local_assoc_make() -> LocalAssoc {
    <LocalAssoc>::make()
}

pub mod import_targets {
    pub fn imported_target() -> i32 {
        21
    }

    pub fn globbed_target() -> i32 {
        34
    }
}

use import_targets as targets_alias;
use import_targets::imported_target as imported_alias;
use import_targets::*;
pub use import_targets::imported_target as reexported_target;

pub fn call_imported_alias_target() -> i32 {
    imported_alias()
}

pub fn call_glob_imported_target() -> i32 {
    globbed_target()
}

pub fn call_reexported_target() -> i32 {
    reexported_target()
}

pub fn call_imported_module_target() -> i32 {
    targets_alias::globbed_target()
}

impl LocalAssoc {
    pub fn instance_value(&self) -> i32 {
        55
    }
}

pub fn call_param_instance_method(value: LocalAssoc) -> i32 {
    value.instance_value()
}

pub fn call_typed_local_instance_method() -> i32 {
    let value: LocalAssoc = LocalAssoc;
    value.instance_value()
}

pub fn call_initialized_local_instance_method() -> i32 {
    let value = LocalAssoc;
    value.instance_value()
}

pub const fn assoc_const_value() -> i32 {
    89
}

pub struct AssocConstCarrier;

impl AssocConstCarrier {
    pub const IMPL_ASSOC_VALUE: i32 = assoc_const_value();
}

pub trait LocalAssocConstTrait {
    const TRAIT_ASSOC_VALUE: i32 = assoc_const_value();
}

pub struct TraitDispatchTarget;

pub trait LocalDispatchTrait {
    fn trait_value(&self) -> i32;
}

impl LocalDispatchTrait for TraitDispatchTarget {
    fn trait_value(&self) -> i32 {
        144
    }
}

pub fn call_param_trait_method(value: TraitDispatchTarget) -> i32 {
    value.trait_value()
}

pub fn call_typed_local_trait_method() -> i32 {
    let value: TraitDispatchTarget = TraitDispatchTarget;
    value.trait_value()
}

pub fn call_initialized_local_trait_method() -> i32 {
    let value = TraitDispatchTarget;
    value.trait_value()
}

pub fn call_parenthesized_local_target() -> i32 {
    (local_target)()
}

pub fn closure_body_call_is_not_outer_call_site() -> i32 {
    let closure = || local_target();
    closure()
}

pub fn async_block_call_is_not_outer_call_site() {
    let _future = async {
        local_target();
    };
}

pub trait LocalAssocFunctionTrait {
    fn trait_make() -> i32 {
        233
    }
}

pub struct TraitAssocFunctionTarget;

impl LocalAssocFunctionTrait for TraitAssocFunctionTarget {}

pub fn call_trait_associated_function() -> i32 {
    <TraitAssocFunctionTarget as LocalAssocFunctionTrait>::trait_make()
}

pub fn call_shadowed_local_target_binding() -> i32 {
    let local_target = || 377;
    local_target()
}

pub fn call_local_function_item_binding() -> i32 {
    let f = local_target;
    f()
}

pub fn call_typed_function_pointer_binding() -> i32 {
    let f: fn() -> i32 = local_target;
    f()
}

pub struct AmbiguousTraitTarget;

pub trait AmbiguousTraitOne {
    fn overlap(&self) -> i32;
}

pub trait AmbiguousTraitTwo {
    fn overlap(&self) -> i32;
}

impl AmbiguousTraitOne for AmbiguousTraitTarget {
    fn overlap(&self) -> i32 {
        1
    }
}

impl AmbiguousTraitTwo for AmbiguousTraitTarget {
    fn overlap(&self) -> i32 {
        2
    }
}

pub fn call_ambiguous_trait_method(value: AmbiguousTraitTarget) -> i32 {
    value.overlap()
}

pub struct InherentPrecedenceTarget;

pub trait InherentPrecedenceTrait {
    fn priority(&self) -> i32;
}

impl InherentPrecedenceTarget {
    pub fn priority(&self) -> i32 {
        10
    }
}

impl InherentPrecedenceTrait for InherentPrecedenceTarget {
    fn priority(&self) -> i32 {
        20
    }
}

pub fn call_inherent_over_trait_method() -> i32 {
    let value: InherentPrecedenceTarget = InherentPrecedenceTarget;
    value.priority()
}

pub trait GenericBoundTrait {
    fn bound_value(&self) -> i32;
}

pub fn call_inline_generic_bound_method<T: GenericBoundTrait>(value: T) -> i32 {
    value.bound_value()
}

pub fn call_where_generic_bound_method<T>(value: T) -> i32
where
    T: GenericBoundTrait,
{
    value.bound_value()
}

pub fn call_trait_object_method(value: &dyn GenericBoundTrait) -> i32 {
    value.bound_value()
}

pub trait TraitDefaultCall {
    fn default_calls_local(&self) -> i32 {
        local_target()
    }
}

pub fn call_parenthesized_function_item_binding() -> i32 {
    let f = local_target;
    (f)()
}

pub fn call_aliased_function_item_binding() -> i32 {
    let f = local_target;
    let g = f;
    g()
}

pub fn call_parenthesized_aliased_function_item_binding() -> i32 {
    let f = local_target;
    let g = f;
    (g)()
}

pub fn call_impl_trait_method(value: impl GenericBoundTrait) -> i32 {
    value.bound_value()
}

pub mod trait_scope {
    pub struct ScopedTraitTarget;

    pub mod traits {
        pub trait ScopedTrait {
            fn scoped_value(&self) -> i32;
        }
    }

    impl traits::ScopedTrait for ScopedTraitTarget {
        fn scoped_value(&self) -> i32 {
            610
        }
    }

    pub mod with_direct_import {
        use super::ScopedTraitTarget;
        use super::traits::ScopedTrait;

        pub fn call_direct_imported_trait_method(value: ScopedTraitTarget) -> i32 {
            value.scoped_value()
        }
    }

    pub mod with_alias_import {
        use super::ScopedTraitTarget;
        use super::traits::ScopedTrait as VisibleTrait;

        pub fn call_alias_imported_trait_method(value: ScopedTraitTarget) -> i32 {
            value.scoped_value()
        }
    }

    pub mod with_glob_import {
        use super::ScopedTraitTarget;
        use super::traits::*;

        pub fn call_glob_imported_trait_method(value: ScopedTraitTarget) -> i32 {
            value.scoped_value()
        }
    }

    pub mod without_trait_import {
        use super::ScopedTraitTarget;

        pub fn call_unimported_trait_method(value: ScopedTraitTarget) -> i32 {
            value.scoped_value()
        }
    }
}

pub fn call_parenthesized_typed_local_instance_method() -> i32 {
    let value: LocalAssoc = LocalAssoc;
    (value).instance_value()
}

pub fn call_parenthesized_initialized_local_trait_method() -> i32 {
    let value = TraitDispatchTarget;
    (value).trait_value()
}

pub mod assoc_import_targets {
    pub struct ImportedAssoc;

    impl ImportedAssoc {
        pub fn make() -> Self {
            Self
        }
    }
}

use assoc_import_targets::ImportedAssoc as ImportedAssocAlias;
use assoc_import_targets::*;
pub use assoc_import_targets::ImportedAssoc as ReexportedAssoc;

pub fn call_imported_type_assoc_make() -> ImportedAssocAlias {
    ImportedAssocAlias::make()
}

pub fn call_glob_imported_type_assoc_make() -> ImportedAssoc {
    ImportedAssoc::make()
}

pub fn call_reexported_type_assoc_make() -> ReexportedAssoc {
    ReexportedAssoc::make()
}

pub mod trait_assoc_import_targets {
    pub trait ImportedAssocFunctionTrait {
        fn imported_trait_make() -> i32 {
            987
        }
    }
}

pub mod trait_assoc_function_scope {
    pub mod with_direct_import {
        use super::super::trait_assoc_import_targets::ImportedAssocFunctionTrait;

        pub fn call_direct_imported_trait_associated_function() -> i32 {
            ImportedAssocFunctionTrait::imported_trait_make()
        }
    }

    pub mod with_alias_import {
        use super::super::trait_assoc_import_targets::ImportedAssocFunctionTrait as VisibleAssocFunctionTrait;

        pub fn call_alias_imported_trait_associated_function() -> i32 {
            VisibleAssocFunctionTrait::imported_trait_make()
        }
    }

    pub mod with_glob_import {
        use super::super::trait_assoc_import_targets::*;

        pub fn call_glob_imported_trait_associated_function() -> i32 {
            ImportedAssocFunctionTrait::imported_trait_make()
        }
    }
}

pub fn call_crate_module_nested_target() -> i32 {
    crate::local_mod::nested_target()
}

pub fn call_self_module_nested_target() -> i32 {
    self::local_mod::nested_target()
}

pub fn call_method_as_associated_function() -> i32 {
    let value = LocalAssoc;
    LocalAssoc::instance_value(&value)
}

pub fn call_typed_function_pointer_alias_binding() -> i32 {
    let f: fn() -> i32 = local_target;
    let g: fn() -> i32 = f;
    g()
}

pub fn call_parenthesized_typed_function_pointer_alias_binding() -> i32 {
    let f: fn() -> i32 = local_target;
    let g: fn() -> i32 = f;
    (g)()
}

pub fn call_generic_fn_once_value_binding<F>(generic_f: F) -> i32
where
    F: FnOnce() -> i32,
{
    generic_f()
}

pub fn call_boxed_dyn_fn_value_binding() -> i32 {
    let boxed_fn: Box<dyn Fn() -> i32> = Box::new(local_target);
    boxed_fn()
}

pub fn generic_identity<T>(value: T) -> T {
    value
}

pub fn call_generic_identity_turbofish() -> i32 {
    generic_identity::<i32>(123)
}

pub mod super_path_scope {
    pub fn call_super_local_target() -> i32 {
        super::local_target()
    }
}

pub fn r#match() -> i32 {
    2048
}

pub fn call_raw_identifier_function() -> i32 {
    r#match()
}

pub struct RawMethodTarget;

impl RawMethodTarget {
    pub fn r#type(&self) -> i32 {
        4096
    }
}

pub fn call_raw_identifier_method() -> i32 {
    let value: RawMethodTarget = RawMethodTarget;
    value.r#type()
}

pub struct GenericMethodTarget;

impl GenericMethodTarget {
    pub fn generic_instance<T>(&self, value: T) -> T {
        value
    }
}

pub fn call_method_turbofish() -> i32 {
    let value: GenericMethodTarget = GenericMethodTarget;
    value.generic_instance::<i32>(123)
}

pub fn call_prelude_drop_value() {
    let value = 1;
    drop(value)
}

#[macro_export]
macro_rules! crate_scoped_macro {
    () => {
        8192
    };
}

pub fn call_crate_scoped_macro() -> i32 {
    crate::crate_scoped_macro!()
}

pub mod prelude_shadow_scope {
    pub fn drop(_value: i32) -> i32 {
        9001
    }

    pub fn call_local_drop_shadow() -> i32 {
        drop(1)
    }
}

pub fn call_borrowed_typed_local_instance_method() -> i32 {
    let value: LocalAssoc = LocalAssoc;
    (&value).instance_value()
}

pub fn call_dereferenced_local_instance_method() -> i32 {
    let value = &LocalAssoc;
    (*value).instance_value()
}

pub fn call_prelude_string_new() -> String {
    String::new()
}

pub fn call_prelude_vec_new() -> Vec<i32> {
    Vec::new()
}

pub fn make_local_assoc() -> LocalAssoc {
    LocalAssoc
}

impl LocalAssoc {
    pub fn clone_assoc(&self) -> Self {
        LocalAssoc
    }
}

pub fn call_path_result_instance_method() -> i32 {
    make_local_assoc().instance_value()
}

pub fn call_method_result_instance_method() -> i32 {
    let value: LocalAssoc = LocalAssoc;
    value.clone_assoc().instance_value()
}

pub struct TupleFieldMethodReceiver(pub LocalAssoc);

pub fn call_tuple_field_instance_method() -> i32 {
    let value = TupleFieldMethodReceiver(LocalAssoc);
    value.0.instance_value()
}

pub struct TupleFieldFunction(pub fn() -> i32);

pub fn call_tuple_field_function() -> i32 {
    let value = TupleFieldFunction(local_target);
    value.0()
}

pub async fn make_ready_local_assoc() -> LocalAssoc {
    LocalAssoc
}

pub async fn call_await_result_instance_method() -> i32 {
    make_ready_local_assoc().await.instance_value()
}

pub fn try_local_assoc() -> Result<LocalAssoc, ()> {
    Ok(LocalAssoc)
}

pub fn call_try_result_instance_method() -> Result<i32, ()> {
    Ok(try_local_assoc()?.instance_value())
}

pub fn call_literal_str_to_string() -> String {
    "literal".to_string()
}

pub fn call_typed_vec_len_external() -> usize {
    let value: Vec<i32> = Vec::new();
    value.len()
}

pub mod local_prelude_shadow {
    pub struct Vec;

    impl Vec {
        pub fn len(&self) -> usize {
            0
        }
    }

    pub fn call_shadowed_typed_vec_len() -> usize {
        let value: Vec = Vec;
        value.len()
    }
}

pub fn call_function_pointer_cast_path() -> i32 {
    (local_target as fn() -> i32)()
}

pub fn call_function_pointer_cast_binding() -> i32 {
    let f: fn() -> i32 = local_target;
    (f as fn() -> i32)()
}

pub fn call_dereferenced_function_pointer_binding() -> i32 {
    let f: fn() -> i32 = local_target;
    (*f)()
}

pub fn call_block_function_item() -> i32 {
    ({ local_target })()
}

pub fn other_target() -> i32 {
    4
}

pub fn call_if_same_function_item(flag: bool) -> i32 {
    (if flag { local_target } else { local_target })()
}

pub fn call_if_ambiguous_function_item(flag: bool) -> i32 {
    (if flag { local_target } else { other_target })()
}

pub fn call_match_same_function_item(flag: bool) -> i32 {
    (match flag {
        true => local_target,
        false => local_target,
    })()
}

pub fn call_match_ambiguous_function_item(flag: bool) -> i32 {
    (match flag {
        true => local_target,
        false => other_target,
    })()
}

pub fn call_match_guarded_function_item(flag: bool) -> i32 {
    (match flag {
        true if flag => local_target,
        _ => local_target,
    })()
}

pub fn call_if_closure_branch(flag: bool) -> i32 {
    (if flag { local_target } else { || 8 })()
}

pub fn call_match_closure_arm(flag: bool) -> i32 {
    (match flag {
        true => local_target,
        false => || 13,
    })()
}

pub fn call_function_pointer_param(f: fn() -> i32) -> i32 {
    f()
}

pub fn call_parenthesized_function_pointer_param(f: fn() -> i32) -> i32 {
    (f)()
}

pub fn call_function_pointer_param_cast(f: fn() -> i32) -> i32 {
    (f as fn() -> i32)()
}

pub fn call_closure_binding_cast() -> i32 {
    let closure = || 21;
    (closure as fn() -> i32)()
}

pub fn call_dereferenced_closure_binding() -> i32 {
    let closure = || 34;
    (*closure)()
}

pub struct CallbackHolder {
    pub callback: fn() -> i32,
}

pub fn call_field_function_param(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

pub fn call_indexed_function_pointer(funcs: [fn() -> i32; 1]) -> i32 {
    funcs[0]()
}

pub fn call_move_closure_literal_with_body_call() -> i32 {
    (move || local_target())()
}

pub fn call_if_function_pointer_param_branch(flag: bool, f: fn() -> i32) -> i32 {
    (if flag { f } else { f })()
}

pub fn call_match_function_pointer_param_arm(flag: bool, f: fn() -> i32) -> i32 {
    (match flag {
        true => f,
        false => f,
    })()
}

pub fn call_if_nested_branch_expression(flag: bool) -> i32 {
    (if flag {
        if flag { local_target } else { local_target }
    } else {
        local_target
    })()
}

pub fn call_match_nested_arm_expression(flag: bool) -> i32 {
    (match flag {
        true => match flag {
            true => local_target,
            false => local_target,
        },
        false => local_target,
    })()
}

pub fn unary_target(value: i32) -> i32 {
    value
}

pub fn make_unary_fn() -> fn(i32) -> i32 {
    unary_target
}

pub fn call_chained_returned_function() -> i32 {
    make_unary_fn()(5)
}

pub fn call_vec_macro() -> Vec<i32> {
    vec![1, 2, 3]
}

pub unsafe fn unsafe_target() -> i32 {
    13
}

pub fn call_unsafe_function() -> i32 {
    unsafe { unsafe_target() }
}

pub struct NewType(pub i32);

pub fn call_new_type_constructor(value: i32) -> NewType {
    NewType(value)
}

pub trait DefaultRequiredCall {
    fn required(&self) -> i32;

    fn default_calls_required(&self) -> i32 {
        self.required()
    }
}

#[macro_export]
macro_rules! call_graph_exported_alias_macro {
    () => {
        21
    };
}

use crate::call_graph_exported_alias_macro as imported_macro_alias;

pub fn call_imported_macro_alias() -> i32 {
    imported_macro_alias!()
}

pub struct ExplicitDropTarget;

impl ExplicitDropTarget {
    pub fn drop(self) -> i32 {
        34
    }
}

pub fn call_explicit_drop_method() -> i32 {
    let value = ExplicitDropTarget;
    value.drop()
}

macro_rules! call_graph_item_macro {
    () => {
        fn generated_by_item_macro() -> i32 {
            55
        }
    };
}

pub fn call_item_macro_inside_body() -> i32 {
    call_graph_item_macro!();
    0
}

pub fn call_parenthesized_generic_fn_once_value_binding<F>(generic_f: F) -> i32
where
    F: FnOnce() -> i32,
{
    (generic_f)()
}

pub fn call_parenthesized_boxed_dyn_fn_value_binding() -> i32 {
    let boxed_fn: Box<dyn Fn() -> i32> = Box::new(local_target);
    (boxed_fn)()
}

#[link(name = "c")]
unsafe extern "C" {
    fn abs(input: i32) -> i32;
}

pub fn call_extern_c_function(value: i32) -> i32 {
    unsafe { abs(value) }
}

#[cfg(test)]
mod call_graph_tests {
    #[test]
    fn assert_eq_macro_call() {
        assert_eq!(1 + 1, 2);
    }
}

pub fn call_async_closure_literal_with_body_call() {
    let _future = (async || local_target())();
}

pub struct NamedCallbackHolder {
    pub callback: fn() -> i32,
}

pub struct CallbackArrayHolder {
    pub callbacks: [fn() -> i32; 1],
}

pub struct TupleCallbackArrayHolder(pub [fn() -> i32; 1]);

pub fn call_named_field_function_binding() -> i32 {
    let holder = NamedCallbackHolder {
        callback: local_target,
    };
    (holder.callback)()
}

pub fn call_aliased_named_field_function_binding() -> i32 {
    let holder = NamedCallbackHolder {
        callback: local_target,
    };
    let alias = holder;
    (alias.callback)()
}

pub fn call_indexed_field_function_param(holder: CallbackArrayHolder) -> i32 {
    holder.callbacks[0]()
}

pub fn call_indexed_named_field_function_binding() -> i32 {
    let holder = CallbackArrayHolder {
        callbacks: [local_target],
    };
    holder.callbacks[0]()
}

pub fn call_indexed_named_field_array_alias_binding() -> i32 {
    let funcs = [local_target];
    let holder = CallbackArrayHolder { callbacks: funcs };
    holder.callbacks[0]()
}

pub fn call_aliased_indexed_named_field_function_binding() -> i32 {
    let holder = CallbackArrayHolder {
        callbacks: [local_target],
    };
    let alias = holder;
    alias.callbacks[0]()
}

pub fn call_indexed_tuple_field_function_param(holder: TupleCallbackArrayHolder) -> i32 {
    holder.0[0]()
}

pub fn call_indexed_tuple_field_function_binding() -> i32 {
    let holder = TupleCallbackArrayHolder([local_target]);
    holder.0[0]()
}

pub fn call_indexed_tuple_field_array_alias_binding() -> i32 {
    let funcs = [local_target];
    let holder = TupleCallbackArrayHolder(funcs);
    holder.0[0]()
}

pub fn call_aliased_indexed_tuple_field_function_binding() -> i32 {
    let holder = TupleCallbackArrayHolder([local_target]);
    let alias = holder;
    alias.0[0]()
}

pub fn call_indexed_initialized_function_array() -> i32 {
    let funcs = [local_target];
    funcs[0]()
}

pub fn call_typed_indexed_initialized_function_array() -> i32 {
    let funcs: [fn() -> i32; 1] = [local_target];
    funcs[0]()
}

pub fn call_aliased_indexed_initialized_function_array() -> i32 {
    let funcs = [local_target];
    let alias = funcs;
    alias[0]()
}

pub mod trait_reexport_scope {
    pub use super::trait_scope::traits::ScopedTrait as ReexportedScopedTrait;
    use super::trait_scope::ScopedTraitTarget;
    use self::ReexportedScopedTrait;

    pub fn call_reexported_trait_method(value: ScopedTraitTarget) -> i32 {
        value.scoped_value()
    }
}

pub trait BlanketDispatchTrait {
    fn blanket_value(&self) -> i32;
}

pub struct BlanketDispatchTarget;

impl<T> BlanketDispatchTrait for T {
    fn blanket_value(&self) -> i32 {
        809
    }
}

pub fn call_blanket_trait_method(value: BlanketDispatchTarget) -> i32 {
    value.blanket_value()
}

pub trait TraitDefaultAssocCall {
    fn required_assoc() -> i32
    where
        Self: Sized;

    fn default_calls_assoc(&self) -> i32
    where
        Self: Sized,
    {
        Self::required_assoc()
    }
}

pub type LocalAssocTypeAlias = LocalAssoc;

pub fn call_type_alias_assoc_make() -> LocalAssocTypeAlias {
    LocalAssocTypeAlias::make()
}

pub type LocalAssocAliasChain = LocalAssocTypeAlias;

pub fn call_type_alias_chain_assoc_make() -> LocalAssocAliasChain {
    LocalAssocAliasChain::make()
}

pub fn call_type_alias_chain_instance_method() -> i32 {
    let value: LocalAssocAliasChain = LocalAssoc;
    value.instance_value()
}

pub mod type_alias_import_targets {
    pub type ImportedLocalAssocAlias = super::LocalAssoc;
}

use type_alias_import_targets::ImportedLocalAssocAlias;

pub fn call_imported_type_alias_assoc_make() -> ImportedLocalAssocAlias {
    ImportedLocalAssocAlias::make()
}

pub fn call_imported_type_alias_instance_method() -> i32 {
    let value: ImportedLocalAssocAlias = LocalAssoc;
    value.instance_value()
}

pub trait BlanketBound {}

impl BlanketBound for BlanketDispatchTarget {}

pub trait InlineBoundBlanketTrait {
    fn inline_bound_value(&self) -> i32;
}

impl<T: BlanketBound> InlineBoundBlanketTrait for T {
    fn inline_bound_value(&self) -> i32 {
        901
    }
}

pub fn call_inline_bound_blanket_trait_method(value: BlanketDispatchTarget) -> i32 {
    value.inline_bound_value()
}

pub trait WhereBoundBlanketTrait {
    fn where_bound_value(&self) -> i32;
}

impl<T> WhereBoundBlanketTrait for T
where
    T: BlanketBound,
{
    fn where_bound_value(&self) -> i32 {
        902
    }
}

pub fn call_where_bound_blanket_trait_method(value: BlanketDispatchTarget) -> i32 {
    value.where_bound_value()
}

pub fn call_dereferenced_param_instance_method(value: &LocalAssoc) -> i32 {
    (*value).instance_value()
}

pub fn call_borrowed_param_instance_method(value: &LocalAssoc) -> i32 {
    value.instance_value()
}

pub fn call_referenced_local_instance_method() -> i32 {
    let value = &LocalAssoc;
    value.instance_value()
}

pub fn call_typed_reference_local_instance_method() -> i32 {
    let value: &LocalAssoc = &LocalAssoc;
    value.instance_value()
}

pub fn call_local_trait_object_binding_method(input: &dyn GenericBoundTrait) -> i32 {
    let value: &dyn GenericBoundTrait = input;
    value.bound_value()
}

pub trait TraitImplBodyCallTrait {
    fn required_impl_call(&self) -> i32;

    fn impl_calls_required(&self) -> i32;
}

impl TraitImplBodyCallTrait for TraitDispatchTarget {
    fn required_impl_call(&self) -> i32 {
        233
    }

    fn impl_calls_required(&self) -> i32 {
        self.required_impl_call()
    }
}

pub fn call_concrete_trait_object_binding_method() -> i32 {
    let value: &dyn LocalDispatchTrait = &TraitDispatchTarget;
    value.trait_value()
}

pub fn call_aliased_concrete_trait_object_binding_method() -> i32 {
    let source = TraitDispatchTarget;
    let value: &dyn LocalDispatchTrait = &source;
    value.trait_value()
}

pub trait TransitiveBaseBound {}

impl TransitiveBaseBound for BlanketDispatchTarget {}

pub trait TransitiveDerivedBound {}

impl<T: TransitiveBaseBound> TransitiveDerivedBound for T {}

pub trait TransitiveBoundBlanketTrait {
    fn transitive_bound_value(&self) -> i32;
}

impl<T: TransitiveDerivedBound> TransitiveBoundBlanketTrait for T {
    fn transitive_bound_value(&self) -> i32 {
        904
    }
}

pub fn call_transitive_bound_blanket_trait_method(value: BlanketDispatchTarget) -> i32 {
    value.transitive_bound_value()
}

pub fn call_imported_function_item_binding() -> i32 {
    let f = imported_alias;
    f()
}

pub fn call_reference_alias_trait_object_binding_method() -> i32 {
    let source = TraitDispatchTarget;
    let alias = &source;
    let value: &dyn LocalDispatchTrait = alias;
    value.trait_value()
}

pub fn call_reference_chain_trait_object_binding_method() -> i32 {
    let source = TraitDispatchTarget;
    let first = &source;
    let second = first;
    let value: &dyn LocalDispatchTrait = second;
    value.trait_value()
}

pub mod grouped_trait_import_scope {
    use super::trait_scope::{ScopedTraitTarget, traits::ScopedTrait};

    pub fn call_grouped_imported_trait_method(value: ScopedTraitTarget) -> i32 {
        value.scoped_value()
    }
}

pub mod trait_assoc_reexport_scope {
    pub use super::trait_assoc_import_targets::ImportedAssocFunctionTrait as ReexportedAssocFunctionTrait;
    use self::ReexportedAssocFunctionTrait;

    pub fn call_reexported_trait_associated_function() -> i32 {
        ReexportedAssocFunctionTrait::imported_trait_make()
    }
}

pub struct GenericWrapper<T>(pub T);

pub struct GenericBoundValue;

pub trait GenericWrapperBound {}

impl GenericWrapperBound for GenericBoundValue {}

pub trait ConstrainedGenericSelfTrait {
    fn constrained_generic_self_value(&self) -> i32;
}

impl<T: GenericWrapperBound> ConstrainedGenericSelfTrait for GenericWrapper<T> {
    fn constrained_generic_self_value(&self) -> i32 {
        905
    }
}

pub fn call_constrained_generic_self_trait_method(value: GenericWrapper<GenericBoundValue>) -> i32 {
    value.constrained_generic_self_value()
}

pub mod grouped_function_import_scope {
    use super::import_targets::{
        globbed_target as grouped_globbed_alias, imported_target as grouped_alias,
    };

    pub fn call_grouped_imported_alias_target() -> i32 {
        grouped_alias()
    }

    pub fn call_grouped_imported_globbed_target() -> i32 {
        grouped_globbed_alias()
    }
}

pub mod grouped_trait_assoc_function_scope {
    use super::trait_assoc_import_targets::{
        ImportedAssocFunctionTrait as GroupedAssocFunctionTrait,
    };

    pub fn call_grouped_imported_trait_associated_function() -> i32 {
        GroupedAssocFunctionTrait::imported_trait_make()
    }
}

pub fn call_typed_double_reference_local_instance_method() -> i32 {
    let value: &&LocalAssoc = &&LocalAssoc;
    value.instance_value()
}
