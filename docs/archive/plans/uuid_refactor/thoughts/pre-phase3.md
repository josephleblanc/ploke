Based on the Phase 3 requirements and the current node structures, here are some strategic suggestions for preparing your node types:

### **1. Core Methods for Node Types**
For primary nodes (`EnumNode`, `StructNode`, etc.), consider adding:

#### **Path Resolution Helpers**
```rust
// In impl EnumNode/StructNode/etc.
pub fn absolute_path(&self, graph: &CodeGraph) -> Vec<String> {
    graph.get_item_module_path(self.id)
        .into_iter()
        .chain(iter::once(self.name.clone()))
        .collect()
}
```
*Why*: Critical for Phase 3 resolution. Combines module path + node name to build absolute paths like `crate::module::MyEnum`.

#### **ID Context Getters**
```rust
pub fn synthetic_id_context(&self) -> (String, Vec<String>, Vec<u8>) {
    (
        self.name.clone(),
        self.absolute_path(), // Requires module tree
        self.cfgs_hash(),     // See below
    )
}
```
*Why*: Encapsulates the exact context used to generate `NodeId::Synthetic` for resolution.

#### **CFG Hash Helper**
```rust
pub fn cfgs_hash(&self) -> Vec<u8> {
    calculate_cfg_hash_bytes(&self.cfgs).unwrap_or_default()
}
```
*Why*: Centralizes CFG hash logic for consistent ID resolution.

---

### **2. Traits for Phase 3 Logic**
Define traits to standardize node handling:

#### **ResolvableNode Trait**
```rust
pub trait ResolvableNode {
    fn pre_resolve(&self, graph: &CodeGraph) -> ResolveContext;
    fn post_resolve(&mut self, resolved_id: NodeId);
}

// Example impl for EnumNode:
impl ResolvableNode for EnumNode {
    fn pre_resolve(&self, graph: &CodeGraph) -> ResolveContext {
        ResolveContext {
            name: self.name.clone(),
            module_path: graph.get_item_module_path(self.id),
            cfgs: self.cfgs.clone(),
            kind: ItemKind::Enum,
        }
    }
    fn post_resolve(&mut self, resolved_id: NodeId) {
        self.id = resolved_id;
    }
}
```
*Why*: Unifies resolution steps across node types. `ResolveContext` can store:
- Name, module path, CFGs, and item kind (for ID regeneration)
- Any `use` statement dependencies (for type resolution)

#### **UseStatementResolver Trait**
```rust
pub trait UseStatementResolver {
    fn resolve_imports(&self, graph: &CodeGraph) -> Vec<ResolvedImport>;
}

// Example for ModuleNode:
impl UseStatementResolver for ModuleNode {
    fn resolve_imports(&self, graph: &CodeGraph) -> Vec<ResolvedImport> {
        self.imports.iter().map(|imp| {
            ResolvedImport {
                path: imp.path.clone(),
                resolved_target: graph.resolve_path(&imp.path), // New method needed in CodeGraph
            }
        }).collect()
    }
}
```
*Why*: Isolves `use` statements to their actual targets during Phase 3.

---

### **3. Module Tree Utilities**
For `ModuleNode`, add:

#### **Nested Module Discovery**
```rust
impl ModuleNode {
    pub fn nested_modules<'a>(&self, graph: &'a CodeGraph) -> impl Iterator<Item = &'a ModuleNode> + 'a {
        graph.modules.iter().filter(|m| {
            m.path.starts_with(&self.path) && m.path.len() == self.path.len() + 1
        })
    }
}
```
*Why*: Helps build the module hierarchy during resolution.

#### **Item Collection**
```rust
pub fn items_in_scope(&self, graph: &CodeGraph) -> Vec<NodeId> {
    graph.relations.iter()
        .filter(|r| r.source == GraphId::Node(self.id) && r.kind == RelationKind::Contains)
        .map(|r| match r.target {
            GraphId::Node(id) => id,
            _ => panic!("Module contains non-node"),
        })
        .collect()
}
```
*Why*: Aggregates all items directly in this module for scope resolution.

---

### **4. Type Resolution Extensions**
For `TypeAliasNode` and similar:

#### **Type Resolution Helper**
```rust
impl TypeAliasNode {
    pub fn resolve_target_type(&self, graph: &CodeGraph) -> Option<TypeId> {
        // Handle aliases like `type Foo = Bar<Baz>`
        graph.resolve_type(self.aliased_type)
    }
}
```
*Why*: Recursive type resolution will be needed for full `TypeId` resolution.

---

### **Recommendation**
1. **Start with Traits**: Implement `ResolvableNode` and `UseStatementResolver` first—they’ll force you to design the resolution pipeline cleanly.
2. **Keep Nodes Lean**: Add only methods that encapsulate *node-specific* logic (e.g., path building). Phase 3’s heavy lifting should live in a dedicated `resolution.rs` module.
3. **Focus on ModuleTree Early**: The module tree is foundational for both path resolution and `use` statement handling. Extend `ModuleNode` utilities first.

### **Phase 3 Readiness Checklist**
- [ ] `ResolvableNode` trait for ID regeneration
- [ ] `UseStatementResolver` trait for import handling  
- [ ] Module tree utilities (`nested_modules`, `items_in_scope`)
- [ ] CFG hashing consistency across nodes
- [ ] Type resolution helpers for aliases/generics

This approach keeps nodes focused while providing the hooks Phase 3 needs. Would you like me to draft any of these traits or methods in more detail?
