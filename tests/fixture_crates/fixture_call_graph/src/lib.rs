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

pub fn recursive_fixture_call(depth: u8) -> u8 {
    if depth == 0 {
        0
    } else {
        recursive_fixture_call(depth - 1)
    }
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

pub struct SelfFieldAssocOwner {
    value: LocalAssoc,
}

impl SelfFieldAssocOwner {
    pub fn call_self_field_instance_method(&self) -> i32 {
        self.value.instance_value()
    }
}

pub mod file_mod;

pub fn call_crate_file_module_target() -> i32 {
    crate::file_mod::file_module_target()
}

pub trait TraitImplAssocMakeTrait {
    fn trait_impl_calls_inherent_make() -> LocalAssoc;
}

impl TraitImplAssocMakeTrait for LocalAssoc {
    fn trait_impl_calls_inherent_make() -> LocalAssoc {
        Self::make()
    }
}

pub enum EnumWithInherentImpl {
    Case(i32),
}

impl EnumWithInherentImpl {
    pub fn helper() -> i32 {
        7
    }
}

pub fn call_enum_variant_with_inherent_impl(value: i32) -> EnumWithInherentImpl {
    EnumWithInherentImpl::Case(value)
}

pub mod qualified_assoc_scope {
    pub struct NestedAssoc;

    impl NestedAssoc {
        pub fn make() -> Self {
            Self
        }
    }
}

pub mod qualified_assoc_callers {
    pub fn call_super_qualified_nested_assoc_make() -> super::qualified_assoc_scope::NestedAssoc {
        super::qualified_assoc_scope::NestedAssoc::make()
    }
}

pub trait TraitMethodPath {
    fn handle(self) -> i32;
}

pub struct TraitMethodPathTarget;

impl TraitMethodPath for TraitMethodPathTarget {
    fn handle(self) -> i32 {
        610
    }
}

pub fn call_trait_method_as_path(value: TraitMethodPathTarget) -> i32 {
    TraitMethodPath::handle(value)
}

pub trait GenericAssocPathTrait {
    fn make(value: i32) -> i32;
}

pub fn call_inline_generic_bound_assoc_path<T: GenericAssocPathTrait>() -> i32 {
    T::make(7)
}

pub fn call_where_generic_bound_assoc_path<T>() -> i32
where
    T: GenericAssocPathTrait,
{
    T::make(11)
}

pub enum AliasConstructorEnum {
    Case(i32),
}

pub type AliasConstructorType = AliasConstructorEnum;

pub fn call_type_alias_enum_variant_constructor(value: i32) -> AliasConstructorType {
    AliasConstructorType::Case(value)
}

pub fn call_if_initialized_function_item_binding(flag: bool) -> i32 {
    let f = if flag { local_target } else { local_target };
    f()
}

pub fn call_parenthesized_match_initialized_function_item_binding(flag: bool) -> i32 {
    let f = match flag {
        true => local_target,
        false => local_target,
    };
    (f)()
}

pub fn call_if_ambiguous_initialized_function_item_binding(flag: bool) -> i32 {
    let f: fn() -> i32 = if flag { local_target } else { other_target };
    f()
}

pub mod external_type_alias_target {
    pub type ImportedExternalVec = std::vec::Vec<i32>;
}

use external_type_alias_target::ImportedExternalVec;

pub fn call_imported_external_type_alias_constructor() -> ImportedExternalVec {
    ImportedExternalVec::new()
}

pub fn call_imported_external_type_alias_initialized_method() -> usize {
    let value = ImportedExternalVec::new();
    value.len()
}

pub mod nested_glob_assoc_source {
    pub struct NestedGlobAssoc;

    impl NestedGlobAssoc {
        pub fn make() -> Self {
            Self
        }
    }
}

pub mod nested_glob_assoc_reexport {
    pub use super::nested_glob_assoc_source::*;
}

pub mod nested_glob_assoc_scope {
    use super::nested_glob_assoc_reexport::*;

    pub fn call_nested_glob_reexported_type_assoc_make() -> NestedGlobAssoc {
        NestedGlobAssoc::make()
    }
}

pub fn call_block_initialized_function_item_binding() -> i32 {
    let f = { local_target };
    f()
}

pub fn call_parenthesized_block_initialized_function_item_binding() -> i32 {
    let f = { local_target };
    (f)()
}

pub fn call_if_expression_receiver_method(flag: bool) -> i32 {
    (if flag { LocalAssoc } else { LocalAssoc }).instance_value()
}

pub fn local_const_initializer_call_is_not_outer_call_site() -> i32 {
    const LOCAL_INITIALIZER_VALUE: i32 = assoc_const_value();
    LOCAL_INITIALIZER_VALUE
}

pub mod direct_reexport_assoc_scope {
    use super::nested_glob_assoc_reexport::NestedGlobAssoc;

    pub fn call_direct_reexported_type_assoc_make() -> NestedGlobAssoc {
        NestedGlobAssoc::make()
    }
}

pub fn call_borrowed_initialized_local_instance_method() -> i32 {
    let value = LocalAssoc;
    (&value).instance_value()
}

pub fn call_qualified_dyn_any_downcast_mut(mut value: Option<i32>) {
    let erased: &mut dyn std::any::Any = &mut value;
    let _ = <dyn std::any::Any>::downcast_mut::<Option<i32>>(erased);
}

pub struct SelfTupleConstructor(pub i32);

impl SelfTupleConstructor {
    pub fn make(value: i32) -> Self {
        Self(value)
    }
}

pub mod inherited_glob_assoc_parent {
    use super::nested_glob_assoc_reexport::*;

    pub mod child {
        use super::*;

        pub fn call_inherited_glob_reexported_type_assoc_make() -> NestedGlobAssoc {
            NestedGlobAssoc::make()
        }
    }
}

impl SelfFieldAssocOwner {
    pub fn call_self_field_method_result_instance_method(&self) -> i32 {
        self.value.clone_assoc().instance_value()
    }
}

pub fn call_initialized_local_alias_instance_method() -> i32 {
    let source = LocalAssoc;
    let value = source;
    value.instance_value()
}

pub fn make_closure() -> impl Fn() -> i32 {
    || 71
}

pub fn call_returned_closure() -> i32 {
    make_closure()()
}

pub fn local_static_initializer_call_is_not_outer_call_site() -> i32 {
    static LOCAL_STATIC_VALUE: i32 = assoc_const_value();
    LOCAL_STATIC_VALUE
}

pub fn local_fn_body_call_is_not_outer_call_site() -> i32 {
    fn inner() -> i32 {
        assoc_const_value()
    }

    inner()
}

pub fn make_bound_closure() -> impl Fn() -> i32 {
    let closure = || 73;
    closure
}

pub fn call_returned_bound_closure() -> i32 {
    make_bound_closure()()
}

pub fn local_impl_method_body_call_is_not_outer_call_site() -> i32 {
    struct LocalImpl;

    impl LocalImpl {
        fn value(&self) -> i32 {
            assoc_const_value()
        }
    }

    let value = LocalImpl;
    value.value()
}

pub fn call_local_impl_where_bound_trait_associated_function() -> i32 {
    struct LocalImpl;

    impl LocalImpl
    where
        TraitAssocFunctionTarget: LocalAssocFunctionTrait,
    {
        fn value(&self) -> i32 {
            TraitAssocFunctionTarget::trait_make()
        }
    }

    let value = LocalImpl;
    value.value()
}

pub fn call_external_param_vec_len(value: Vec<i32>) -> usize {
    value.len()
}

pub fn call_external_borrowed_param_vec_len(value: &Vec<i32>) -> usize {
    value.len()
}

pub fn make_alias_bound_closure() -> impl Fn() -> i32 {
    let closure = || 79;
    let alias = closure;
    alias
}

pub fn call_returned_alias_bound_closure() -> i32 {
    make_alias_bound_closure()()
}

fn call_single_function_pointer_param(f: fn() -> i32) -> i32 {
    f()
}

pub fn call_single_function_pointer_param_with_local_target() -> i32 {
    call_single_function_pointer_param(local_target)
}

fn call_single_generic_fn_once_param<F>(generic_f: F) -> i32
where
    F: FnOnce() -> i32,
{
    generic_f()
}

pub fn call_single_generic_fn_once_param_with_local_target() -> i32 {
    call_single_generic_fn_once_param(local_target)
}

fn call_single_parenthesized_function_pointer_param(f: fn() -> i32) -> i32 {
    (f)()
}

pub fn call_single_parenthesized_function_pointer_param_with_local_target() -> i32 {
    call_single_parenthesized_function_pointer_param(local_target)
}

pub async fn call_awaited_async_closure_literal_with_body_call() {
    (async || local_target())().await;
}

fn call_single_indexed_field_function_param(holder: CallbackArrayHolder) -> i32 {
    holder.callbacks[0]()
}

pub fn call_single_indexed_field_function_param_with_local_target() -> i32 {
    call_single_indexed_field_function_param(CallbackArrayHolder { callbacks: [local_target] })
}

fn call_single_indexed_tuple_field_function_param(holder: TupleCallbackArrayHolder) -> i32 {
    holder.0[0]()
}

pub fn call_single_indexed_tuple_field_function_param_with_local_target() -> i32 {
    call_single_indexed_tuple_field_function_param(TupleCallbackArrayHolder([local_target]))
}

pub fn call_tuple_pattern_local_instance_method() -> i32 {
    let (value, _) = (LocalAssoc, 0);
    value.instance_value()
}

pub fn make_local_assoc_pair() -> (LocalAssoc, i32) {
    (LocalAssoc, 0)
}

pub fn call_typed_tuple_pattern_local_instance_method() -> i32 {
    let (value, _): (LocalAssoc, i32) = make_local_assoc_pair();
    value.instance_value()
}

fn call_single_function_pointer_param_cast(f: fn() -> i32) -> i32 {
    (f as fn() -> i32)()
}

pub fn call_single_function_pointer_param_cast_with_local_target() -> i32 {
    call_single_function_pointer_param_cast(local_target)
}

pub fn local_fn_forward_call_resolves_local_item() -> i32 {
    let value = inner();

    fn inner() -> i32 {
        assoc_const_value()
    }

    value
}

pub fn call_borrowed_value_param_instance_method(value: LocalAssoc) -> i32 {
    (&value).instance_value()
}

pub fn call_borrowed_value_param_method_result_instance_method(value: LocalAssoc) -> i32 {
    (&value).clone_assoc().instance_value()
}

fn call_single_named_field_function_param(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

pub fn call_single_named_field_function_param_with_local_target() -> i32 {
    call_single_named_field_function_param(CallbackHolder {
        callback: local_target,
    })
}

pub fn call_match_expression_receiver_method(flag: bool) -> i32 {
    (match flag {
        true => LocalAssoc,
        false => LocalAssoc,
    })
    .instance_value()
}

impl LocalAssoc {
    pub fn try_clone_assoc(&self) -> Result<LocalAssoc, ()> {
        Ok(LocalAssoc)
    }

    pub fn try_instance_value(&self) -> Result<i32, ()> {
        Ok(55)
    }
}

pub fn call_try_method_result_instance_method() -> Result<i32, ()> {
    let value: LocalAssoc = LocalAssoc;
    value.try_clone_assoc()?.try_instance_value()
}

pub fn call_tuple_return_pattern_local_instance_method() -> i32 {
    let (value, _) = make_local_assoc_pair();
    value.instance_value()
}

fn call_single_indexed_function_pointer_param(funcs: [fn() -> i32; 1]) -> i32 {
    funcs[0]()
}

pub fn call_single_indexed_function_pointer_param_with_local_target() -> i32 {
    call_single_indexed_function_pointer_param([local_target])
}

pub struct NestedSelfFieldAssocOwner {
    inner: SelfFieldAssocOwner,
}

impl NestedSelfFieldAssocOwner {
    pub fn call_nested_self_field_instance_method(&self) -> i32 {
        self.inner.value.instance_value()
    }
}

fn call_single_if_function_pointer_param_branch(flag: bool, f: fn() -> i32) -> i32 {
    (if flag { f } else { f })()
}

pub fn call_single_if_function_pointer_param_branch_with_local_target(flag: bool) -> i32 {
    call_single_if_function_pointer_param_branch(flag, local_target)
}

fn call_single_match_function_pointer_param_arm(flag: bool, f: fn() -> i32) -> i32 {
    (match flag {
        true => f,
        false => f,
    })()
}

pub fn call_single_match_function_pointer_param_arm_with_local_target(flag: bool) -> i32 {
    call_single_match_function_pointer_param_arm(flag, local_target)
}

fn call_single_aliased_function_pointer_param(f: fn() -> i32) -> i32 {
    let g = f;
    g()
}

pub fn call_single_aliased_function_pointer_param_with_local_target() -> i32 {
    call_single_aliased_function_pointer_param(local_target)
}

fn call_single_parenthesized_aliased_function_pointer_param(f: fn() -> i32) -> i32 {
    let g = f;
    (g)()
}

pub fn call_single_parenthesized_aliased_function_pointer_param_with_local_target() -> i32 {
    call_single_parenthesized_aliased_function_pointer_param(local_target)
}

pub fn call_borrowed_concrete_trait_object_binding_method() -> i32 {
    let value: &dyn LocalDispatchTrait = &TraitDispatchTarget;
    (&value).trait_value()
}

pub fn call_dereferenced_boxed_dyn_fn_value_binding() -> i32 {
    let boxed_fn: Box<dyn Fn() -> i32> = Box::new(local_target);
    (*boxed_fn)()
}

pub fn call_async_closure_binding_without_await_with_body_call() {
    let closure = async || local_target();
    closure();
}

pub async fn call_awaited_async_closure_binding_with_body_call() {
    let closure = async || local_target();
    closure().await;
}

pub fn call_async_closure_future_binding_without_await_with_body_call() {
    let closure = async || local_target();
    let _future = closure();
}

pub async fn call_awaited_async_closure_future_binding_with_body_call() {
    let closure = async || local_target();
    let future = closure();
    future.await;
}

pub async fn call_awaited_async_closure_future_alias_with_body_call() {
    let closure = async || local_target();
    let future = closure();
    let alias = future;
    alias.await;
}

fn call_single_parenthesized_generic_fn_once_param<F>(generic_f: F) -> i32
where
    F: FnOnce() -> i32,
{
    (generic_f)()
}

pub fn call_single_parenthesized_generic_fn_once_param_with_local_target() -> i32 {
    call_single_parenthesized_generic_fn_once_param(local_target)
}

fn call_multi_function_pointer_param(f: fn() -> i32) -> i32 {
    f()
}

pub fn call_multi_function_pointer_param_with_local_target_a() -> i32 {
    call_multi_function_pointer_param(local_target)
}

pub fn call_multi_function_pointer_param_with_local_target_b() -> i32 {
    call_multi_function_pointer_param(local_target)
}

fn call_multi_conflicting_function_pointer_param(f: fn() -> i32) -> i32 {
    f()
}

pub fn call_multi_conflicting_function_pointer_param_with_local_target() -> i32 {
    call_multi_conflicting_function_pointer_param(local_target)
}

pub fn call_multi_conflicting_function_pointer_param_with_other_target() -> i32 {
    call_multi_conflicting_function_pointer_param(other_target)
}

fn call_multi_generic_fn_once_param<F>(generic_f: F) -> i32
where
    F: FnOnce() -> i32,
{
    generic_f()
}

pub fn call_multi_generic_fn_once_param_with_local_target_a() -> i32 {
    call_multi_generic_fn_once_param(local_target)
}

pub fn call_multi_generic_fn_once_param_with_local_target_b() -> i32 {
    call_multi_generic_fn_once_param(local_target)
}

fn call_multi_conflicting_generic_fn_once_param<F>(generic_f: F) -> i32
where
    F: FnOnce() -> i32,
{
    generic_f()
}

pub fn call_multi_conflicting_generic_fn_once_param_with_local_target() -> i32 {
    call_multi_conflicting_generic_fn_once_param(local_target)
}

pub fn call_multi_conflicting_generic_fn_once_param_with_other_target() -> i32 {
    call_multi_conflicting_generic_fn_once_param(other_target)
}

impl LocalAssoc {
    pub fn tuple_pair(&self) -> (LocalAssoc, i32) {
        (LocalAssoc, 0)
    }
}

pub fn call_method_tuple_return_pattern_local_instance_method() -> i32 {
    let value = LocalAssoc;
    let (next, _) = value.tuple_pair();
    next.instance_value()
}

pub struct ParamFieldMethodReceiver {
    pub value: LocalAssoc,
}

pub fn call_param_field_instance_method(holder: ParamFieldMethodReceiver) -> i32 {
    holder.value.instance_value()
}

pub fn call_match_arm_initialized_receiver_method(flag: bool) -> i32 {
    match LocalAssoc {
        value if flag && value.instance_value() > 0 => value.instance_value(),
        _ => 0,
    }
}

pub async fn call_awaited_async_closure_future_block_alias_with_body_call() {
    let closure = async || local_target();
    let future = closure();
    let alias = { future };
    alias.await;
}

pub fn call_match_struct_pattern_initialized_receiver_method() -> i32 {
    match (ParamFieldMethodReceiver { value: LocalAssoc }) {
        ParamFieldMethodReceiver { value } => value.instance_value(),
    }
}

pub fn call_if_initialized_local_instance_method(flag: bool) -> i32 {
    let value = if flag { LocalAssoc } else { LocalAssoc };
    value.instance_value()
}

pub fn call_match_initialized_local_instance_method(flag: bool) -> i32 {
    let value = match flag {
        true => LocalAssoc,
        false => LocalAssoc,
    };
    value.instance_value()
}

fn call_multi_named_field_function_param(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

pub fn call_multi_named_field_function_param_with_local_target_a() -> i32 {
    call_multi_named_field_function_param(CallbackHolder {
        callback: local_target,
    })
}

pub fn call_multi_named_field_function_param_with_local_target_b() -> i32 {
    call_multi_named_field_function_param(CallbackHolder {
        callback: local_target,
    })
}

fn call_multi_conflicting_named_field_function_param(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

pub fn call_multi_conflicting_named_field_function_param_with_local_target() -> i32 {
    call_multi_conflicting_named_field_function_param(CallbackHolder {
        callback: local_target,
    })
}

pub fn call_multi_conflicting_named_field_function_param_with_other_target() -> i32 {
    call_multi_conflicting_named_field_function_param(CallbackHolder {
        callback: other_target,
    })
}

pub fn call_param_alias_instance_method(value: LocalAssoc) -> i32 {
    let alias = value;
    alias.instance_value()
}

pub mod deep_path_root {
    pub mod branch {
        pub mod leaf {
            pub fn deep_target() -> i32 {
                144
            }
        }
    }

    pub fn call_self_deep_path_target() -> i32 {
        self::branch::leaf::deep_target()
    }
}

pub fn call_crate_deep_path_target() -> i32 {
    crate::deep_path_root::branch::leaf::deep_target()
}

pub fn call_self_deep_path_target() -> i32 {
    self::deep_path_root::branch::leaf::deep_target()
}

pub fn call_external_default_bound_assoc<T>() -> T
where
    T: Default,
{
    T::default()
}

pub async fn call_awaited_async_closure_future_alias_chain_with_body_call() {
    let closure = async || local_target();
    let future = closure();
    let alias = future;
    let second = alias;
    second.await;
}

pub fn call_iter_result_size_hint<I>(iter: I) -> (usize, Option<usize>)
where
    I: IntoIterator<Item = i32>,
{
    let iter = iter.into_iter();
    iter.size_hint()
}

pub struct AwaitMethodResultSource;

impl AwaitMethodResultSource {
    pub fn ready_result(&self) -> std::future::Ready<Result<i32, ()>> {
        std::future::ready(Ok(91))
    }
}

pub async fn call_await_method_result_unwrap() -> i32 {
    let source = AwaitMethodResultSource;
    source.ready_result().await.unwrap()
}

fn call_forwarded_function_pointer_leaf(f: fn() -> i32) -> i32 {
    f()
}

fn call_forwarded_function_pointer_wrapper(f: fn() -> i32) -> i32 {
    call_forwarded_function_pointer_leaf(f)
}

pub fn call_forwarded_function_pointer_param_with_local_target() -> i32 {
    call_forwarded_function_pointer_wrapper(local_target)
}

fn call_forwarded_conflicting_function_pointer_leaf(f: fn() -> i32) -> i32 {
    f()
}

fn call_forwarded_conflicting_function_pointer_wrapper(f: fn() -> i32) -> i32 {
    call_forwarded_conflicting_function_pointer_leaf(f)
}

pub fn call_forwarded_conflicting_function_pointer_param_with_local_target() -> i32 {
    call_forwarded_conflicting_function_pointer_wrapper(local_target)
}

pub fn call_forwarded_conflicting_function_pointer_param_with_other_target() -> i32 {
    call_forwarded_conflicting_function_pointer_wrapper(other_target)
}

fn call_forwarded_named_field_leaf(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

fn call_forwarded_named_field_wrapper(holder: CallbackHolder) -> i32 {
    call_forwarded_named_field_leaf(holder)
}

pub fn call_forwarded_named_field_param_with_local_target() -> i32 {
    call_forwarded_named_field_wrapper(CallbackHolder {
        callback: local_target,
    })
}

fn call_forwarded_conflicting_named_field_leaf(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

fn call_forwarded_conflicting_named_field_wrapper(holder: CallbackHolder) -> i32 {
    call_forwarded_conflicting_named_field_leaf(holder)
}

pub fn call_forwarded_conflicting_named_field_param_with_local_target() -> i32 {
    call_forwarded_conflicting_named_field_wrapper(CallbackHolder {
        callback: local_target,
    })
}

pub fn call_forwarded_conflicting_named_field_param_with_other_target() -> i32 {
    call_forwarded_conflicting_named_field_wrapper(CallbackHolder {
        callback: other_target,
    })
}

fn call_two_hop_forwarded_function_pointer_leaf(f: fn() -> i32) -> i32 {
    f()
}

fn call_two_hop_forwarded_function_pointer_middle(f: fn() -> i32) -> i32 {
    call_two_hop_forwarded_function_pointer_leaf(f)
}

fn call_two_hop_forwarded_function_pointer_wrapper(f: fn() -> i32) -> i32 {
    call_two_hop_forwarded_function_pointer_middle(f)
}

pub fn call_two_hop_forwarded_function_pointer_param_with_local_target() -> i32 {
    call_two_hop_forwarded_function_pointer_wrapper(local_target)
}

fn call_two_hop_forwarded_conflicting_function_pointer_leaf(f: fn() -> i32) -> i32 {
    f()
}

fn call_two_hop_forwarded_conflicting_function_pointer_middle(f: fn() -> i32) -> i32 {
    call_two_hop_forwarded_conflicting_function_pointer_leaf(f)
}

fn call_two_hop_forwarded_conflicting_function_pointer_wrapper(f: fn() -> i32) -> i32 {
    call_two_hop_forwarded_conflicting_function_pointer_middle(f)
}

pub fn call_two_hop_forwarded_conflicting_function_pointer_param_with_local_target() -> i32 {
    call_two_hop_forwarded_conflicting_function_pointer_wrapper(local_target)
}

pub fn call_two_hop_forwarded_conflicting_function_pointer_param_with_other_target() -> i32 {
    call_two_hop_forwarded_conflicting_function_pointer_wrapper(other_target)
}

fn call_two_hop_forwarded_named_field_leaf(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

fn call_two_hop_forwarded_named_field_middle(holder: CallbackHolder) -> i32 {
    call_two_hop_forwarded_named_field_leaf(holder)
}

fn call_two_hop_forwarded_named_field_wrapper(holder: CallbackHolder) -> i32 {
    call_two_hop_forwarded_named_field_middle(holder)
}

pub fn call_two_hop_forwarded_named_field_param_with_local_target() -> i32 {
    call_two_hop_forwarded_named_field_wrapper(CallbackHolder {
        callback: local_target,
    })
}

fn call_two_hop_forwarded_conflicting_named_field_leaf(holder: CallbackHolder) -> i32 {
    (holder.callback)()
}

fn call_two_hop_forwarded_conflicting_named_field_middle(holder: CallbackHolder) -> i32 {
    call_two_hop_forwarded_conflicting_named_field_leaf(holder)
}

fn call_two_hop_forwarded_conflicting_named_field_wrapper(holder: CallbackHolder) -> i32 {
    call_two_hop_forwarded_conflicting_named_field_middle(holder)
}

pub fn call_two_hop_forwarded_conflicting_named_field_param_with_local_target() -> i32 {
    call_two_hop_forwarded_conflicting_named_field_wrapper(CallbackHolder {
        callback: local_target,
    })
}

pub fn call_two_hop_forwarded_conflicting_named_field_param_with_other_target() -> i32 {
    call_two_hop_forwarded_conflicting_named_field_wrapper(CallbackHolder {
        callback: other_target,
    })
}

pub fn call_method_result_binding_instance_method() -> i32 {
    let value: LocalAssoc = LocalAssoc;
    let cloned = value.clone_assoc();
    cloned.instance_value()
}

pub struct AwaitLocalAssocMethodResultSource;

impl AwaitLocalAssocMethodResultSource {
    pub async fn ready_assoc(&self) -> LocalAssoc {
        LocalAssoc
    }
}

pub async fn call_await_method_result_instance_method() -> i32 {
    let source = AwaitLocalAssocMethodResultSource;
    source.ready_assoc().await.instance_value()
}

fn return_forwarded_function_pointer(f: fn() -> i32) -> fn() -> i32 {
    f
}

pub fn call_returned_forwarded_function_pointer_param_with_local_target() -> i32 {
    return_forwarded_function_pointer(local_target)()
}

fn return_conflicting_forwarded_function_pointer(f: fn() -> i32) -> fn() -> i32 {
    f
}

pub fn call_returned_conflicting_forwarded_function_pointer_param_with_local_target() -> i32 {
    return_conflicting_forwarded_function_pointer(local_target)()
}

pub fn call_returned_conflicting_forwarded_function_pointer_param_with_other_target() -> i32 {
    return_conflicting_forwarded_function_pointer(other_target)()
}

pub fn call_parenthesized_referenced_dyn_fn_value_binding() -> i32 {
    let referenced_fn: &dyn Fn() -> i32 = &local_target;
    (referenced_fn)()
}

pub fn call_item_macro_generated_function() -> i32 {
    call_graph_item_macro!();
    generated_by_item_macro()
}

pub fn call_parenthesized_mut_referenced_dyn_fnmut_value_binding() -> i32 {
    let mut target = local_target;
    let mut referenced_fn: &mut dyn FnMut() -> i32 = &mut target;
    (referenced_fn)()
}

macro_rules! call_graph_const_item_macro {
    () => {
        const GENERATED_BY_CONST_ITEM_MACRO: i32 = assoc_const_value();
    };
}

pub fn call_const_item_macro_generated_const_initializer() -> i32 {
    call_graph_const_item_macro!();
    0
}

fn call_single_referenced_dyn_fn_param(f: &dyn Fn() -> i32) -> i32 {
    f()
}

pub fn call_single_referenced_dyn_fn_param_with_local_target() -> i32 {
    call_single_referenced_dyn_fn_param(&local_target)
}

fn call_single_parenthesized_referenced_dyn_fn_param(f: &dyn Fn() -> i32) -> i32 {
    (f)()
}

pub fn call_single_parenthesized_referenced_dyn_fn_param_with_local_target() -> i32 {
    call_single_parenthesized_referenced_dyn_fn_param(&local_target)
}

fn call_single_boxed_dyn_fn_param(f: Box<dyn Fn() -> i32>) -> i32 {
    f()
}

pub fn call_single_boxed_dyn_fn_param_with_local_target() -> i32 {
    call_single_boxed_dyn_fn_param(Box::new(local_target))
}

fn call_single_parenthesized_boxed_dyn_fn_param(f: Box<dyn Fn() -> i32>) -> i32 {
    (f)()
}

pub fn call_single_parenthesized_boxed_dyn_fn_param_with_local_target() -> i32 {
    call_single_parenthesized_boxed_dyn_fn_param(Box::new(local_target))
}

fn call_forwarded_referenced_dyn_fn_leaf(f: &dyn Fn() -> i32) -> i32 {
    f()
}

fn call_forwarded_referenced_dyn_fn_wrapper(f: &dyn Fn() -> i32) -> i32 {
    call_forwarded_referenced_dyn_fn_leaf(f)
}

pub fn call_forwarded_referenced_dyn_fn_with_local_target() -> i32 {
    call_forwarded_referenced_dyn_fn_wrapper(&local_target)
}

fn call_two_hop_forwarded_referenced_dyn_fn_leaf(f: &dyn Fn() -> i32) -> i32 {
    f()
}

fn call_two_hop_forwarded_referenced_dyn_fn_middle(f: &dyn Fn() -> i32) -> i32 {
    call_two_hop_forwarded_referenced_dyn_fn_leaf(f)
}

fn call_two_hop_forwarded_referenced_dyn_fn_wrapper(f: &dyn Fn() -> i32) -> i32 {
    call_two_hop_forwarded_referenced_dyn_fn_middle(f)
}

pub fn call_two_hop_forwarded_referenced_dyn_fn_with_local_target() -> i32 {
    call_two_hop_forwarded_referenced_dyn_fn_wrapper(&local_target)
}

pub async fn call_awaited_async_closure_future_tuple_field_with_body_call() {
    let closure = async || local_target();
    let futures = (closure(),);
    futures.0.await;
}

macro_rules! call_graph_static_item_macro {
    () => {
        static GENERATED_BY_STATIC_ITEM_MACRO: i32 = assoc_const_value();
    };
}

pub fn call_static_item_macro_generated_static_initializer() -> i32 {
    call_graph_static_item_macro!();
    0
}

fn call_forwarded_boxed_dyn_fn_leaf(f: Box<dyn Fn() -> i32>) -> i32 {
    f()
}

fn call_forwarded_boxed_dyn_fn_wrapper(f: Box<dyn Fn() -> i32>) -> i32 {
    call_forwarded_boxed_dyn_fn_leaf(f)
}

pub fn call_forwarded_boxed_dyn_fn_with_local_target() -> i32 {
    call_forwarded_boxed_dyn_fn_wrapper(Box::new(local_target))
}

fn call_two_hop_forwarded_boxed_dyn_fn_leaf(f: Box<dyn Fn() -> i32>) -> i32 {
    f()
}

fn call_two_hop_forwarded_boxed_dyn_fn_middle(f: Box<dyn Fn() -> i32>) -> i32 {
    call_two_hop_forwarded_boxed_dyn_fn_leaf(f)
}

fn call_two_hop_forwarded_boxed_dyn_fn_wrapper(f: Box<dyn Fn() -> i32>) -> i32 {
    call_two_hop_forwarded_boxed_dyn_fn_middle(f)
}

pub fn call_two_hop_forwarded_boxed_dyn_fn_with_local_target() -> i32 {
    call_two_hop_forwarded_boxed_dyn_fn_wrapper(Box::new(local_target))
}

fn call_forwarded_conflicting_boxed_dyn_fn_leaf(f: Box<dyn Fn() -> i32>) -> i32 {
    f()
}

fn call_forwarded_conflicting_boxed_dyn_fn_wrapper(f: Box<dyn Fn() -> i32>) -> i32 {
    call_forwarded_conflicting_boxed_dyn_fn_leaf(f)
}

pub fn call_forwarded_conflicting_boxed_dyn_fn_with_local_target() -> i32 {
    call_forwarded_conflicting_boxed_dyn_fn_wrapper(Box::new(local_target))
}

pub fn call_forwarded_conflicting_boxed_dyn_fn_with_other_target() -> i32 {
    call_forwarded_conflicting_boxed_dyn_fn_wrapper(Box::new(other_target))
}

macro_rules! call_graph_expr_path_macro {
    () => {
        local_target();
    };
}

pub fn call_expr_macro_generated_path_call() {
    call_graph_expr_path_macro!();
}

struct AsyncFutureHolder<F> {
    future: F,
}

pub async fn call_awaited_async_closure_future_named_field_with_body_call() {
    let closure = async || local_target();
    let holder = AsyncFutureHolder { future: closure() };
    holder.future.await;
}

fn local_result_target(value: i32) -> Result<i32, ()> {
    Ok(value)
}

fn call_single_result_callback(f: fn(i32) -> Result<i32, ()>) -> Result<i32, ()> {
    Ok::<i32, ()>(1).and_then(f)
}

pub fn call_single_result_callback_with_local_target() -> Result<i32, ()> {
    call_single_result_callback(local_result_target)
}
