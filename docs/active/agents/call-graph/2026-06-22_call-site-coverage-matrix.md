# Call-site coverage matrix

Date: 2026-06-22
Status: active test-planning matrix
Scope: enumerate parser call-site shapes, fixture candidates, expected structural classification, and current/future resolver outcome expectations before adding more extraction/resolution behavior.

## Purpose

Do not broaden call-graph behavior opportunistically. For each new extractor or resolver slice, choose a row from this matrix, add/adjust the focused test first, then implement only the behavior needed for that row.

The intended invariant is:

```text
source Rust expression shape -> one structural CallNode class -> one resolver status
```

A resolved edge is optional and only appears when semantic proof is available.

## Structural classes

| Rust shape | `syn` shape | Structural class | Notes |
|---|---|---|---|
| `helper()` | `ExprCall(func = ExprPath)` | `PathCall` | May later resolve to `FunctionNodeId`. |
| `module::helper()` | `ExprCall(func = ExprPath)` | `PathCall` | Path can be local, imported, external, or unresolved. |
| `Type::new()` / `Self::new()` | `ExprCall(func = ExprPath)` | `PathCall` | Constructor/associated-function-shaped; target may be `MethodNodeId` or constructor target later. |
| `TupleStruct(...)` | `ExprCall(func = ExprPath)` | `PathCall` | Future semantic target is constructor, not function. |
| `Enum::Variant(...)` | `ExprCall(func = ExprPath)` | `PathCall` | Future semantic target is tuple variant constructor. |
| `receiver.method()` | `ExprMethodCall` | `MethodCall` | Receiver classification must grow conservatively. |
| `self.method()` | `ExprMethodCall` | `MethodCall` | First resolved slice: exact inherent local method. |
| `self.field.len()` | `ExprMethodCall` | `MethodCall` | Should be structural method call; likely unresolved/external/unsupported until receiver typing exists. |
| `closure(...)` where `closure` is a binding | `ExprCall(func = ExprPath)` | TBD | Structurally path-shaped, semantically dynamic. Decide whether to keep `PathCall` plus later binding-aware status or reclassify as `DynamicCall`. |
| `(closure)(...)` | `ExprCall(func != ExprPath)` | `DynamicCall` | Requires fixture if not already present. |
| `make_fn()(...)` | nested `ExprCall(func = ExprCall)` | `DynamicCall` outer call | Requires fixture if not already present. |
| `macro_name!(...)` | `ExprMacro` | `MacroCall` | Invocation site only; no expansion in parser-native structural pass. |

## Current green fixture rows

| Test | Fixture expression | Expected structural class | Resolver expectation |
|---|---|---|---|
| `fixture_nodes_public_method_records_self_private_method_call_site` | `self.private_method()` in `impls.rs` | `MethodCall` | structural only test |
| `fixture_nodes_public_method_resolves_self_private_method_edge` | same | `MethodCall` | `Resolved(LocalExact)` plus `CallRelation::Method` |
| `fixture_nodes_use_imported_items_records_pathbuf_new_path_call_site` | `PathBuf::new()` in `imports.rs` | `PathCall` | `Unsupported`, no function edge |
| `fixture_nodes_use_imported_items_records_documented_macro_call_site` | `documented_macro!(...)` in `imports.rs` | `MacroCall` | `Unsupported`, no resolved edge |

## Candidate fixture rows already present

### Path calls

- `HashMap::<String, i32>::new()` in `fixture_nodes/src/imports.rs` — generic arg count path call.
- `fs::read_to_string("dummy")` in `fixture_nodes/src/imports.rs` — imported/external module-qualified path call.
- `EnumWithData::Variant1(1)` in `fixture_nodes/src/imports.rs` — tuple variant constructor-shaped path call.
- `Duration::from_secs(1)` in `fixture_nodes/src/imports.rs` — external associated-function-shaped path call.
- `Arc::new(1)` in `fixture_nodes/src/imports.rs` — external associated-function-shaped path call.
- `TupleStruct(1, 2)` in `fixture_nodes/src/imports.rs` — tuple struct constructor-shaped path call.
- `five()` in `fixture_nodes/src/const_static.rs` — const initializer call; owner family must be extended before extraction.

### Method calls

- `self.private_method()` in `fixture_nodes/src/impls.rs` — local inherent method, currently resolved.
- `self.secret.len()` in `fixture_nodes/src/impls.rs` — method on field receiver, should not fabricate local edge.
- `self.value.len()` in `fixture_nodes/src/impls.rs` — method on generic/string-like field receiver.
- `self.value.into()` in `fixture_nodes/src/impls.rs` — conversion method on generic receiver.
- `self.content.to_uppercase()` and `self.is_high_priority()` in `fixture_nodes/src/new_test_module.rs` are in a non-exported module from `lib.rs` today, so use only after fixture/module routing is clarified.

### Macro calls

- `documented_macro!(fixture alias coverage)` in `fixture_nodes/src/imports.rs` — currently structural macro-call green.
- `println!(...)` in `fixture_nodes/src/const_static.rs` — standard macro invocation.
- `format!(...)` in `fixture_nodes/src/new_test_module.rs` — use only after module routing is clarified.

### Dynamic / binding-sensitive calls

- `alias_checker(&_trait_user)` in `fixture_nodes/src/imports.rs` — path-shaped call to closure binding; classification policy still needs a decision.
- `cfg_checker(&_trait_user)` in `fixture_nodes/src/imports.rs` under cfg block — useful after cfg expectations are explicit.
- No obvious `(f)()` / `make_fn()()` fixture row in `fixture_nodes`; add a fixture only after an explicit fixture-change task.

## Status expectations

Every call site considered by `resolve_call_relations_after_tree` should eventually have exactly one status:

| Structural class | Current status policy |
|---|---|
| `MethodCall` with literal `self`, inherent impl owner, unique same-impl name | `Resolved(LocalExact)` + typed `CallRelation::Method` |
| `MethodCall` unsupported receiver or trait/default context | `Unsupported` or more specific future status, no edge |
| `PathCall` | currently `Unsupported`, no edge |
| `MacroCall` | currently `Unsupported`, no edge |
| `DynamicCall` | future `Unsupported`, no edge until target-set proof exists |

## Next recommended test slices

1. Add green assertions that current path and macro rows receive `Unsupported` statuses.
2. Add RED structural method-call test for `self.secret.len()` to drive non-literal receiver classification without resolving it.
3. Decide the policy for path-to-binding calls such as `alias_checker(...)` before implementing dynamic extraction.
4. Add an explicit fixture for `(f)()` or `make_fn()()` if no suitable existing fixture row exists.
