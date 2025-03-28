
# Snippets from `syn` source code

## A note on accessing span:

```rust
/// A trait that can provide the `Span` of the complete contents of a syntax
/// tree node.
///
/// This trait is automatically implemented for all types that implement
/// [`ToTokens`] from the `quote` crate, as well as for `Span` itself.
///
/// [`ToTokens`]: quote::ToTokens
///
/// See the [module documentation] for an example.
///
/// [module documentation]: self
pub trait Spanned: private::Sealed {
    /// Returns a `Span` covering the complete contents of this syntax tree
    /// node, or [`Span::call_site()`] if this node is empty.
    ///
    /// [`Span::call_site()`]: proc_macro2::Span::call_site
    fn span(&self) -> Span;
}

impl<T: ?Sized + ToTokens> Spanned for T {
    fn span(&self) -> Span {
        self.__span()
    }
}

mod private {
    use crate::spanned::ToTokens;

    pub trait Sealed {}
    impl<T: ?Sized + ToTokens> Sealed for T {}

    #[cfg(any(feature = "full", feature = "derive"))]
    impl Sealed for crate::QSelf {}
}
```

See [condensed proc_macro2 documentation](docs/dependency_details/syn/proc_macro2_span.txt)

See full page on [proc_macro::Span](https://docs.rs/proc-macro2/1.0.94/proc_macro2/struct.Span.html)

## Item::Use(ItemUse) deep dive

```rust
    /// Things that can appear directly inside of a module or scope.
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: crate::expr::Expr#syntax-tree-enums
    #[cfg_attr(docsrs, doc(cfg(feature = "full")))]
    #[non_exhaustive]
    pub enum Item {
        /// A constant item: `const MAX: u16 = 65535`.
        Const(ItemConst),

        /// An enum definition: `enum Foo<A, B> { A(A), B(B) }`.
        Enum(ItemEnum),

        /// An `extern crate` item: `extern crate serde`.
        ExternCrate(ItemExternCrate),

        /// A free-standing function: `fn process(n: usize) -> Result<()> { ...
        /// }`.
        Fn(ItemFn),

        /// A block of foreign items: `extern "C" { ... }`.
        ForeignMod(ItemForeignMod),

        /// An impl block providing trait or associated items: `impl<A> Trait
        /// for Data<A> { ... }`.
        Impl(ItemImpl),

        /// A macro invocation, which includes `macro_rules!` definitions.
        Macro(ItemMacro),

        /// A module or module declaration: `mod m` or `mod m { ... }`.
        Mod(ItemMod),

        /// A static item: `static BIKE: Shed = Shed(42)`.
        Static(ItemStatic),

        /// A struct definition: `struct Foo<A> { x: A }`.
        Struct(ItemStruct),

        /// A trait definition: `pub trait Iterator { ... }`.
        Trait(ItemTrait),

        /// A trait alias: `pub trait SharableIterator = Iterator + Sync`.
        TraitAlias(ItemTraitAlias),

        /// A type alias: `type Result<T> = std::result::Result<T, MyError>`.
        Type(ItemType),

        /// A union definition: `union Foo<A, B> { x: A, y: B }`.
        Union(ItemUnion),

        /// A use declaration: `use std::collections::HashMap`.
        Use(ItemUse),

        /// Tokens forming an item not interpreted by Syn.
        Verbatim(TokenStream),

        // For testing exhaustiveness in downstream code, use the following idiom:
        //
        //     match item {
        //         #![cfg_attr(test, deny(non_exhaustive_omitted_patterns))]
        //
        //         Item::Const(item) => {...}
        //         Item::Enum(item) => {...}
        //         ...
        //         Item::Verbatim(item) => {...}
        //
        //         _ => { /* some sane fallback */ }
        //     }
        //
        // This way we fail your tests but don't break your library when adding
        // a variant. You will be notified by a test failure when a variant is
        // added, so that you can add code to handle it, but your library will
        // continue to compile and work for downstream users in the interim.
    }
```
```
```
### ItemUse
```rust

    /// A use declaration: `use std::collections::HashMap`.
    #[cfg_attr(docsrs, doc(cfg(feature = "full")))]
    pub struct ItemUse {
        pub attrs: Vec<Attribute>,
        pub vis: Visibility,
        pub use_token: Token![use],
        pub leading_colon: Option<Token![::]>,
        pub tree: UseTree,
        pub semi_token: Token![;],
    }
```
...
### UseTree
```rust

    /// A suffix of an import tree in a `use` item: `Type as Renamed` or `*`.
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: crate::expr::Expr#syntax-tree-enums
    #[cfg_attr(docsrs, doc(cfg(feature = "full")))]
    pub enum UseTree {
        /// A path prefix of imports in a `use` item: `std::...`.
        Path(UsePath),

        /// An identifier imported by a `use` item: `HashMap`.
        Name(UseName),

        /// An renamed identifier imported by a `use` item: `HashMap as Map`.
        Rename(UseRename),

        /// A glob import in a `use` item: `*`.
        Glob(UseGlob),

        /// A braced group of imports in a `use` item: `{A, B, C}`.
        Group(UseGroup),
    }
```

### UseTree

 ```rust
    /// A path prefix of imports in a `use` item: `std::...`.
    #[cfg_attr(docsrs, doc(cfg(feature = "full")))]
    pub struct UsePath {
        pub ident: Ident,
        pub colon2_token: Token![::],
        pub tree: Box<UseTree>,
    }
  ```
