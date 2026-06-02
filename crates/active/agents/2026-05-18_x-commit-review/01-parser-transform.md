No correctness regressions found in the scoped parser/transform/type-relation path.

Verification: `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture` passed.

Test gap: the parser-side suite still does not directly assert the exact-source shape for direct generic-param-owned where-bound roots, and associated-type bounds are only covered at the single-bound level in the parser tests.
