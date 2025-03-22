## Target Schema Design Framework
**Key Entities (Per [datatypes.rst])**
| Rust Item          | Cozo Node Name | Key Relations                  | Metadata Example                |
|--------------------|----------------|--------------------------------|----------------------------------|
| Function           | `function`     | `has_param`, `returns`         | `{async: bool, unsafe: bool}`   |
| Struct             | `struct`       | `has_field`, `impl_for`        | `{repr: Option<String>}`        |  
| Enum               | `enum`         | `has_variant`                  | `{repr: Option<String>}`        |
| Trait              | `trait`        | `requires_trait`, `provides`   | `{auto: bool, unsafe: bool}`    |
| Impl Block         | `impl`         | `for_type`, `implements`       | `{trait: "..."}`                |
| Module             | `module`       | `contains`                     | `{path: "..."}`                 |
| Type Alias         | `type_alias`   | `aliases`                      | `{generics: [...]}`             |
| Macro              | `macro`        | `expands_to`                   | `{hygiene: "..."}`              |


```rust
// syn::Item
pub enum Item {
    Const(ItemConst),
    Enum(ItemEnum),
    ExternCrate(ItemExternCrate),
    Fn(ItemFn),
    ForeignMod(ItemForeignMod),
    Impl(ItemImpl),
    Macro(ItemMacro),
    Mod(ItemMod),
    Static(ItemStatic),
    Struct(ItemStruct),
    Trait(ItemTrait),
    TraitAlias(ItemTraitAlias),
    Type(ItemType),
    Union(ItemUnion),
    Use(ItemUse),
    Verbatim(TokenStream),
}
```

```rust
// syn::ItemConst
pub struct ItemConst {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub const_token: Const,
    pub ident: Ident,
    pub generics: Generics,
    pub colon_token: Colon,
    pub ty: Box<Type>,
    pub eq_token: Eq,
    pub expr: Box<Expr>,
    pub semi_token: Semi,
}
  ```
```rust
// syn::ItemEnum
pub struct ItemEnum {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub enum_token: Enum,
    pub ident: Ident,
    pub generics: Generics,
    pub brace_token: Brace,
    pub variants: Punctuated<Variant, Comma>,
}
```
```rust
// syn::ItemExternCrate
pub struct ItemExternCrate {
  // fields
}
```
We won't handle Extern for now, probably we won't handle it ever.

### syn::ItemFn
 ```rust
pub struct ItemFn {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub block: Box<Block>,
}
```

```rust
// syn::Attribute
// Shared by most (all?) Item____ structs
pub struct Attribute {
    pub pound_token: Pound,
    pub style: AttrStyle,
    pub bracket_token: Bracket,
    pub meta: Meta,
}
```

```rust
// syn::Meta
pub enum Meta {
    //A meta path is like the test in #[test]. 
    Path(Path), 
    // A name-value meta is like
    // the path = "..." in #[path = "sys/windows.rs"]. 
    List(MetaList), 
    // A name-value pair within an attribute, like feature = "nightly".
    NameValue(MetaNameValue),
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
// syn::Stmt
pub enum Stmt {
    // A local (let) binding.
    Local(Local),
    // An item definition.
    // !!! Recursive: Item(ItemFn) -> Block -> Stmt -> Item(_)
    Item(Item),
    Expr(Expr, Option<Semi>),
    // We won't handle macros beyond tracking macro names
    Macro(StmtMacro),
}
```

```rust
pub struct Block {
    pub brace_token: Brace,
    pub stmts: Vec<Stmt>,
}
```


```rust
// syn::Type
pub enum Type {
    Array(TypeArray),
    BareFn(TypeBareFn),
    Group(TypeGroup),
    ImplTrait(TypeImplTrait),
    Infer(TypeInfer),
    Macro(TypeMacro),
    Never(TypeNever),
    Paren(TypeParen),
    Path(TypePath),
    Ptr(TypePtr),
    Reference(TypeReference),
    Slice(TypeSlice),
    TraitObject(TypeTraitObject),
    Tuple(TypeTuple),
    Verbatim(TokenStream),
}
```

## CozoScript Schema Definition
```cozoscript
/* Nodes = All Definable Entities */
::create nodes {
    id: Uuid,          // Deterministic UUIDv5 (path + item name)
    kind: String,      // Type from table above
    name: String,      // Qualified name via RFC #926
    meta: Json?,       // Item-specific metadata
    valid_from: Validity  // Timestamp, per temporal docs
}

/* Relations = Code Structure Graph */
::create relations {
    source: Uuid,      // From node.id 
    target: Uuid,      // To node.id
    rel_type: String,  // From table column 3
    meta: Json?        // Position data, modifiers, etc.
}
```

**Critical Cozo Features Used:**
1. **UUID as Primary Key** ([datatypes.rst#Uuid])
   - Stable IDs via hash of item path + name space
   - Enables fast joins across tables

2. **Validity Type** ([datatypes.rst#Validity])
   - Track code evolution: older versions remain queryable
   - Example: Find all functions as of commit X

3. **JSON Column** ([datatypes.rst#Json])
   - Flexible storage for variant metadata shapes
   - Cozo supports partial indexing on JSON paths

## Immediate Implementation Plan

**1. Schema Migration**
```rust
// Replaces current CodeGraph struct with Cozo-native relations
fn batch_push(&mut self, table: &str, row: Vec<DataValue>) {
    // Convert existing NodeId-based relations to UUIDs
}
```

**2. Transaction Wrapper** ([stored.rst#Chaining])
```rust
impl CodeVisitorV2 {
    pub fn run_transaction<F>(db: &Db, f: F) 
    where F: FnOnce(&mut Self) {
        db.run_script("{?[] <- [] :start_txn}", ...).unwrap();
        let mut visitor = Self::new(db);
        f(&mut visitor);
        visitor.flush_all();
        db.run_script("{?[] <- [] :commit}", ...).unwrap();
    }
}
```

**3. Function Processing Example**
```rust
fn visit_item_fn(&mut self, item: &ItemFn) {
    let fn_id = item.to_uuid(); // Implement RFC 926 style path hashing
    
    // Store function node
    self.batch_push("nodes", vec![
        UuidWrapper(fn_id),
        "function".into(),
        item.sig.ident.to_string().into(),
        json!({"async": item.sig.asyncness.is_some()}),
        DataValue::Validity(/* timestamp */),
    ]);
    
    // Process parameters (demoing relationships)
    for param in &item.sig.inputs {
        let param_id = param.to_uuid(fn_id);
        self.batch_push("nodes", param.node_data());
        self.batch_push("relations", vec![
            UuidWrapper(fn_id),
            UuidWrapper(param_id),
            "has_param".into(),
            json!({"position": index}),
        ]);
    }
}
```

**Key Optimization (Per [stored.rst#Indices])**
```cozoscript
::index create nodes.by_kind { kind, valid_from => id }
::index create relations.by_type { rel_type, valid_from }
```

## Query Patterns for RAG Context
```cozoscript
// Get all functions modifying a target struct
?[fn_body] := 
    *nodes[struct_id, "struct", "TargetStruct"],
    *relations[impl_id, struct_id, "for_type"],
    *relations[impl_id, fn_id, "contains"],
    *code_snippets[fn_id, fn_body]
    valid_from @ '2024-02-01' // Temporal query
    
// Find traits a type indirectly implements
?[trait_name, depth] := 
    *nodes[type_id, "struct", "MyType"],
    relations*[type_id, trait_id, "requires_trait", depth: 1..5],
    *nodes[trait_id, "trait", trait_name]
    :order +depth  // From low to high specificity
```

**Implementation Checklist**
1. UUID generation service for all item types
2. Metadata schema per entity type (valid JSON shapes)
3. Batch size autotuning based on MemStorage limits
4. Temporal version hooks (git commit timestamps?)

Would any component benefit from deeper implementation guidance?


---

### **Understanding `%debug`** (Per Request #4)

From [stored.rst#L209-257](source/stored.rst):
> "The `%debug` command prints the contents of ephemeral relations (tables starting with `_`) to stdout during script execution."

**Key Characteristics:**
- *Works Only in Chained Queries*: Requires transaction blocks (`{}` in scripts)
- *Ephemeral Relations Only*: For tables named like `_temp_data`
- *Atomic Printing*: Shows relation state at the debug point

**Example Usage with Tests:**
```rust
db.run_script(
    r#"{
        ?[a] <- [[1], [2]]
        :replace _test
    }
    %debug _test
    {
        :rm _test{a}
    }
    "#,
    Default::default(),
    ScriptMutability::Mutable,
);
// Prints _test contents between operations
```

---

### Critical Areas Needing Validation

1. **Epoch Handling**  
Cozo's [timetravel docs](source/timetravel.rst) mention validity tracking - tests should verify that code version timestamps are recorded correctly.

2. **Second-Axis Indexing**  
If implementing scope stacks later:
```rust
{
    parent[ancestor] := *relations[child, ancestor, 'contains']
    parent[parent_id] := parent[child, parent_id],
                        parent[parent_id, ancestor]
    ?[depth] := parent['root_scope_id', 'current_fn_scope_id', depth]
}
```

Let me know if you want deeper analysis on specific query patterns!
