Evaluate caller-supplied module-boundary policy rules from an exact Rust code item.

Use this when you need to answer architecture-review questions such as "does this
entrypoint cross from one module layer into a forbidden module layer?" Resolve the
owner with the same exact endpoint fields used by `code_item_call_path` and
`code_item_effect_guard`. Each rule supplies a `rule_id`, a
`caller_module_prefix`, and a `callee_module_prefix`, where prefixes are arrays
of module path segments such as `["crate", "ext_traits"]`.

The tool only evaluates existing resolved call graph module-boundary edges. It
does not infer dependency policy, does not inspect targetless frontier rows, and
does not create call edges for unsupported or external callsites.
