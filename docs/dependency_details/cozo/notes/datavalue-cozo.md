# Critical Excerpts from `cozo` crate source code:

## DataValue type
Many of the below examples are somewhat obvious, but we are **critically underusing** the `DataValue::List` variant.
```rust
/// A Value in the database
#[derive(
Clone, PartialEq, Eq, PartialOrd, Ord, serde_derive::Deserialize, serde_derive::Serialize, Hash,
)]
pub enum DataValue {
    /// null
    Null,
    /// boolean
    Bool(bool),
    /// number, may be int or float
    Num(Num),
    /// string
    Str(SmartString<LazyCompact>),
    /// bytes
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),
    /// UUID
    Uuid(UuidWrapper),
    /// Regex, used internally only
    Regex(RegexWrapper),
    /// list
    List(Vec<DataValue>),
    /// set, used internally only
    Set(BTreeSet<DataValue>),
    /// Array, mainly for proximity search
    Vec(Vector),
    /// Json
    Json(JsonData),
    /// validity,
    Validity(Validity),
    /// bottom type, used internally only
    Bot,
}
```

We should consider how the `DataValueList` variant can be used to improve our `schema` and queries:

### On `Datavalue::List` Usage

1. **List Handling in Cozo**:
- CozoScript fully supports querying nested Lists. The syntax would be:
```cozo
// Find items visible to a specific path
?[id] := *visibility[id, 'restricted', path],
         contains(path, 'my_module')
```
- Lists can be:
  - Indexed: `path[0]` gets first element
  - Checked for membership: `contains(path, 'x')`
  - Pattern matched: `path matches ['super', 'module', _]`

2. **DataValue Insights**:
From the source, key observations:
- Lists maintain ordering (important for visibility paths)
- Comparison operations work depth-wise on lists
- List elements can be heterogenous but we'll use homogeneous String lists
- Memory efficiency is good since it uses SmartString

3. **Improved Visibility Design**:

```rust
// In traits.rs
fn visibility_to_cozo(v: VisibilityKind) -> (String, Option<DataValue>) {
    match v {
        VisibilityKind::Public => ("public".into(), None),
        VisibilityKind::Crate => ("crate".into(), None),
        VisibilityKind::Restricted(path) => {
            let list = DataValue::List(path.into_iter().map(DataValue::from).collect());
            ("restricted".into(), Some(list))
        }
        VisibilityKind::Inherited => ("inherited".into(), None),
    }
}
```

4. **Enhanced Schema**:
```cozo
:create visibility {
    node_id: Int =>
    kind: String,
    path: [String]?  // More idiomatic than List[String]
}

::index create visibility:by_kind_path {kind, path, node_id}
```

5. **Query Examples**:
```cozo
// Find all items visible to current module's parent
?[id] := *visibility[id, 'restricted', path],
         path matches ['super', ..]

// Find crate-local items
?[id] := *visibility[id, $kind, _],
         kind in ['crate', 'public']

// Check specific visibility
?[is_visible] := *visibility[$node_id, 'restricted', ['my', 'module']],
                 is_visible = true
```

