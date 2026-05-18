# Type Resolution Coverage

Date: 2026-05-18

This document inventories the current tests for type resolution and the typed type graph. It focuses on the code items under test, where they are tested, which fixture source they come from, the property asserted, the IDs or selectors involved, and the current pass/fail state.

The shared real-corpus matrix lives in `crates/test-utils/src/type_shape_matrix.rs` and is the authority for source-pinned DB/RAG/TUI coverage. Parser fixture tests are summarized by macro table because those tests are generated from in-file rows; DB/RAG/TUI corpus rows are listed one per case.

## Status Snapshot

| Layer | Command or source | Current state on 2026-05-18 | Notes |
| --- | --- | --- | --- |
| Parser typed fixture relations | `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture` | Pass: `64 passed; 0 failed; 0 ignored` | Focused test-runner verification during this document pass. |
| DB typed graph queries | `cargo test -p ploke-db --features typed_type_graph unit::type_graph_queries -- --nocapture` | Pass: `75 passed; 0 failed; 6 ignored` | Discovery sub-agent ran the DB query suite during this pass. Ignored tests are full source-parse corpus variants. |
| DB shared corpus matrix | `cargo test -p ploke-db --features typed_type_graph corpus_matrix_ -- --nocapture` | Pass, included in current `type_graph_queries` run | Matrix rows below are ordinary executable contracts over registered backups. |
| RAG shared corpus matrix | `PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs cargo test -p ploke-rag --features typed_type_graph corpus_type_shape_matrix -- --nocapture` | Pass: `1 passed; 0 failed; 0 ignored` | Focused test-runner verification during this document pass. |
| TUI direct matrix | `request_code_context_tool_emits_matrix_type_context` | Ignored/quarantined | This test is intentionally not trusted because it starts from BM25 search terms. Explicit ignored runs are known to fail on search-seed/type-context mismatch. |
| TUI live matrix | `live_request_code_context_matrix_uses_production_tool_payload -- --ignored` | Ignored/manual; not run by default | Provider-spending path. It should not be used as proof until event correlation and seed identity assertions are tightened. |
| Workspace | `PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs cargo test --workspace` | Pass in latest recovery verification | Verified after the TUI config-env isolation fix and before this document was created. |

## Fixture Inventory

| Fixture | Source and checkout | Backup location | Used for | Status |
| --- | --- | --- | --- | --- |
| `fixture_type_resolution_v2` | <pre><code>tests/fixture_crates/fixture_type_resolution_v2</code></pre> | Fresh parse, no backup required | Parser v2 relation tests; DB direct root/reachability/invariant tests | Pass in typed fixture suites |
| `fixture_types` | <pre><code>tests/fixture_crates/fixture_types</code></pre> | Fresh parse, no backup required | Parser imported type aliases and function type roots; DB function return/root coverage | Pass in fixture suites |
| `fixture_nodes` | <pre><code>tests/fixture_crates/fixture_nodes</code></pre> | `tests/backup_dbs/fixture_nodes_canonical_2026-05-17.sqlite`; local embedding variant `fixture_nodes_local_embeddings_2026-05-17.sqlite` | Parser broad fixture coverage; RAG local embedding type-context smoke | Pass |
| `fixture_conflation` | <pre><code>tests/fixture_crates/fixture_conflation</code></pre> | Fresh parse, no backup required | Parser import/conflation and nested generic field coverage | Pass in parser typed suite |
| `corpus_semver` | <pre><code>github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90</code></pre> | `tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite`; searchable `corpus_semver_openrouter_embeddings_2026-05-17.sqlite` | Function params, reference containment, field generic args, method params | Pass for backup contracts |
| `corpus_memchr` | <pre><code>github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905</code></pre> | `tests/backup_dbs/corpus_memchr_type_graph_2026-05-17.sqlite`; searchable `corpus_memchr_openrouter_embeddings_2026-05-17.sqlite` | Named returns, function pointer containment, raw pointer/generic param terminal | Pass for backup contracts |
| `corpus_generic_array` | <pre><code>github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite`; searchable `corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite` | Qualified projections, aliases, trait bounds, where subjects/bounds, impl roots, impl trait params | Pass for backup contracts |
| `corpus_chrono` | <pre><code>github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite`; searchable `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite` | Named generic args, arrays, tuples, slices, statics, consts, method returns, generic bounds, where bounds, associated type bounds | Pass for backup contracts |
| `corpus_axum` | <pre><code>github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1
workspace targets: axum, axum-core, axum-macros</code></pre> | `tests/backup_dbs/corpus_axum_type_graph_2026-05-17.sqlite`; searchable `corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | Trait objects, impl trait, macro and paren no-target structural cases | Pass for DB/RAG seeded paths; TUI search-driven matrix is quarantined |

## Parser Fresh-Fixture Coverage

File: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`

Feature: `typed_type_graph`

Fixture locations: `tests/fixture_crates/fixture_type_resolution_v2`, `tests/fixture_crates/fixture_types`, `tests/fixture_crates/fixture_nodes`, `tests/fixture_crates/fixture_conflation`

The parser test file has 64 generated tests: 60 single-relation rows through `type_relation_cases!` and 4 exact-source tests through `type_relations_exact_sources_case!`.

| Test or group | Code items tested | Fixture | Property tested | IDs/selectors involved | Current state |
| --- | --- | --- | --- | --- | --- |
| `v2_generic_param_shadows_module_item_with_same_name` | <pre><code>fn generic_shadow&lt;T&gt;(arg: T)</code></pre> | `fixture_type_resolution_v2` | Generic parameter shadows module item of the same name. | Owner `item(["crate"], "generic_shadow", Function)`; slot `FunctionParam(0)`; target generic param `T`. | Pass in typed parser suite |
| `v2_resolves_ordinary_item_path_when_no_generic_param_shadows_it` | <pre><code>fn concrete(arg: T)</code></pre> | `fixture_type_resolution_v2` | Ordinary item path resolves to local struct `T` when no generic shadows it. | Owner `concrete`; slot `FunctionParam(0)`; ordinary target `struct T`. | Pass |
| `v2_resolves_trait_position_supertrait` | <pre><code>trait ChildTrait: LocalTrait</code></pre> | `fixture_type_resolution_v2` | Trait-position supertrait resolves as trait relation. | Owner `ChildTrait`; slot `TraitSuper(0)`; target `LocalTrait`. | Pass |
| `v2_resolves_generic_declaration_bound` | <pre><code>struct LocallyBound&lt;T: LocalTrait&gt;</code></pre> | `fixture_type_resolution_v2` | Generic declaration bound resolves to trait target. | Owner `LocallyBound`; slot `GenericParamBound { param_index: 0, bound_index: 0 }`; target `LocalTrait`. | Pass |
| `v2_resolves_qualified_projection_trait_qualifier` | <pre><code>type ProjectedArrayLength = &lt;Const&lt;N&gt; as IntoArrayLength&gt;::ArrayLength</code></pre> | `fixture_type_resolution_v2` | Qualified projection trait qualifier resolves as trait relation. | Owner `ProjectedArrayLength`; slot `TypeAliasTarget`; terminal `named(["IntoArrayLength"])`; target `IntoArrayLength`. | Pass |
| `v2_resolves_associated_type_bound` | <pre><code>trait LocalAssocBound { type Output: LocalTrait; }</code></pre> | `fixture_type_resolution_v2` | Associated type bound resolves to trait target from containing trait. | Owner `LocalAssocBound`; slot `AssociatedTypeBound(0)`; target `LocalTrait`. | Pass |
| `v2_resolves_where_direct_type_param_*` | <pre><code>struct WhereLocal&lt;T&gt; where T: LocalTrait</code></pre> | `fixture_type_resolution_v2` | Where subject and where bound are separate exact source slots. | Subject slot `WherePredicateSubject(0)` to generic param `T`; bound slot `WherePredicateBound { predicate_index: 0, bound_index: 0 }` to `LocalTrait`. | Pass |
| `v2_resolves_where_multi_bound_*` | <pre><code>where T: LocalTrait + ExtraTrait</code></pre> | `fixture_type_resolution_v2` | Multiple bounds on one predicate keep distinct `bound_index` values. | Bound slots `(0, 0)` and `(0, 1)`; target traits `LocalTrait`, `ExtraTrait`; subject `T`. | Pass |
| `v2_resolves_where_multi_predicate_*` | <pre><code>where T: LocalTrait, U: ExtraTrait</code></pre> | `fixture_type_resolution_v2` | Multiple predicates keep distinct `predicate_index` values. | Subject slots `0`, `1`; bound coordinates `(0,0)`, `(1,0)`; targets `T`, `U`, `LocalTrait`, `ExtraTrait`. | Pass |
| `v2_resolves_where_repeated_subject_*` | <pre><code>where T: LocalTrait, T: ExtraTrait</code></pre> | `fixture_type_resolution_v2` | Repeated same-subject predicates remain distinguishable. | Subject slots `0`, `1`; bound slots `(0,0)`, `(1,0)`; both subjects target generic param `T`. | Pass |
| `v2_resolves_where_enum_*` | <pre><code>enum WhereEnum&lt;T&gt; where T: LocalTrait</code></pre> | `fixture_type_resolution_v2` | Enum owner where subject and bound are represented. | Owner `WhereEnum`; `WherePredicateSubject(0)`; `WherePredicateBound(0,0)`. | Pass |
| `v2_resolves_where_type_alias_*` | <pre><code>type WhereAliasMulti&lt;T&gt; where T: LocalTrait + ExtraTrait = ...</code></pre> | `fixture_type_resolution_v2` | Type alias owner where subject and multi-bound slots are represented. | Owner `WhereAliasMulti`; subject `T`; bounds `LocalTrait`, `ExtraTrait`. | Pass |
| `v2_resolves_where_impl_*` | <pre><code>impl&lt;T&gt; LocalTrait for WhereImpl&lt;T&gt; where T: ExtraTrait + AnotherTrait</code></pre> | `fixture_type_resolution_v2` | Impl owner where subject and multi-bound slots are represented. | Owner `impl_block(... WhereImpl ... LocalTrait)`; subject `T`; bounds `ExtraTrait`, `AnotherTrait`. | Pass |
| `v2_resolves_where_composite_subject_*` | <pre><code>where Vec&lt;T&gt;: LocalTrait</code></pre> | `fixture_type_resolution_v2` | Composite where subject preserves nested terminal `T`. | Owner `WhereComposite`; subject slot `WherePredicateSubject(0)`; terminal path `named(["T"])`; target generic param `T`. | Pass |
| `v2_resolves_where_composite_multi_*` | <pre><code>where Vec&lt;T&gt;: LocalTrait + ExtraTrait</code></pre> | `fixture_type_resolution_v2` | Composite subject plus multiple bounds are represented with exact coordinates. | Subject terminal `T`; bound slots `(0,0)`, `(0,1)` to `LocalTrait`, `ExtraTrait`. | Pass |
| `v2_resolves_where_projection_subject_*` | <pre><code>where &lt;T as LocalAssocBound&gt;::Output: LocalTrait</code></pre> | `fixture_type_resolution_v2` | Projection subject exposes the trait qualifier terminal. | Subject slot `WherePredicateSubject(0)`; terminal `LocalAssocBound`; target trait `LocalAssocBound`. | Pass |
| `v2_resolves_where_projection_multi_*` | <pre><code>where &lt;T as LocalAssocBound&gt;::Output: LocalTrait + ExtraTrait</code></pre> | `fixture_type_resolution_v2` | Projection subject with multiple bounds keeps exact subject and bound coordinates. | Subject terminal `LocalAssocBound`; bounds `LocalTrait`, `ExtraTrait`. | Pass |
| `fixture_types_v2_imported_*` | <pre><code>consumes_point(Point)
math_operation_consumer(MathOperation)
math_operation_producer() -&gt; MathOperation</code></pre> | `fixture_types` | Imported aliases in function params and returns resolve to the correct type alias items, including nested module duplicate cases. | Owners under `["crate", "func", "return_types"]`; slots `FunctionParam(0)`, `FunctionReturn`; targets `Point`, `MathOperation`. | Pass |
| `fixture_nodes_v2_inner/local impl cases` | <pre><code>impl SimpleStruct
impl SimpleTrait for InnerStruct
impl SimpleTrait for SimpleStruct</code></pre> | `fixture_nodes` | Impl self and impl trait sources resolve through imports and local definitions. | Owners `impl_block(...)`; slots `ImplSelf`, `ImplTrait`; ordinary targets `SimpleStruct`, `InnerStruct`; trait target `SimpleTrait`. | Pass |
| `fixture_nodes_v2_supertrait cases` | <pre><code>trait SuperTrait: SimpleTrait
trait MultiSuperTrait: SimpleTrait + InternalTrait
trait GenericSuperTrait&lt;T&gt;: GenericTrait&lt;T&gt;</code></pre> | `fixture_nodes` | Trait super slots resolve to exact trait targets and nested generic argument source is included in exact-source test. | Slots `TraitSuper(0)`, `TraitSuper(1)`; targets `SimpleTrait`, `InternalTrait`, `GenericTrait`, generic param `T`. | Pass |
| `fixture_nodes_v2_type_alias cases` | <pre><code>type IdAlias = SimpleId
type OuterPoint = Point
type UseInner = InnerPublic</code></pre> | `fixture_nodes` | Type alias target roots resolve to local aliases across modules/imports. | Slot `TypeAliasTarget`; targets `SimpleId`, `Point`, `InnerPublic`. | Pass |
| `fixture_nodes_v2_const_*` | <pre><code>const STRUCT_CONST: SimpleStruct
const ALIASED_CONST: MyInt</code></pre> | `fixture_nodes` | Const type annotations resolve to struct and alias targets. | Slot `ConstType`; targets `SimpleStruct`, `MyInt`. | Pass |
| `fixture_nodes_v2_field/variant/union/method generic cases` | <pre><code>GenericStruct&lt;T&gt;.field: T
JustTypeGeneric&lt;A&gt;::VariantA(A)
GenericUnion&lt;T&gt; { value: ManuallyDrop&lt;T&gt; }
GenericTrait&lt;T&gt; for GenericStruct&lt;T&gt;::generic_trait_method(value: T)</code></pre> | `fixture_nodes` | Field and method parameter type-use sources resolve to generic params owned by the correct enclosing item or impl. | Owner selectors `struct_field`, `enum_variant_field`, `union_field`, `method`; slots `FieldType`, `MethodParam(1)`; targets generic params `T`, `A`. | Pass |
| `fixture_nodes_v2_where_subject cases` | <pre><code>ComplexGeneric&lt;T&gt; where T: ...
impl&lt;T&gt; SimpleTrait for GenericStruct&lt;T&gt; where T: ...
GenericEnum&lt;T&gt; where T: ...
JustWhereClause&lt;T&gt; where T: ...</code></pre> | `fixture_nodes` | Where predicate subjects resolve to the intended generic param for aliases, impls, enums, and where-only enum shapes. | Slot `WherePredicateSubject(0)`; targets generic params owned by alias, impl, or enum. | Pass |
| `fixture_type_resolution_v2_projection_and_where_sources_are_exact` | <pre><code>ProjectedArrayLength
LocalAssocBound::Output
WhereComposite
WhereProjection</code></pre> | `fixture_type_resolution_v2` | Exact source set equality for selected projection and where sources; rejects extra rows for the same source. | `type_relations_exact_sources_case!`; terminals `IntoArrayLength`, `LocalTrait`, `T`, `LocalAssocBound`. | Pass |
| `fixture_type_resolution_v2_expanded_where_sources_are_exact` | <pre><code>WhereMultiBound
WhereMultiPredicate
WhereRepeatedSubject
WhereEnum
WhereAliasMulti
WhereImpl
WhereCompositeMulti
WhereProjectionMulti</code></pre> | `fixture_type_resolution_v2` | Exact source set equality for expanded where-clause cases. | Exact subject and bound coordinates across repeated, multi-bound, composite, and projection where predicates. | Pass |
| `fixture_nodes_v2_generic_trait_supertrait_complete_slot` | <pre><code>trait GenericSuperTrait&lt;T&gt;: GenericTrait&lt;T&gt;</code></pre> | `fixture_nodes` | Complete slot asserts both trait target and nested generic argument target. | Slot `TraitSuper(0)`; terminals `root`, `named(["T"])`; targets `GenericTrait`, generic param `T`. | Pass |
| `fixture_conflation_v2_*` | <pre><code>impl TopLevelTrait for inner_mod::InnerStruct
InnerStruct::inner_method() -&gt; InnerStruct
NestedGeneric&lt;T&gt;(TopLevelStruct&lt;T&gt;)</code></pre> | `fixture_conflation` | Import/conflation resolution and nested generic field exact-source behavior. | Owners `impl_block`, `method`, `struct_field`; slots `ImplTrait`, `MethodReturn`, `FieldType`; targets `TopLevelTrait`, `InnerStruct`, `TopLevelStruct`, generic param `T`. | Pass |

### Legacy Parser Type-Use Coverage

File: `crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_use_resolution.rs`

Feature state: compiled when `typed_type_graph` is disabled. This is the legacy late type-use resolver surface, not the active typed v2 relation model.

The legacy parser file has 24 generated tests: 21 single-edge tests through `type_use_resolution_case!` and 3 complete-slot tests through `type_use_slot_resolution_case!`.

| Test or group | Code items tested | Fixture | Property tested | IDs/selectors involved | Current state |
| --- | --- | --- | --- | --- | --- |
| `fixture_types_imported_*` | <pre><code>consumes_point(Point)
math_operation_consumer(MathOperation)
math_operation_producer() -&gt; MathOperation
restricted_duplicate::consumes_point(Point)
restricted_duplicate::math_operation_producer() -&gt; MathOperation</code></pre> | `fixture_types` | Legacy import-backed type-use resolution for function params and returns. | Owners under `["crate", "func", "return_types"]`; roles `FunctionParam`, `FunctionReturn`; targets `Point`, `MathOperation`; `expect_resolved_type_id: true`. | Pass when legacy resolver suite is compiled |
| `fixture_nodes_inner/local impl cases` | <pre><code>impl SimpleStruct
impl SimpleTrait for InnerStruct
SimpleStruct::new() -&gt; Self
impl SimpleTrait for SimpleStruct</code></pre> | `fixture_nodes` | Legacy impl self, impl trait, and method return resolution through imports and local definitions. | Selectors `impl_block`, `impl_selector`, `method`; roles `ImplSelf`, `ImplTrait`, `MethodReturn`; targets `SimpleStruct`, `SimpleTrait`, `InnerStruct`. | Pass when legacy resolver suite is compiled |
| `fixture_nodes_trait_super cases` | <pre><code>trait SuperTrait: SimpleTrait
trait MultiSuperTrait: SimpleTrait + InternalTrait
trait GenericSuperTrait&lt;T&gt;: GenericTrait&lt;T&gt;</code></pre> | `fixture_nodes` | Legacy trait super resolution and partial multi-supertrait coverage. | Role `TraitSuper`; slots `TraitSuper(0)`, `TraitSuper(1)`; targets `SimpleTrait`, `InternalTrait`, `GenericTrait`. | Pass; external/builtin supertraits are documented as partial |
| `fixture_nodes_generic_trait_supertrait_complete_slot` | <pre><code>trait GenericSuperTrait&lt;T&gt;: GenericTrait&lt;T&gt;</code></pre> | `fixture_nodes` | Complete slot multiset includes both trait target and nested generic parameter target. | `type_use_slot_resolution_case!`; slot targets `GenericTrait` and generic param `T`. | Pass when legacy resolver suite is compiled |
| `fixture_nodes_type_alias/const cases` | <pre><code>type IdAlias = SimpleId
type OuterPoint = Point
type UseInner = InnerPublic
const STRUCT_CONST: SimpleStruct
const ALIASED_CONST: MyInt</code></pre> | `fixture_nodes` | Legacy type alias targets and const type annotations resolve to local items. | Roles `TypeAliasTarget`, `Const`; targets `SimpleId`, `Point`, `InnerPublic`, `SimpleStruct`, `MyInt`. | Pass when legacy resolver suite is compiled |
| `fixture_conflation_*` | <pre><code>impl TopLevelTrait for inner_mod::InnerStruct
InnerStruct::inner_method() -&gt; InnerStruct
NestedGeneric&lt;T&gt;(TopLevelStruct&lt;T&gt;)</code></pre> | `fixture_conflation` | Legacy conflation/import resolution and complete-slot behavior for nested generic field. | Roles `ImplTrait`, `MethodReturn`, `Field`; complete-slot tests for imported trait impl and nested generic field. | Pass when legacy resolver suite is compiled |
| Legacy documented not-covered roles | <pre><code>static VALUE: LocalType
fn method_param(&amp;self, value: LocalType)
enum E { Variant(LocalType), Struct { field: LocalType } }
union U { field: LocalType }</code></pre> | N/A | The legacy file documents these as walker branches that are not asserted there. | N/A | Not covered in legacy file; many are covered by typed v2 and DB tests |
| Legacy documented failing/TDD target | <pre><code>struct UsesFileModule&lt;T&gt; { field: other_mod::OtherFileStruct&lt;T&gt; }</code></pre> | Future fixture target | File-module qualified path through module declaration/definition traversal remains a documented legacy resolver gap. | Would involve module declaration to definition backlink traversal. | Not an active passing test |

## DB Typed Graph Query Coverage

Files under `crates/ploke-db/tests/unit/type_graph_queries`.

| Test file and tests | Code items tested | Fixture | Property tested | IDs/selectors involved | Current state |
| --- | --- | --- | --- | --- | --- |
| `containment.rs`: `named_generic_arguments_preserve_order`, `tuple_elements_preserve_order`, `reference_type_has_single_referenced_child`, `function_pointer_type_contains_params_and_return`, `trait_object_contains_trait_bound_child`, `qualified_projection_contains_self_and_trait_qualifier` | <pre><code>Mapping&lt;K,V&gt;
Point tuple/alias shapes
StrSlice
MathOperation function pointer
DynDrawable trait object
ProjectedArrayLength qualified projection</code></pre> | `fixture_nodes`; `fixture_type_resolution_v2` | Direct `type_contains` edge shape, order, position, and cardinality. | `direct_type_contains`; `TypeContainmentKind::{Argument, Element, Referenced, FunctionParam, FunctionReturn, TraitBound, QualifiedSelf, QualifiedTrait}`. | Pass |
| `direct_roots.rs`: 9 tests | <pre><code>concrete(param)
apply_op() -&gt; i32
type Mapping = ...
LocallyBound&lt;T: LocalTrait&gt;
LocalAssocBound::Output
WhereLocal
WhereMultiBound
WhereRepeatedSubject
WhereComposite</code></pre> | `fixture_type_resolution_v2`, `fixture_types`, `fixture_nodes` | `type_uses_for_owner` exposes exact owner/root/role/coordinate rows before traversal. | `TypeUseRole`; `TypeUseCoordinate::{ParamSlot, GenericParamBoundSlot, AssociatedTypeBoundSlot, WhereSubjectSlot, WhereBoundSlot, WhereGenericParamBoundSlot}`. | Pass |
| `reachability.rs`: 9 tests | <pre><code>direct param struct target
reference trait object to trait
nested ordinary type
multi-terminal root
generic declaration bound
where direct type-param bound
multi-bound where traits
repeated-subject where traits
composite where subject nested type param</code></pre> | `fixture_type_resolution_v2`, `fixture_types` | Owner roots reach expected terminal targets with relation kind and depth. | `assert_type_use_reaches_target`; `assert_owner_reaches_target`; `TypeRelationKind`; exact `TypeUseCoordinate`. | Pass |
| `related_owners.rs`: 2 tests | <pre><code>owners related by shared target T</code></pre> | `fixture_type_resolution_v2` | Exact-root matches outrank nested-terminal matches for owner and target-centered queries. | `type_related_owners`; `type_owners_for_target`; target distance fields. | Pass |
| `endpoint_families.rs`: 3 tests | <pre><code>invalid ordinary relation to trait
invalid trait relation to ordinary target
ordinary relation from trait-bound source</code></pre> | `fixture_type_resolution_v2` | Invalid endpoint-family rows are excluded from public reachability query results. | Raw `type_relation` insertion; query-time endpoint family validation. | Pass |
| `fixed_rules.rs`: 2 tests | <pre><code>TypeTargetPaths fixed rule output</code></pre> | `fixture_type_resolution_v2` | Fixed-rule traversal matches recursive Datalog and preserves exact `type_use_id`. | `ploke.TypeTargetPaths`; recursive `type_target` query; root IDs. | Pass |
| `invariants.rs`: 2 tests | <pre><code>type_use coordinate tables</code></pre> | `fixture_type_resolution_v2` | Coordinate-bearing type uses have exactly one coordinate row, and coordinate rows never point to missing type uses. | `type_use_*` coordinate relations and `type_use_id` foreign-key integrity. | Pass |
| `matrix.rs`: 3 tests | <pre><code>positive_type_shape_cases()
no_target_type_shape_cases()
absent_type_shape_cases()</code></pre> | Five real-corpus typed backups | Exact roots, terminals, relation kind, depth, no-target retained structure, and fallback absence. | `OwnerSelector`; `TargetSelector`; `CoordinateSpec`; `TypeShapeCase`; `TypeShapeNoTargetCase`. | Pass |

## Real-Corpus DB Contracts

File: `crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs`

| Test name | Code item tested | Fixture and location | Property tested | IDs/selectors involved | State on 2026-05-18 |
| --- | --- | --- | --- | --- | --- |
| `semver_matches_req_reaches_both_public_model_types` | <pre><code>matches_req(req: &amp;VersionReq, ver: &amp;Version)</code></pre> | Source parse `dtolnay__semver` checkout | Function parameter reference traversal reaches `VersionReq` and `Version`. | `function_id_by_name_in_module`; `struct_id_by_name_in_module`; `struct_id_by_name`. | Ignored source-parse variant |
| `semver_backup_matches_req_reaches_both_public_model_types` | <pre><code>matches_req(req: &amp;VersionReq, ver: &amp;Version)</code></pre> | `tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite` | Same contract over backup fixture. | Same selectors. | Pass |
| `memchr_iter_return_reaches_iterator_struct` | <pre><code>memchr_iter(...) -&gt; Memchr&lt;'h&gt;</code></pre> | Source parse `BurntSushi__memchr` checkout | Function return reaches iterator struct. | `function_id_by_name_in_module`; `struct_id_by_name`. | Ignored source-parse variant |
| `memchr_backup_iter_return_reaches_iterator_struct` | <pre><code>memchr_iter(...) -&gt; Memchr&lt;'h&gt;</code></pre> | `tests/backup_dbs/corpus_memchr_type_graph_2026-05-17.sqlite` | Same contract over backup fixture. | Same selectors. | Pass |
| `semver_version_req_comparators_field_reaches_comparator` | <pre><code>VersionReq.comparators: Vec&lt;Comparator&gt;</code></pre> | Source parse `dtolnay__semver` checkout | Field root reaches nested generic argument `Comparator`. | `struct_id_by_name_in_module`; `field_id_by_owner_index`; `struct_id_by_name`. | Ignored source-parse variant |
| `semver_backup_version_req_comparators_field_reaches_comparator` | <pre><code>VersionReq.comparators: Vec&lt;Comparator&gt;</code></pre> | `tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite` | Same contract over backup fixture. | Same selectors. | Pass |
| `chrono_backup_weekday_set_from_array_param_reaches_weekday` | <pre><code>WeekdaySet::from_array(days: [Weekday; C])</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Array parameter element reaches `Weekday`. | `method_id_by_impl_self_type_name`; `enum_id_by_name`; `TypeRelationKind::Ordinary`. | Pass |
| `hyper_watch_channel_tuple_return_reaches_sender_and_receiver` | <pre><code>channel(...) -&gt; (Sender, Receiver)</code></pre> | Source parse `hyperium__hyper` checkout | Tuple return reaches `Sender` and `Receiver`. | `function_id_by_name_in_file_suffix`; `struct_id_by_name_in_file_suffix`. | Ignored source-parse variant |
| `semver_backup_version_req_matches_method_param_reaches_version` | <pre><code>VersionReq::matches(&amp;self, version: &amp;Version)</code></pre> | `tests/backup_dbs/corpus_semver_type_graph_2026-05-17.sqlite` | Method parameter reference reaches `Version`. | `method_id_by_impl_self_type_name`; `struct_id_by_name`. | Pass |
| `chrono_backup_parsed_to_datetime_return_reaches_datetime_and_offset` | <pre><code>Parsed::to_datetime() -&gt; ParseResult&lt;DateTime&lt;FixedOffset&gt;&gt;</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Method return reaches `DateTime` and nested `FixedOffset`. | `method_id_by_impl_self_type_name`; `struct_id_by_name`; depth `1` and `2`. | Pass |
| `generic_array_backup_generate_method_return_reaches_generic_array` | <pre><code>GenericSequence::generate(...) -&gt; GenericArray&lt;T, N&gt;</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Impl method return reaches `GenericArray`. | `method_id_by_impl_trait_and_self_type_names`; `struct_id_by_name`. | Pass |
| `generic_array_backup_generic_sequence_impl_reaches_self_and_trait` | <pre><code>impl&lt;T, N: ArrayLength&gt; GenericSequence&lt;T&gt; for GenericArray&lt;T, N&gt;</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Impl root reaches both self type and trait type. | `impl_id_by_trait_and_self_type_names`; target `GenericArray`; target `GenericSequence`; relation kinds `Ordinary`, `Trait`. | Pass |
| `generic_array_const_generic_alias_reaches_generic_array` | <pre><code>type ConstGenericArray&lt;T, const N&gt; = GenericArray&lt;T, ConstArrayLength&lt;N&gt;&gt;</code></pre> | Source parse `fizyk20__generic-array` checkout | Alias target reaches storage type. | `type_alias_row_by_name`; `struct_id_by_name`. | Ignored source-parse variant |
| `generic_array_backup_const_generic_alias_reaches_generic_array` | <pre><code>type ConstGenericArray&lt;T, const N&gt; = GenericArray&lt;T, ConstArrayLength&lt;N&gt;&gt;</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Same contract over backup fixture. | Same selectors. | Pass |
| `chrono_mapped_local_time_alias_reaches_local_result` | <pre><code>type MappedLocalTime&lt;T&gt; = LocalResult&lt;T&gt;</code></pre> | Source parse `chronotope__chrono` checkout | Alias reaches enum target. | `type_alias_row_by_name`; `enum_id_by_name`. | Ignored source-parse variant |
| `chrono_backup_mapped_local_time_alias_reaches_local_result` | <pre><code>type MappedLocalTime&lt;T&gt; = LocalResult&lt;T&gt;</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Same contract over backup fixture. | Same selectors. | Pass |
| `chrono_backup_min_naive_date_const_reaches_naive_date` | <pre><code>const MIN_DATE: NaiveDate</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Const type annotation reaches `NaiveDate`. | `const_id_by_name_in_file_suffix`; `struct_id_by_name`. | Pass |
| `chrono_backup_strftime_static_format_reaches_item` | <pre><code>static D_FMT: &amp;[Item&lt;'static&gt;]</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Static reference/slice element reaches `Item`. | `static_id_by_name_in_file_suffix`; `enum_id_by_name`; depth `2`. | Pass |
| `generic_array_backup_generic_array_bound_reaches_array_length_trait` | <pre><code>struct GenericArray&lt;T, N: ArrayLength&gt;</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Generic declaration bound reaches `ArrayLength`. | `struct_id_by_name`; `trait_id_by_name_in_module`. | Pass |
| `chrono_backup_date_timezone_bound_reaches_timezone_trait` | <pre><code>struct Date&lt;Tz: TimeZone&gt;</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Generic declaration bound reaches `TimeZone`. | `struct_id_by_name`; `trait_id_by_name_in_module`. | Pass |
| `chrono_backup_datetime_timezone_param_bound_source_reaches_timezone_trait` | <pre><code>struct DateTime&lt;Tz: TimeZone&gt;</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Generic-param-owned bound reaches `TimeZone`. | `generic_type_param_id_by_owner_name`; `trait_id_by_name_in_module`. | Pass |
| `chrono_backup_subsec_round_impl_where_bound_reaches_timelike_trait` | <pre><code>impl&lt;T&gt; SubsecRound for T where T: Timelike + ...</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Where predicate bound slot reaches `Timelike`. | `impl_id_by_trait_name_in_file_suffix`; `TypeUseRole::WherePredicateBound`; `WhereBoundSlot { predicate_index: 0, bound_index: 0 }`. | Pass |
| `generic_array_backup_concat_impl_where_bounds_reach_array_length_trait` | <pre><code>impl Concat for GenericArray where N: ArrayLength + Add&lt;M&gt;, M: ArrayLength, Sum&lt;N, M&gt;: ArrayLength</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Multi-predicate/multi-bound coordinates reach local `ArrayLength`. | `WhereBoundSlot` coordinates `(0,0)`, `(1,0)`, `(2,0)`. | Pass |
| `chrono_backup_format_with_items_where_bound_reaches_nested_item_enum` | <pre><code>B: Borrow&lt;Item&lt;'a&gt;&gt;</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Nested local terminal through external where bound is reachable. | `method_id_by_impl_self_type_name`; `WhereBoundSlot { predicate_index: 1, bound_index: 0 }`; target `Item`. | Pass |
| `generic_array_backup_zip_repeated_rhs_where_bounds_reach_both_local_traits` | <pre><code>FunctionalSequence::zip where Rhs: MappedGenericSequence&lt;...&gt;, Rhs: GenericSequence&lt;...&gt;</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Repeated same-subject where predicates reach distinct local traits. | `method_id_by_impl_trait_and_self_type_names`; slots `(1,0)`, `(2,0)`; targets `MappedGenericSequence`, `GenericSequence`. | Pass |
| `generic_array_backup_mapped_sequence_impl_composite_where_subject_reaches_generic_array` | <pre><code>where GenericArray&lt;U, N&gt;: GenericSequence&lt;U, Length = N&gt;</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Composite where subject root reaches `GenericArray`. | `impl_id_by_trait_name_in_file_suffix`; `TypeUseRole::WherePredicateSubject`; `WhereSubjectSlot { predicate_index: 0 }`. | Pass |
| `generic_array_backup_const_array_length_projection_reaches_into_array_length_trait` | <pre><code>type ConstArrayLength&lt;const N&gt; = &lt;Const&lt;N&gt; as IntoArrayLength&gt;::ArrayLength</code></pre> | `tests/backup_dbs/corpus_generic_array_type_graph_2026-05-17.sqlite` | Qualified projection trait qualifier reaches `IntoArrayLength`. | `type_alias_row_by_name`; `trait_id_by_name_in_module`; `TypeRelationKind::Trait`. | Pass |
| `chrono_backup_timezone_associated_offset_bound_reaches_offset_trait` | <pre><code>trait TimeZone { type Offset: Offset; }</code></pre> | `tests/backup_dbs/corpus_chrono_type_graph_2026-05-17.sqlite` | Associated type bound reaches target trait. | `trait_id_by_name_in_module`; `TypeRelationKind::Trait`. | Pass |

## Shared Real-Corpus Matrix

File: `crates/test-utils/src/type_shape_matrix.rs`

DB tests: `crates/ploke-db/tests/unit/type_graph_queries/matrix.rs`

RAG tests: `crates/ploke-rag/src/core/unit_tests.rs`

TUI tests: `crates/ploke-tui/src/tools/request_code_context.rs`

| Case | Code item tested | Fixture | Property tested | IDs/selectors involved | Coverage and current state |
| --- | --- | --- | --- | --- | --- |
| `named_return_memchr_iter` | <pre><code>memchr_iter(...) -&gt; Memchr&lt;'h&gt;</code></pre> | `corpus_memchr_*_2026-05-17.sqlite` | Named return terminal reaches `Memchr`. | Owner `FunctionInModule(["crate","memchr"], "memchr_iter")`; role `FunctionReturn`; terminal `StructByName("Memchr")`; relation `Ordinary`; depth `0`. | DB pass; RAG/TUI matrix coverage intended; TUI direct quarantined |
| `reference_param_semver_matches_req` | <pre><code>matches_req(req: &amp;VersionReq, ...)</code></pre> | `corpus_semver_*_2026-05-17.sqlite` | Reference parameter reaches referenced `VersionReq`. | Owner `FunctionInModule(["crate","eval"], "matches_req")`; role `FunctionParam`; coordinate `ParamSlot(0)`; terminal `StructInModule(["crate"], "VersionReq")`; relation `Ordinary`; depth `1`. | DB pass; RAG pass; TUI direct quarantined |
| `named_generic_argument_chrono_weekday_set_single_day` | <pre><code>WeekdaySet::single_day(...) -&gt; Option&lt;Weekday&gt;</code></pre> | `corpus_chrono_*_2026-05-17.sqlite` | Named generic argument reaches `Weekday`. | Owner `MethodByImplSelf("WeekdaySet", "single_day")`; role `MethodReturn`; terminal `EnumByName("Weekday")`; relation `Ordinary`; depth `1`. | DB pass; RAG owner-seeded pass; BM25 smoke pass; TUI direct quarantined |
| `qualified_projection_generic_array_const_array_length` | <pre><code>&lt;Const&lt;N&gt; as IntoArrayLength&gt;::ArrayLength</code></pre> | `corpus_generic_array_*_2026-05-17.sqlite` | Qualified projection reaches trait qualifier `IntoArrayLength`. | Owner `TypeAlias("ConstArrayLength")`; role `TypeAliasTarget`; terminal `TraitInModule(["crate"], "IntoArrayLength")`; relation `Trait`; depth `1`. | DB pass; RAG API pass |
| `function_pointer_memchr_searcher_kind` | <pre><code>type SearcherKindFn = unsafe fn(&amp;Searcher, &amp;mut PrefilterState, ...) -&gt; Option&lt;usize&gt;</code></pre> | `corpus_memchr_*_2026-05-17.sqlite` | Function pointer parameter containment reaches `Searcher`. | Owner `TypeAlias("SearcherKindFn")`; role `TypeAliasTarget`; terminal `StructInModule(["crate","memmem","searcher"], "Searcher")`; relation `Ordinary`; depth `2`. | DB pass; RAG API pass |
| `slice_static_chrono_d_fmt` | <pre><code>static D_FMT: &amp;[Item&lt;'static&gt;]</code></pre> | `corpus_chrono_*_2026-05-17.sqlite` | Reference and slice containment reaches `Item`. | Owner `StaticInFile("src/format/strftime.rs", "D_FMT")`; role `StaticType`; terminal `EnumByName("Item")`; relation `Ordinary`; depth `2`. | DB pass; RAG API pass |
| `array_param_chrono_weekday_set` | <pre><code>WeekdaySet::from_array(days: [Weekday; C])</code></pre> | `corpus_chrono_*_2026-05-17.sqlite` | Array element reaches `Weekday`. | Owner `MethodByImplSelf("WeekdaySet", "from_array")`; role `MethodParam`; coordinate `ParamSlot(0)`; terminal `EnumByName("Weekday")`; depth `1`. | DB pass; RAG API pass |
| `tuple_return_chrono_weekday_set_split_at` | <pre><code>WeekdaySet::split_at(...) -&gt; (Self, Self)</code></pre> | `corpus_chrono_*_2026-05-17.sqlite` | Tuple element `Self` reaches `WeekdaySet`. | Owner `MethodByImplSelf("WeekdaySet", "split_at")`; role `MethodReturn`; terminal `StructByName("WeekdaySet")`; depth `1`. | DB pass; RAG API pass |
| `raw_pointer_memchr_pointer_distance` | <pre><code>impl&lt;T&gt; Pointer for *const T { distance(self, origin: *const T) }</code></pre> | `corpus_memchr_type_graph_2026-05-17.sqlite` | Raw pointer parameter reaches generic param `T`. | Owner `MethodByRawPointerImpl("src/ext.rs", "Pointer", false, "distance")`; role `MethodParam`; coordinate `ParamSlot(1)`; terminal `GenericParamReachableByName("T")`; depth `1`. | DB-only pass |
| `trait_object_axum_boxed_into_route` | <pre><code>BoxedIntoRoute&lt;S, E&gt;(Box&lt;dyn ErasedIntoRoute&lt;S, E&gt;&gt;)</code></pre> | `corpus_axum_*_2026-05-17.sqlite` | Trait object nested bound reaches `ErasedIntoRoute`. | Owner `FieldByStructInFile("axum/src/boxed.rs", "BoxedIntoRoute", 0)`; role `FieldType`; coordinate `FieldSlot(0)`; terminal `TraitInFile("axum/src/boxed.rs", "ErasedIntoRoute")`; relation `Trait`; depth `2`. | DB pass; RAG seeded/sparse pass; TUI direct quarantined |
| `trait_object_axum_map_layer_fn_field` | <pre><code>Map.layer: Box&lt;dyn LayerFn&lt;E, E2&gt;&gt;</code></pre> | `corpus_axum_*_2026-05-17.sqlite` | Trait object field reaches `LayerFn`. | Owner `FieldByStructInFile("axum/src/boxed.rs", "Map", 1)`; role `FieldType`; coordinate `FieldSlot(1)`; terminal `TraitInFile("axum/src/boxed.rs", "LayerFn")`; relation `Trait`; depth `2`. | DB pass; RAG owner-seeded pass; BM25 smoke pass; TUI direct quarantined |
| `trait_object_axum_make_erased_handler_clone_box` | <pre><code>MakeErasedHandler::clone_box(...) -&gt; Box&lt;dyn ErasedIntoRoute&lt;S, Infallible&gt;&gt;</code></pre> | `corpus_axum_*_2026-05-17.sqlite` | Trait-object method return reaches `ErasedIntoRoute`. | Owner `MethodByImplTraitAndSelf("ErasedIntoRoute", "MakeErasedHandler", "clone_box")`; role `MethodReturn`; terminal `TraitInFile("axum/src/boxed.rs", "ErasedIntoRoute")`; depth `2`. | DB pass; RAG/TUI matrix intended; TUI direct quarantined |
| `impl_trait_axum_strip_prefix_zip_longest_item` | <pre><code>zip_longest(...) -&gt; impl Iterator&lt;Item = Item&lt;I::Item&gt;&gt;</code></pre> | `corpus_axum_*_2026-05-17.sqlite` | Impl trait associated item reaches `Item`. | Owner `FunctionInFile("axum/src/routing/strip_prefix.rs", "zip_longest")`; role `FunctionReturn`; terminal `EnumByName("Item")`; depth `2`. | DB pass; RAG/TUI matrix intended; TUI direct quarantined |
| `impl_trait_axum_strip_prefix_layer` | <pre><code>StripPrefix::layer(...) -&gt; impl Layer&lt;S, Service = Self&gt; + Clone</code></pre> | `corpus_axum_*_2026-05-17.sqlite` | Impl trait associated type/self path reaches `StripPrefix`. | Owner `MethodByImplSelf("StripPrefix", "layer")`; role `MethodReturn`; terminal `StructByName("StripPrefix")`; depth `2`. | DB pass; RAG/TUI matrix intended; TUI direct quarantined |
| `impl_trait_generic_array_array_builder_extend_source` | <pre><code>ArrayBuilder::extend(..., source: impl Iterator&lt;Item = T&gt;)</code></pre> | `corpus_generic_array_type_graph_2026-05-17.sqlite` | Impl trait parameter reaches generic param `T`. | Owner `MethodByImplSelf("ArrayBuilder", "extend")`; role `MethodParam`; coordinate `ParamSlot(1)`; terminal `GenericParamReachableByName("T")`; depth `2`. | DB-only pass |
| `trait_bound_generic_array_array_length` | <pre><code>struct GenericArray&lt;T, N: ArrayLength&gt;</code></pre> | `corpus_generic_array_*_2026-05-17.sqlite` | Generic declaration bound reaches `ArrayLength`. | Owner `StructByName("GenericArray")`; role `GenericBound`; coordinate `GenericBoundSlot { generic_param_index: 1, bound_index: 0 }`; relation `Trait`; depth `0`. | DB pass; RAG API pass |
| `associated_type_bound_chrono_timezone_offset` | <pre><code>trait TimeZone { type Offset: Offset; }</code></pre> | `corpus_chrono_*_2026-05-17.sqlite` | Associated type bound reaches `Offset`. | Owner `TraitInModule(["crate","offset"], "TimeZone")`; role `AssociatedTypeBound`; coordinate `AssociatedTypeBoundSlot { associated_type_index: 0, associated_type_name: "Offset", bound_index: 0 }`; relation `Trait`; depth `0`. | DB pass; RAG API pass |
| `trait_super_generic_array_concat` | <pre><code>unsafe trait Concat&lt;T, M: ArrayLength&gt;: GenericSequence&lt;T&gt;</code></pre> | `corpus_generic_array_*_2026-05-17.sqlite` | Supertrait root reaches `GenericSequence`. | Owner `TraitInModule(["crate","sequence"], "Concat")`; role `TraitSuper`; coordinate `TraitSuperSlot(0)`; relation `Trait`; depth `0`. | DB pass; RAG API pass |
| `generic_param_bound_chrono_datetime_tz` | <pre><code>struct DateTime&lt;Tz: TimeZone&gt;</code></pre> | `corpus_chrono_type_graph_2026-05-17.sqlite` | Generic-param-owned bound reaches `TimeZone`. | Owner `GenericTypeParamByContainingOwner(StructByName("DateTime"), "Tz")`; role `GenericParamBound`; coordinate includes containing owner and bound slot; relation `Trait`. | DB-only pass |
| `where_bound_chrono_subsec_round` | <pre><code>impl&lt;T&gt; SubsecRound for T where T: Timelike + ...</code></pre> | `corpus_chrono_*_2026-05-17.sqlite` | Where bound reaches `Timelike`. | Owner `ImplByTraitInFile("src/round.rs", "SubsecRound")`; role `WherePredicateBound`; coordinate `WhereBoundSlot { predicate_index: 0, bound_index: 0 }`; relation `Trait`. | DB pass; RAG API pass |
| `where_subject_generic_array_mapped_sequence` | <pre><code>where GenericArray&lt;U, N&gt;: GenericSequence&lt;...&gt;</code></pre> | `corpus_generic_array_*_2026-05-17.sqlite` | Where subject root reaches `GenericArray`. | Owner `ImplByTraitInFile("src/lib.rs", "MappedGenericSequence")`; role `WherePredicateSubject`; coordinate `WhereSubjectSlot { predicate_index: 0 }`; relation `Ordinary`. | DB pass; RAG API pass |
| `where_generic_param_bound_generic_array_zip_rhs` | <pre><code>FunctionalSequence::zip where Rhs: GenericSequence&lt;B, ...&gt;</code></pre> | `corpus_generic_array_type_graph_2026-05-17.sqlite` | Generic-param-owned where bound reaches `GenericSequence`. | Owner `WhereGenericParamBoundOwner(MethodByImplTraitAndSelf("FunctionalSequence", "GenericArray", "zip"), predicate_index: 2, bound_index: 0)`; role `WhereGenericParamBound`; relation `Trait`. | DB-only pass |
| `never_return_generic_array_from_iter_length_fail` | <pre><code>from_iter_length_fail(...) -&gt; !</code></pre> | `corpus_generic_array_type_graph_2026-05-17.sqlite` | Root `never_type` is retained without a fabricated target relation. | Owner `FunctionInFile("src/lib.rs", "from_iter_length_fail")`; role `FunctionReturn`; root relation `never_type`. | DB no-target pass |
| `macro_argument_axum_punctuated_token` | <pre><code>variants: Punctuated&lt;syn::Variant, Token![,]&gt;</code></pre> | `corpus_axum_type_graph_2026-05-17.sqlite` | Nested macro type is retained without a bogus terminal target. | Owner `FunctionInFile("axum-macros/src/from_request/mod.rs", "impl_enum_by_extracting_all_at_once")`; role `FunctionParam`; coordinate `ParamSlot(1)`; nested relation `macro_type` through `Argument`. | DB no-target pass |
| `paren_nested_axum_core_error_source` | <pre><code>Error::source() -&gt; Option&lt;&amp;(dyn StdError + 'static)&gt;</code></pre> | `corpus_axum_type_graph_2026-05-17.sqlite` | Nested paren type is retained without a bogus terminal target. | Owner `MethodByImplSelf("Error", "source")`; role `MethodReturn`; nested relation `paren_type` through `Argument -> Referenced`. | DB no-target pass |
| `inferred_type` absent case | <pre><code>inferred_type</code></pre> | All registered corpus backups | Current corpus backups do not contain fallback inferred rows. | `absent_type_shape_cases()` relation check. | DB absent pass |
| `unknown_type` absent case | <pre><code>unknown_type</code></pre> | All registered corpus backups | Current corpus backups do not contain fallback unknown rows. | `absent_type_shape_cases()` relation check. | DB absent pass |

## RAG and TUI Type-Context Coverage

| Test name | Code item or search term tested | Fixture | Property tested | IDs/selectors involved | Current state |
| --- | --- | --- | --- | --- | --- |
| `type_context_expansion_adds_materializable_type_neighbors` | <pre><code>method "new" seed -&gt; impl self target</code></pre> | `tests/backup_dbs/fixture_nodes_local_embeddings_2026-05-17.sqlite` | Expansion preserves seed and adds materializable owning impl-self target with `TypeDefinitionImpact`. | Seed UUID from `unique_id_by_name("method", "new")`; target UUID from `impl_self_target_for_method_name("new")`. | Pass |
| `corpus_type_shape_matrix_expands_db_and_rag_type_context` | <pre><code>positive_type_shape_cases() where coverage includes RagApi</code></pre> | Per-case OpenRouter searchable corpus backup | DB owner-target path exists and RAG expansion materializes matching `TypeContextInfo`. | `resolve_matrix_owner`; `resolve_matrix_target`; `TypeContextSeed::Owner`; expected `TypeContextKind`. | Pass: `1 passed; 0 failed; 0 ignored` in current focused run |
| `axum_struct_seed_materializes_nested_trait_object_target` | <pre><code>BoxedIntoRoute -&gt; ErasedIntoRoute</code></pre> | `tests/backup_dbs/corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | Struct seed materializes nested trait object target with `UsesTypeNested`. | Seed `BoxedIntoRoute` by file suffix; target `ErasedIntoRoute` by file suffix. | Pass |
| `axum_boxed_into_route_sparse_context_emits_nested_trait_type_context` | <pre><code>search_bm25_strict("BoxedIntoRoute struct", 1)</code></pre> | `corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | Sparse retrieval plus expansion emits nested trait type context. | Expected seed `BoxedIntoRoute`; target `ErasedIntoRoute`; relation `UsesTypeNested`. | Pass |
| `chrono_single_day_owner_seeded_expands_weekday_type_context` | <pre><code>WeekdaySet::single_day seed -&gt; Weekday</code></pre> | `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite` | Owner-seeded expansion reaches `Weekday` with `UsesTypeNested`. | Seed `method_by_impl_self_query("WeekdaySet", "single_day")`; target `EnumByName("Weekday")`. | Pass |
| `chrono_single_day_bm25_precise_query_retrieves_method_owner` | <pre><code>search_bm25_strict("single_day", 15)</code></pre> | `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite` | BM25 retrieval smoke: exact method owner appears in top 15. | Method UUID for `WeekdaySet::single_day`. | Pass |
| `axum_map_layer_field_seeded_expands_layer_fn_type_context` | <pre><code>Map.layer field seed -&gt; LayerFn</code></pre> | `corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | Field-seeded expansion reaches `LayerFn` with `UsesTypeNested`. | Field owner `Map`, index `1`; target trait `LayerFn`. | Pass |
| `axum_map_bm25_precise_query_retrieves_map_struct` | <pre><code>search_bm25_strict("Map", 25)</code></pre> | `corpus_axum_openrouter_embeddings_2026-05-17.sqlite` | BM25 retrieval smoke: exact `Map` struct appears in top 25. | `StructInFile("axum/src/boxed.rs", "Map")`. | Pass |
| `request_code_context_tool_emits_matrix_type_context` | <pre><code>positive_type_shape_cases() where coverage includes TuiTool
search_term = case.search_term</code></pre> | Per-case OpenRouter searchable corpus backup | Intended to assert direct `request_code_context` payload contains matching `ConciseContext.type_context`. | `resolve_matrix_owner`; `resolve_matrix_target`; expected seed IDs; relation mapped from `TypeContextRelation`. | Ignored/quarantined; known unsuitable as proof because BM25 seed selection is not controlled |
| `live_request_code_context_matrix_uses_production_tool_payload` | <pre><code>positive_type_shape_cases() where coverage includes LiveIgnored
live prompt = case.live_prompt</code></pre> | Per-case OpenRouter searchable corpus backup | Intended to assert live model/tool event payload contains matching type context. | Tool events, requested seed IDs, expected target selector. | Ignored/manual provider path; not trusted until event call-id correlation and seed identity are tightened |

## Gaps

| Gap | Current evidence | Why it matters |
| --- | --- | --- |
| TUI direct matrix is quarantined | DB/RAG owner-seeded paths pass, but `request_code_context_tool_emits_matrix_type_context` starts from BM25 terms and is ignored. | It cannot prove type-context expansion from a specific owner unless the search seed is controlled or the entrypoint starts from resolved UUIDs. |
| Live TUI matrix is not authoritative | The live test is ignored/manual and provider-spending. Prior review found it can accept the wrong tool completion unless tied to a specific `call_id`. | Live proof needs event correlation and strict payload identity, not final model wording or any parseable completion. |
| Source-parse corpus variants are ignored | `corpus_contracts.rs` has six ignored source-parse variants; backup variants pass. | Backups prove persisted snapshots; ignored source-parse variants would prove full fresh parse/transform over the external corpus checkout. |
| External/non-node targets are not first-class | Tests assert local terminals nested inside external containers, but external traits like `Borrow`, `Add`, `Sub`, primitives, lifetimes, and const values are not modeled as resolved targets. | Graph queries cannot yet answer questions whose semantic target is outside the local crate graph or is a non-item Rust type family. |
| Precise associated item ownership is still incomplete | Associated type bounds are currently exposed through the containing trait. | This is useful for graphRAG, but it is not precise enough for associated type defaults, impl associated definitions, or associated consts. |
| `type_use` coordinate model is still role-specific | Tests pin current coordinates, but `slot_index` semantics vary by role and the durable coordinate model still needs design. | UI/provenance consumers need stable coordinates that are not ambiguous across params, fields, bounds, predicates, and generic-param-owned projections. |
| Import/re-export stress remains thin | Fresh fixtures include some imports and conflation cases, but real-corpus matrix does not yet cover renamed imports, glob imports, multi-hop re-exports, and file-module boundary stress comprehensively. | Type resolution failures often appear at import-chain depth and module boundary edges. |
| Target family coverage is incomplete | Real-corpus contracts hit many structs/enums/traits/generic params, but not every ordinary family such as unions and type aliases as terminal targets. | Endpoint-family validation is only as strong as the target families represented in contracts. |
| Search behavior is intentionally separated from type-context expansion | RAG now has separate owner-seeded type-context tests and BM25 smoke tests. | This is correct, but it means search-driven user paths still need separate evaluation across sparse, dense, and hybrid retrieval. |

## Weaknesses In The Current Approach

| Weakness | Consequence | Safer direction |
| --- | --- | --- |
| Matrix rows are code constants, not generated from a manifest | Coverage is discoverable in Rust but not easily audited outside code. | Keep this document in sync and consider generating a markdown report from `TypeShapeCase` rows. |
| Macro-generated parser tests compress many cases into one source file | Test names exist at runtime, but doc readers must inspect macro tables to see every row. | Keep table rows grouped by source shape and add new exact-source tests for high-risk shapes. |
| Backup fixtures are schema-coupled snapshots | Passing backup tests can lag fresh parser behavior if snapshots are stale; fresh parse variants are ignored for cost. | Regenerate or verify fixtures through the registry when schema changes; run source-parse corpus tests intentionally before major claims. |
| RAG proof depends on materialization and context assembly policy | A DB path can pass while assembled context omits a target due to search seed, budget, or materialization. | Test DB expansion, RAG owner-seeded expansion, and search retrieval as separate contracts. |
| TUI payload assertions still use target labels/snippets in places | Label collisions like `T`, `Item`, and `Map` can produce false confidence. | Resolve expected target UUIDs and assert payload identity by UUID or a strict selector-derived identity. |
| Query-time endpoint validation does not make bad persisted rows impossible | Invalid rows can still exist in a backup or manual insert; current public APIs filter them. | Decide whether insert-time/schema-level rejection is needed in addition to query-time filtering. |
| No exhaustive recursive grammar coverage | Tests cover representative deepest examples, not every Rust type grammar combination. | Add targeted rows when a new structural shape becomes important; avoid claiming grammar exhaustion. |
