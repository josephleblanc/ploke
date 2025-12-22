use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, DeriveInput, Fields, LitStr};

#[proc_macro_derive(CozoRow, attributes(cozo))]
pub fn derive_cozo_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match impl_cozo_row(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct FieldSpec {
    ident: syn::Ident,
    ty: syn::Type,
    column: String,
}

fn impl_cozo_row(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_ident = &input.ident;

    let data = match &input.data {
        syn::Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new(
                input.span(),
                "CozoRow can only be derived for structs",
            ))
        }
    };

    let fields = match &data.fields {
        Fields::Named(named) => &named.named,
        _ => {
            return Err(syn::Error::new(
                input.span(),
                "CozoRow requires named fields",
            ))
        }
    };

    let mut specs = Vec::with_capacity(fields.len());

    for field in fields {
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new(field.span(), "Expected named field"))?
            .clone();
        let ty = field.ty.clone();

        let mut column: Option<String> = None;

        for attr in &field.attrs {
            if !attr.path().is_ident("cozo") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("col") {
                    let lit: LitStr = meta.value()?.parse()?;
                    column = Some(lit.value());
                }
                Ok(())
            })?;
        }

        let col_name = column.unwrap_or_else(|| ident.to_string());
        specs.push(FieldSpec {
            ident,
            ty,
            column: col_name,
        });
    }

    let map_inits = specs.iter().map(|spec| {
        let FieldSpec { ident, ty, column } = spec;
        quote! {
            #ident: row.get::<#ty>(#column)?
        }
    });

    let struct_name = struct_ident.to_string();

    Ok(quote! {
        impl<'a> ::core::convert::TryFrom<::ploke_db::result::Row<'a>> for #struct_ident {
            type Error = ::ploke_db::DbError;

            fn try_from(row: ::ploke_db::result::Row<'a>) -> Result<Self, Self::Error> {
                Ok(Self {
                    #(#map_inits,)*
                })
            }
        }

        impl ::core::convert::TryFrom<::ploke_db::QueryResult> for #struct_ident {
            type Error = ::ploke_db::DbError;

            fn try_from(qr: ::ploke_db::QueryResult) -> Result<Self, Self::Error> {
                if qr.rows.len() != 1 {
                    return Err(::ploke_db::DbError::Cozo(format!(
                        "Expected exactly one row for {}: found {}",
                        #struct_name,
                        qr.rows.len()
                    )));
                }
                let row = qr.row_refs().next().ok_or_else(|| ::ploke_db::DbError::Cozo(format!(
                    "Expected at least one row for {}",
                    #struct_name
                )))?;
                <#struct_ident as ::core::convert::TryFrom<::ploke_db::result::Row<'_>>>::try_from(row)
            }
        }

        impl ::core::convert::TryFrom<::ploke_db::QueryResult> for ::std::vec::Vec<#struct_ident> {
            type Error = ::ploke_db::DbError;

            fn try_from(qr: ::ploke_db::QueryResult) -> Result<Self, Self::Error> {
                qr.row_refs()
                    .map(|row| <#struct_ident as ::core::convert::TryFrom<::ploke_db::result::Row<'_>>>::try_from(row))
                    .collect()
            }
        }
    })
}
