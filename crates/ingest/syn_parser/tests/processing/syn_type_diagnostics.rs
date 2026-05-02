//! Small diagnostic tests for how `syn` parses Rust type syntax.
//!
//! These are intentionally narrow and parser-facing. They help answer
//! questions like:
//! - which forms are all just `syn::Type::Path` at parse time?
//! - which forms become `TraitObject`, `ImplTrait`, or `BareFn`?
//! - where do trait names show up as plain paths versus trait bounds?
//!
//! This is useful for the type-resolution design work because `syn` is our
//! frontend: we need to know what distinctions are available structurally
//! before we decide what semantic layers to add on top.

use syn::{
    AngleBracketedGenericArguments, GenericArgument, GenericParam, PathArguments, ReturnType, Type,
    TypeParamBound, TypeTraitObject, parse_str,
};

fn parse_type(src: &str) -> Type {
    parse_str::<Type>(src).unwrap_or_else(|err| panic!("failed to parse type `{src}`: {err}"))
}

fn first_segment_ident(ty: &Type) -> String {
    let Type::Path(type_path) = ty else {
        panic!("expected Type::Path");
    };

    type_path
        .path
        .segments
        .first()
        .expect("path should have at least one segment")
        .ident
        .to_string()
}

#[test]
fn plain_named_builtin_self_and_generic_forms_are_type_path() {
    for src in ["OtherStruct", "T", "Self", "u8"] {
        let ty = parse_type(src);
        assert!(
            matches!(ty, Type::Path(_)),
            "expected `{src}` to parse as Type::Path"
        );
    }

    assert_eq!(
        first_segment_ident(&parse_type("OtherStruct")),
        "OtherStruct"
    );
    assert_eq!(first_segment_ident(&parse_type("T")), "T");
    assert_eq!(first_segment_ident(&parse_type("Self")), "Self");
    assert_eq!(first_segment_ident(&parse_type("u8")), "u8");
}

#[test]
fn plain_fn_trait_form_is_not_accepted_as_a_type() {
    let result = parse_str::<Type>("Fn() -> ()");
    assert!(
        result.is_err(),
        "expected plain `Fn() -> ()` to be rejected as a standalone syn::Type"
    );
}

#[test]
fn dyn_fn_parses_as_trait_object_with_trait_bound() {
    let ty = parse_type("dyn Fn() -> ()");
    let Type::TraitObject(TypeTraitObject { bounds, .. }) = ty else {
        panic!("expected Type::TraitObject");
    };

    assert_eq!(bounds.len(), 1, "expected one trait bound");
    let first = bounds.first().expect("expected first bound");

    let TypeParamBound::Trait(trait_bound) = first else {
        panic!("expected trait bound");
    };

    let segment = trait_bound
        .path
        .segments
        .last()
        .expect("trait path should have a segment");
    assert_eq!(segment.ident.to_string(), "Fn");
    assert!(
        matches!(segment.arguments, PathArguments::Parenthesized(_)),
        "expected Fn trait bound to carry parenthesized args"
    );
}

#[test]
fn fn_trait_bound_position_carries_parenthesized_path_arguments() {
    let param = parse_str::<GenericParam>("T: Fn() -> ()")
        .unwrap_or_else(|err| panic!("failed to parse generic param bound: {err}"));

    let GenericParam::Type(type_param) = param else {
        panic!("expected type generic param");
    };

    let first = type_param.bounds.first().expect("expected first bound");
    let TypeParamBound::Trait(trait_bound) = first else {
        panic!("expected trait bound");
    };

    let segment = trait_bound
        .path
        .segments
        .last()
        .expect("Fn path should have a segment");
    assert_eq!(segment.ident.to_string(), "Fn");

    let PathArguments::Parenthesized(args) = &segment.arguments else {
        panic!("expected parenthesized arguments for Fn trait bound");
    };

    assert!(args.inputs.is_empty(), "expected no inputs");
    match &args.output {
        ReturnType::Type(_, output) => {
            assert!(
                matches!(&**output, Type::Tuple(tuple) if tuple.elems.is_empty()),
                "expected unit return type `()`"
            );
        }
        ReturnType::Default => panic!("expected explicit unit return type"),
    }
}

#[test]
fn impl_display_parses_as_impl_trait_with_trait_bound() {
    let ty = parse_type("impl Display");
    let Type::ImplTrait(type_impl_trait) = ty else {
        panic!("expected Type::ImplTrait");
    };

    assert_eq!(type_impl_trait.bounds.len(), 1, "expected one bound");

    let first = type_impl_trait
        .bounds
        .first()
        .expect("expected first impl-trait bound");
    let TypeParamBound::Trait(trait_bound) = first else {
        panic!("expected trait bound");
    };
    assert_eq!(
        trait_bound
            .path
            .segments
            .last()
            .expect("Display path should have a segment")
            .ident
            .to_string(),
        "Display"
    );
}

#[test]
fn bare_fn_type_is_not_a_path() {
    let ty = parse_type("fn() -> ()");
    assert!(
        matches!(ty, Type::BareFn(_)),
        "expected bare fn type to parse as Type::BareFn"
    );
}

#[test]
fn trait_bound_position_is_not_plain_type_path() {
    let param = parse_str::<GenericParam>("T: Clone")
        .unwrap_or_else(|err| panic!("failed to parse generic param bound: {err}"));

    let GenericParam::Type(type_param) = param else {
        panic!("expected type generic param");
    };

    assert_eq!(type_param.bounds.len(), 1, "expected one bound");
    let first = type_param.bounds.first().expect("expected first bound");
    let TypeParamBound::Trait(trait_bound) = first else {
        panic!("expected trait bound");
    };

    assert_eq!(
        trait_bound
            .path
            .segments
            .last()
            .expect("Clone path should have a segment")
            .ident
            .to_string(),
        "Clone"
    );
}

#[test]
fn nested_generic_named_type_stays_a_path_with_nested_type_arguments() {
    let ty = parse_type("Option<Result<T, Vec<Self>>>");
    let Type::Path(type_path) = ty else {
        panic!("expected nested generic type to parse as Type::Path");
    };

    let outer = type_path
        .path
        .segments
        .last()
        .expect("expected outer path segment");
    assert_eq!(outer.ident.to_string(), "Option");

    let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
        &outer.arguments
    else {
        panic!("expected angle-bracketed arguments on Option");
    };
    assert_eq!(args.len(), 1, "expected one Option type argument");

    let Some(GenericArgument::Type(Type::Path(result_path))) = args.first() else {
        panic!("expected Option argument to be a nested Type::Path");
    };
    let result_segment = result_path
        .path
        .segments
        .last()
        .expect("expected Result segment");
    assert_eq!(result_segment.ident.to_string(), "Result");

    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: result_args, ..
    }) = &result_segment.arguments
    else {
        panic!("expected angle-bracketed arguments on Result");
    };
    assert_eq!(result_args.len(), 2, "expected two Result type arguments");
}

#[test]
fn associated_type_path_uses_qself() {
    let ty = parse_type("<T as Iterator>::Item");
    let Type::Path(type_path) = ty else {
        panic!("expected associated type path to parse as Type::Path");
    };

    assert!(
        type_path.qself.is_some(),
        "expected qself on associated type path"
    );
    assert_eq!(
        type_path
            .path
            .segments
            .last()
            .expect("expected Item segment")
            .ident
            .to_string(),
        "Item"
    );
}

#[test]
fn reference_to_dyn_fn_is_reference_wrapping_trait_object() {
    let ty = parse_type("&dyn Fn(u8) -> Result<(), E>");
    let Type::Reference(reference) = ty else {
        panic!("expected outer type to parse as Type::Reference");
    };

    let Type::TraitObject(TypeTraitObject { bounds, .. }) = &*reference.elem else {
        panic!("expected reference target to be Type::TraitObject");
    };

    let Some(TypeParamBound::Trait(trait_bound)) = bounds.first() else {
        panic!("expected first trait-object bound to be a trait");
    };

    let fn_segment = trait_bound
        .path
        .segments
        .last()
        .expect("expected Fn segment");
    assert_eq!(fn_segment.ident.to_string(), "Fn");

    let PathArguments::Parenthesized(args) = &fn_segment.arguments else {
        panic!("expected Fn bound to have parenthesized path arguments");
    };
    assert_eq!(args.inputs.len(), 1, "expected one Fn input");
    assert!(
        matches!(args.inputs.first(), Some(Type::Path(_))),
        "expected Fn input to be a plain path type"
    );
    match &args.output {
        ReturnType::Type(_, output) => {
            assert!(
                matches!(&**output, Type::Path(_)),
                "expected Fn output to be a nested path type"
            );
        }
        ReturnType::Default => panic!("expected explicit Fn return type"),
    }
}

#[test]
fn bare_fn_can_nest_reference_pointer_and_array_types() {
    let ty = parse_type("fn(&str, *const T) -> [u8; 4]");
    let Type::BareFn(bare_fn) = ty else {
        panic!("expected composite function type to parse as Type::BareFn");
    };

    assert_eq!(bare_fn.inputs.len(), 2, "expected two bare fn inputs");
    assert!(
        matches!(&bare_fn.inputs[0].ty, Type::Reference(_)),
        "expected first bare fn input to be a reference type"
    );
    assert!(
        matches!(&bare_fn.inputs[1].ty, Type::Ptr(_)),
        "expected second bare fn input to be a raw pointer type"
    );

    match &bare_fn.output {
        ReturnType::Type(_, output) => {
            assert!(
                matches!(&**output, Type::Array(_)),
                "expected bare fn output to be an array type"
            );
        }
        ReturnType::Default => panic!("expected explicit bare fn return type"),
    }
}
