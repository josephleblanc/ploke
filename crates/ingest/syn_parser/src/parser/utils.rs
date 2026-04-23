use quote::ToTokens;
use syn::{
    ConstParam, Field, ImplItemFn, ItemConst, ItemEnum, ItemExternCrate, ItemFn, ItemImpl,
    ItemMacro, ItemMod, ItemStatic, ItemStruct, ItemTrait, ItemType, ItemUnion, ItemUse, Lifetime,
    LifetimeParam, TraitItemFn, TypeParam, UseGlob, UseName, UseRename, UseTree, Variant,
    spanned::Spanned,
};

// ============================================================================
// Syn1 to Syn2 Type Conversion
// ============================================================================
// These functions convert syn1 types to syn2 types, allowing us to use the
// same processing logic for both syn versions. This is primarily needed for
// handling Rust 2015 syntax that syn2 rejects but syn1 accepts.

/// Converts a syn1::Type to a syn::Type.
pub fn convert_type_syn1_to_syn2(ty: &syn1::Type) -> syn::Type {
    match ty {
        syn1::Type::Array(arr) => {
            // Convert element type, stringify the length expression
            let elem = convert_type_syn1_to_syn2(&arr.elem);
            let len_tokens = arr.len.to_token_stream();
            // Parse the stringified expression as syn2 Expr
            let len: syn::Expr = syn::parse2(len_tokens).unwrap_or_else(|_| {
                // Fallback to a placeholder if parsing fails
                syn::parse_quote!(0)
            });
            syn::Type::Array(syn::TypeArray {
                bracket_token: syn::token::Bracket::default(),
                elem: Box::new(elem),
                semi_token: syn::token::Semi::default(),
                len,
            })
        }

        syn1::Type::BareFn(bare) => syn::Type::BareFn(syn::TypeBareFn {
            lifetimes: bare
                .lifetimes
                .as_ref()
                .map(convert_bound_lifetimes_syn1_to_syn2),
            unsafety: bare.unsafety.map(|_| syn::token::Unsafe::default()),
            abi: bare.abi.as_ref().map(convert_abi_syn1_to_syn2),
            fn_token: syn::token::Fn::default(),
            paren_token: syn::token::Paren::default(),
            inputs: bare
                .inputs
                .iter()
                .map(|b| syn::BareFnArg {
                    attrs: vec![], // Skip attribute conversion for now
                    name: b
                        .name
                        .as_ref()
                        .map(|(ident, _colon)| (ident.clone(), syn::token::Colon::default())),
                    ty: convert_type_syn1_to_syn2(&b.ty),
                })
                .collect(),
            variadic: bare.variadic.as_ref().map(|_| syn::BareVariadic {
                attrs: vec![], // Skip attribute conversion
                name: None,    // syn1 Variadic doesn't have name, syn2 BareVariadic does
                dots: syn::token::DotDotDot::default(),
                comma: None, // Could try to detect from input but using default
            }),
            output: convert_return_type_syn1_to_syn2(&bare.output),
        }),

        syn1::Type::Group(grp) => syn::Type::Group(syn::TypeGroup {
            group_token: syn::token::Group::default(),
            elem: Box::new(convert_type_syn1_to_syn2(&grp.elem)),
        }),

        syn1::Type::ImplTrait(impl_trait) => syn::Type::ImplTrait(syn::TypeImplTrait {
            impl_token: syn::token::Impl::default(),
            bounds: impl_trait
                .bounds
                .iter()
                .map(convert_type_param_bound_syn1_to_syn2)
                .collect(),
        }),

        syn1::Type::Macro(mac) => syn::Type::Macro(syn::TypeMacro {
            mac: convert_macro_syn1_to_syn2(&mac.mac),
        }),

        syn1::Type::Never(_never) => syn::Type::Never(syn::TypeNever {
            bang_token: syn::token::Not::default(),
        }),

        syn1::Type::Paren(paren) => syn::Type::Paren(syn::TypeParen {
            paren_token: syn::token::Paren::default(),
            elem: Box::new(convert_type_syn1_to_syn2(&paren.elem)),
        }),

        syn1::Type::Path(path) => syn::Type::Path(syn::TypePath {
            qself: path.qself.as_ref().map(|q| syn::QSelf {
                lt_token: syn::token::Lt::default(),
                ty: Box::new(convert_type_syn1_to_syn2(&q.ty)),
                position: q.position,
                as_token: q.as_token.map(|_| syn::token::As::default()),
                gt_token: syn::token::Gt::default(),
            }),
            path: convert_path_syn1_to_syn2(&path.path),
        }),

        syn1::Type::Reference(reference) => syn::Type::Reference(syn::TypeReference {
            and_token: syn::token::And::default(),
            lifetime: reference.lifetime.as_ref().map(|lt| {
                syn::Lifetime::new(&format!("'{}", lt.ident), proc_macro2::Span::call_site())
            }),
            mutability: reference.mutability.map(|_| syn::token::Mut::default()),
            elem: Box::new(convert_type_syn1_to_syn2(&reference.elem)),
        }),

        syn1::Type::Slice(slice) => syn::Type::Slice(syn::TypeSlice {
            bracket_token: syn::token::Bracket::default(),
            elem: Box::new(convert_type_syn1_to_syn2(&slice.elem)),
        }),

        syn1::Type::TraitObject(trait_obj) => syn::Type::TraitObject(syn::TypeTraitObject {
            dyn_token: trait_obj.dyn_token.map(|_| syn::token::Dyn::default()),
            bounds: trait_obj
                .bounds
                .iter()
                .map(convert_type_param_bound_syn1_to_syn2)
                .collect(),
        }),

        syn1::Type::Tuple(tuple) => syn::Type::Tuple(syn::TypeTuple {
            paren_token: syn::token::Paren::default(),
            elems: tuple.elems.iter().map(convert_type_syn1_to_syn2).collect(),
        }),

        syn1::Type::Verbatim(tokens) => syn::Type::Verbatim(tokens.clone()),

        syn1::Type::Infer(_infer) => syn::Type::Infer(syn::TypeInfer {
            underscore_token: syn::token::Underscore::default(),
        }),

        syn1::Type::Ptr(ptr) => syn::Type::Ptr(syn::TypePtr {
            star_token: syn::token::Star::default(),
            const_token: ptr.const_token.map(|_| syn::token::Const::default()),
            mutability: ptr.mutability.map(|_| syn::token::Mut::default()),
            elem: Box::new(convert_type_syn1_to_syn2(&ptr.elem)),
        }),

        // Handle unexpected variants gracefully
        _ => syn::Type::Verbatim(quote::quote!({})),
    }
}

/// Converts a syn1::Path to a syn::Path.
fn convert_path_syn1_to_syn2(path: &syn1::Path) -> syn::Path {
    syn::Path {
        leading_colon: path.leading_colon.map(|_| syn::token::PathSep::default()),
        segments: path
            .segments
            .iter()
            .map(|seg| syn::PathSegment {
                ident: seg.ident.clone(),
                arguments: convert_path_arguments_syn1_to_syn2(&seg.arguments),
            })
            .collect(),
    }
}

/// Converts syn1::PathArguments to syn::PathArguments.
fn convert_path_arguments_syn1_to_syn2(args: &syn1::PathArguments) -> syn::PathArguments {
    match args {
        syn1::PathArguments::None => syn::PathArguments::None,
        syn1::PathArguments::AngleBracketed(angled) => {
            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                colon2_token: angled.colon2_token.map(|_| syn::token::PathSep::default()),
                lt_token: syn::token::Lt::default(),
                args: angled
                    .args
                    .iter()
                    .map(convert_generic_argument_syn1_to_syn2)
                    .collect(),
                gt_token: syn::token::Gt::default(),
            })
        }
        syn1::PathArguments::Parenthesized(paren) => {
            syn::PathArguments::Parenthesized(syn::ParenthesizedGenericArguments {
                paren_token: syn::token::Paren::default(),
                inputs: paren.inputs.iter().map(convert_type_syn1_to_syn2).collect(),
                output: convert_return_type_syn1_to_syn2(&paren.output),
            })
        }
    }
}

/// Converts syn1::GenericArgument to syn::GenericArgument.
fn convert_generic_argument_syn1_to_syn2(arg: &syn1::GenericArgument) -> syn::GenericArgument {
    match arg {
        syn1::GenericArgument::Lifetime(lt) => syn::GenericArgument::Lifetime(syn::Lifetime::new(
            &format!("'{}", lt.ident),
            proc_macro2::Span::call_site(),
        )),
        syn1::GenericArgument::Type(ty) => {
            syn::GenericArgument::Type(convert_type_syn1_to_syn2(ty))
        }
        syn1::GenericArgument::Const(expr) => {
            // Stringify the const expression and parse as syn2 Expr
            let expr_tokens = expr.to_token_stream();
            let expr: syn::Expr = syn::parse2(expr_tokens).unwrap_or_else(|_| {
                // Fallback to a placeholder underscore if parsing fails
                syn::parse_quote!(_)
            });
            syn::GenericArgument::Const(expr)
        }
        syn1::GenericArgument::Binding(binding) => {
            // syn1 Binding -> syn2 AssocType
            syn::GenericArgument::AssocType(syn::AssocType {
                ident: binding.ident.clone(),
                generics: None, // syn1 Binding doesn't have generic args
                eq_token: syn::token::Eq::default(),
                ty: convert_type_syn1_to_syn2(&binding.ty),
            })
        }
        syn1::GenericArgument::Constraint(constraint) => {
            syn::GenericArgument::Constraint(syn::Constraint {
                ident: constraint.ident.clone(),
                generics: None, // syn1 Constraint doesn't have generic args
                colon_token: syn::token::Colon::default(),
                bounds: constraint
                    .bounds
                    .iter()
                    .map(convert_type_param_bound_syn1_to_syn2)
                    .collect(),
            })
        }
    }
}

/// Converts syn1::TypeParamBound to syn::TypeParamBound.
fn convert_type_param_bound_syn1_to_syn2(bound: &syn1::TypeParamBound) -> syn::TypeParamBound {
    match bound {
        syn1::TypeParamBound::Lifetime(lt) => syn::TypeParamBound::Lifetime(syn::Lifetime::new(
            &format!("'{}", lt.ident),
            proc_macro2::Span::call_site(),
        )),
        syn1::TypeParamBound::Trait(trait_bound) => syn::TypeParamBound::Trait(syn::TraitBound {
            paren_token: trait_bound
                .paren_token
                .map(|_| syn::token::Paren::default()),
            modifier: convert_trait_bound_modifier_syn1_to_syn2(&trait_bound.modifier),
            lifetimes: trait_bound.lifetimes.as_ref().map(|bl| {
                syn::BoundLifetimes {
                    for_token: syn::token::For::default(),
                    lt_token: syn::token::Lt::default(),
                    lifetimes: bl
                        .lifetimes
                        .iter()
                        .map(|ld| {
                            syn::GenericParam::Lifetime(syn::LifetimeParam {
                                attrs: vec![], // Skip attrs
                                lifetime: syn::Lifetime::new(
                                    &format!("'{}", ld.lifetime.ident),
                                    proc_macro2::Span::call_site(),
                                ),
                                colon_token: ld.colon_token.map(|_| syn::token::Colon::default()),
                                bounds: ld
                                    .bounds
                                    .iter()
                                    .map(|b| {
                                        syn::Lifetime::new(
                                            &format!("'{}", b.ident),
                                            proc_macro2::Span::call_site(),
                                        )
                                    })
                                    .collect(),
                            })
                        })
                        .collect(),
                    gt_token: syn::token::Gt::default(),
                }
            }),
            path: convert_path_syn1_to_syn2(&trait_bound.path),
        }),
    }
}

/// Converts syn1::TraitBoundModifier to syn::TraitBoundModifier.
fn convert_trait_bound_modifier_syn1_to_syn2(
    modifier: &syn1::TraitBoundModifier,
) -> syn::TraitBoundModifier {
    match modifier {
        syn1::TraitBoundModifier::None => syn::TraitBoundModifier::None,
        syn1::TraitBoundModifier::Maybe(_) => {
            syn::TraitBoundModifier::Maybe(syn::token::Question::default())
        }
    }
}

/// Converts syn1::ReturnType to syn::ReturnType.
fn convert_return_type_syn1_to_syn2(ret: &syn1::ReturnType) -> syn::ReturnType {
    match ret {
        syn1::ReturnType::Default => syn::ReturnType::Default,
        syn1::ReturnType::Type(_, ty) => {
            // Create a new syn2 RArrow token instead of copying from syn1
            syn::ReturnType::Type(
                syn::token::RArrow::default(),
                Box::new(convert_type_syn1_to_syn2(ty)),
            )
        }
    }
}

/// Converts syn1::BoundLifetimes to syn::BoundLifetimes.
fn convert_bound_lifetimes_syn1_to_syn2(bl: &syn1::BoundLifetimes) -> syn::BoundLifetimes {
    syn::BoundLifetimes {
        for_token: syn::token::For::default(),
        lt_token: syn::token::Lt::default(),
        lifetimes: bl
            .lifetimes
            .iter()
            .map(|ld| {
                syn::GenericParam::Lifetime(syn::LifetimeParam {
                    attrs: vec![], // Skip attribute conversion for lifetime params
                    lifetime: syn::Lifetime::new(
                        &format!("'{}", ld.lifetime.ident),
                        proc_macro2::Span::call_site(),
                    ),
                    colon_token: ld.colon_token.map(|_| syn::token::Colon::default()),
                    bounds: ld
                        .bounds
                        .iter()
                        .map(|b| {
                            syn::Lifetime::new(
                                &format!("'{}", b.ident),
                                proc_macro2::Span::call_site(),
                            )
                        })
                        .collect(),
                })
            })
            .collect(),
        gt_token: syn::token::Gt::default(),
    }
}

/// Converts syn1::Abi to syn::Abi.
fn convert_abi_syn1_to_syn2(abi: &syn1::Abi) -> syn::Abi {
    syn::Abi {
        extern_token: syn::token::Extern::default(),
        name: abi
            .name
            .as_ref()
            .map(|lit| syn::LitStr::new(&lit.value(), proc_macro2::Span::call_site())),
    }
}

/// Converts syn1::Macro to syn::Macro.
fn convert_macro_syn1_to_syn2(mac: &syn1::Macro) -> syn::Macro {
    syn::Macro {
        path: convert_path_syn1_to_syn2(&mac.path),
        bang_token: syn::token::Not::default(),
        delimiter: convert_macro_delimiter_syn1_to_syn2(&mac.delimiter),
        tokens: mac.tokens.clone(),
    }
}

/// Converts syn1::MacroDelimiter to syn::MacroDelimiter.
fn convert_macro_delimiter_syn1_to_syn2(del: &syn1::MacroDelimiter) -> syn::MacroDelimiter {
    match del {
        syn1::MacroDelimiter::Paren(_) => syn::MacroDelimiter::Paren(syn::token::Paren::default()),
        syn1::MacroDelimiter::Brace(_) => syn::MacroDelimiter::Brace(syn::token::Brace::default()),
        syn1::MacroDelimiter::Bracket(_) => {
            syn::MacroDelimiter::Bracket(syn::token::Bracket::default())
        }
    }
}

/// Converts syn1::Attribute to syn::Attribute using TokenStream roundtrip.
///
/// This attempts to parse syn1's attribute path and tokens into syn2's structured Meta format.
/// If parsing fails, returns a specific error for debugging.
pub fn convert_attribute_syn1_to_syn2(
    attr: &syn1::Attribute,
) -> Result<syn::Attribute, crate::error::CodeVisitorError> {
    use quote::ToTokens;

    // Combine path and tokens for roundtrip parsing
    let path_str = attr.path.to_token_stream().to_string();
    let tokens_str = attr.tokens.to_string();

    // Create a TokenStream combining path and tokens
    let combined: proc_macro2::TokenStream = attr
        .path
        .to_token_stream()
        .into_iter()
        .chain(attr.tokens.clone())
        .collect();

    // Attempt to parse as syn2 Meta
    match syn::parse2::<syn::Meta>(combined) {
        Ok(meta) => Ok(syn::Attribute {
            pound_token: syn::token::Pound::default(),
            style: convert_attr_style_syn1_to_syn2(&attr.style),
            bracket_token: syn::token::Bracket::default(),
            meta,
        }),
        Err(e) => Err(
            crate::error::CodeVisitorError::Syn1ToSyn2AttributeConversion {
                path: path_str,
                tokens: tokens_str,
                parse_error: e.to_string(),
            },
        ),
    }
}

/// Converts syn1::AttrStyle to syn::AttrStyle.
fn convert_attr_style_syn1_to_syn2(style: &syn1::AttrStyle) -> syn::AttrStyle {
    match style {
        syn1::AttrStyle::Outer => syn::AttrStyle::Outer,
        syn1::AttrStyle::Inner(_) => syn::AttrStyle::Inner(syn::token::Not::default()),
    }
}

// Note: From the proc_macro2 crate documentation on Span::byte_range()
//When executing in a procedural macro context, the returned range is only accurate if compiled
//with a nightly toolchain. The stable toolchain does not have this information available. When
//executing outside of a procedural macro, such as main.rs or build.rs, the byte range is always
//accurate regardless of toolchain.
/// Helper trait to extract the byte span from a syn item node in the AST. The returned
/// (usize, usize) is the start/end byte position of the item definition in the target source code.
pub trait ExtractSpan
where
    Self: Spanned,
{
    fn extract_span_bytes(&self) -> (usize, usize) {
        let byte_range = self.span().byte_range();
        (byte_range.start, byte_range.end)
    }
}
impl ExtractSpan for ItemStruct {}
impl ExtractSpan for ItemFn {}
impl ExtractSpan for ItemEnum {}
impl ExtractSpan for ItemImpl {}
impl ExtractSpan for ImplItemFn {}
impl ExtractSpan for ItemTrait {}
impl ExtractSpan for TraitItemFn {}
impl ExtractSpan for ItemType {}
impl ExtractSpan for ItemUnion {}
impl ExtractSpan for ItemUse {}
impl ExtractSpan for ItemMod {}
impl ExtractSpan for UseTree {}
impl ExtractSpan for UseName {}
impl ExtractSpan for UseRename {}
impl ExtractSpan for Lifetime {}
impl ExtractSpan for UseGlob {}
impl ExtractSpan for Field {}
impl ExtractSpan for Variant {}
impl ExtractSpan for ItemExternCrate {}
impl ExtractSpan for ItemConst {}
impl ExtractSpan for ItemStatic {}
impl ExtractSpan for ItemMacro {}
impl ExtractSpan for TypeParam {}
impl ExtractSpan for LifetimeParam {}
impl ExtractSpan for ConstParam {}

// impl ExtractSpan for ItemMod {}

// --- Helper potentially in syn_parser ---
// It's often useful to have a helper to get the string representation.
// Place this where appropriate (e.g., type_processing.rs or utils.rs).
// Removed unused import: use quote::ToTokens;

// Removed unused function: type_to_string

// --- Example Usage in syn_parser ---
/*
// Inside get_or_create_type or similar logic:

let type_str = type_to_string(ty); // Use the helper

// Check cache... if not found:
let new_type_id = TypeId::generate_synthetic(
    state.crate_namespace,      // Namespace of the crate being parsed
    &state.current_file_path,   // File where 'ty' was encountered
    &type_str,                  // The string representation of 'ty'
);

// Store new_type_id in cache (mapping type_str -> new_type_id)
// Create TypeNode using new_type_id
// Return new_type_id
*/

// This implementation uses the string representation derived from `to_token_stream()` as the core
// input representing the type, combined with the necessary file and crate context, to generate a
// deterministic `TypeId::Synthetic`.
