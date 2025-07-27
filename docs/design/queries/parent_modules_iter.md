

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
