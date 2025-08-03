

The examples below can be found in `tests/fixture_crates/fixture_nodes/src/imports.rs` 
[here](./../../../tests/fixture_crates/fixture_nodes/src/imports.rs).

### Query parent modules (explicit)

This is a query for structs that are nested three deep in a module, including containing files.


```
?[name, module_name, mod_parent_name, mod_grandparent_name] := *struct {id: struct_id, name},
  *module {id: module_id, name: module_name},
  *module {id: mod_parent_id, name: mod_parent_name},
  *module {id: mod_grandparent_id, name: mod_grandparent_name},
  *syntax_edge { source_id: module_id, target_id: struct_id },
  *syntax_edge { source_id: mod_parent_id, target_id: module_id },
  *syntax_edge { source_id: mod_grandparent_id, target_id: mod_parent_id }
```

Returns:

|name|module_name|mod_parent_name|mod_grandparent_name|
|-----|-----|-----|-----|
|"NestedItem"|"nested_sub"|"sub_imports"|"imports"|


### All modules ancestors of a struct (recursive)

Query:
```
parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

?[struct_name, module_name] := *struct {id: struct_id, name: struct_name},
    *module {id: module_id, name: module_name},
    parent_of[struct_id, module_id]
```


Returns:

|struct_name|module_name|
|-----|-----|
|"AttributedStruct"|"structs"|
|"Container"|"const_static"|
|"DocumentedStruct"|"structs"|
|"DummyTraitUser"|"imports"|
|"GenTraitImpl"|"imports"|
|"GenericStruct"|"impls"|
|"GenericStruct"|"structs"|
|"InnerStruct"|"inner"|
|"NestedItem"|"nested_sub"|
|"PrivateStruct"|"impls"|
|"SampleStruct"|"structs"|
|"SimpleStruct"|"const_static"|
|"SimpleStruct"|"impls"|
|"SubItem"|"sub_imports"|
|"TupleStruct"|"structs"|
|"UnitStruct"|"structs"|


### All module, function, struct nodes within target file (not including file-level mod itself)
- Recursively queries the parents of each item, stopping with the target file.
- Does not include the file-based module which is the parent itself.
- arbitrarily has a limit of 12, which could just as easily be removed
  - same re: sorting by hash (which is actually not sorting by hash, see cozo docs on uuid ordering)


```
parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc],
    *file_mod{owner_id: asc}

needs_embedding[id, name, hash, span] :=
    *module{id, name, tracking_hash: hash, span, embedding }
    or *function{id, name, tracking_hash: hash, span, embedding }
    or *struct{id, name, tracking_hash: hash, span, embedding},
    !is_null(embedding)

is_root_module[id] := *module{id}, *file_mod {owner_id: id}

batch[id, name, target_file, file_hash, hash, span, namespace] :=
    needs_embedding[id, name, hash, span],
    ancestor[id, mod_id],
    is_root_module[mod_id],
    *module{id: mod_id, tracking_hash: file_hash },
    *file_mod {owner_id: mod_id, file_path: target_file, namespace },
    target_file = "crates/ploke-tui/src/lib.rs"

?[id, name, target_file, file_hash, hash, span, namespace] :=
    batch[id, name, target_file, file_hash, hash, span, namespace]
    :sort id
    :limit 10
```
