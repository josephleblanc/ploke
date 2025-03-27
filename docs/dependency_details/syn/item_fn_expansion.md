## `syn` documentation snippets

 ```rust
pub struct ItemFn {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub block: Box<Block>,
}
```

```rust
// syn::Visibility
// The visibility level of an item: inherited or pub or pub(restricted)
pub enum Visibility {
    Public(Pub),
    Restricted(VisRestricted),
    Inherited,
}
```

```rust
    /// A visibility level restricted to some path: `pub(self)` or
    /// `pub(super)` or `pub(crate)` or `pub(in some::module)`.
    #[cfg_attr(docsrs, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct VisRestricted {
        pub pub_token: Token![pub],
        pub paren_token: token::Paren,
        pub in_token: Option<Token![in]>,
        pub path: Box<Path>,
    }
```

```rust
    /// A path at which a named item is exported (e.g. `std::collections::HashMap`).
    #[cfg_attr(docsrs, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct Path {
        pub leading_colon: Option<Token![::]>,
        pub segments: Punctuated<PathSegment, Token![::]>,
    }

    /// A segment of a path together with any path arguments on that segment.
    #[cfg_attr(docsrs, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub struct PathSegment {
        pub ident: Ident,
        pub arguments: PathArguments,
    }
```

A note on the `Ident` type:
```
A word of Rust code, which may be a keyword or legal variable name.

An identifier consists of at least one Unicode code point, the first of which has the XID_Start property and the rest of which have the XID_Continue property.

The empty string is not an identifier. Use Option<Ident>.
A lifetime is not an identifier. Use syn::Lifetime instead.
An identifier constructed with Ident::new is permitted to be a Rust keyword, though parsing one through its Parse implementation rejects Rust keywords. Use input.call(Ident::parse_any) when parsing to match the behaviour of Ident::new.
```

```rust
    /// Angle bracketed or parenthesized arguments of a path segment.
    ///
    /// ## Angle bracketed
    ///
    /// The `<'a, T>` in `std::slice::iter<'a, T>`.
    ///
    /// ## Parenthesized
    ///
    /// The `(A, B) -> C` in `Fn(A, B) -> C`.
    #[cfg_attr(docsrs, doc(cfg(any(feature = "full", feature = "derive"))))]
    pub enum PathArguments {
        None,
        /// The `<'a, T>` in `std::slice::iter<'a, T>`.
        AngleBracketed(AngleBracketedGenericArguments),
        /// The `(A, B) -> C` in `Fn(A, B) -> C`.
        Parenthesized(ParenthesizedGenericArguments),
    }
```
