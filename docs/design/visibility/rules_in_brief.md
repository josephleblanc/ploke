Let me carefully analyze `fixture_test_crate/src/main.rs` to verify and expand on the visibility rules. Here's my detailed breakdown:

### Verified Visibility Rules from the Fixture:

1. **Module Privacy Chains**:
   - A module's privacy affects all its contents, even `pub` items
   - Example: `public_func_in_private_mod` is `pub` but in private module `d`
     - Accessible within `d` and its descendants
     - Accessible elsewhere only via `use crate::a::d::public_func_in_private_mod`

2. **Direct Visibility**:
   - Items are directly visible in:
     - Their own module
     - Child modules (inherited downward)
   - Not visible in parent/sibling modules without `use`

3. **Use Statement Validity**:
   - `use` path must be fully visible:
     - All modules in path must be accessible
     - Target item must be `pub` relative to path
   - Examples:
     - `use crate::a::b::public_func` - valid (all public)
     - `use crate::a::c::private_func` - invalid (`c` is private)
     - `use crate::a::d::public_func_in_private_mod` - valid (item is public despite `d` being private)

4. **Relative Paths**:
   - The fixture uses absolute paths (`crate::`), but relative paths would follow same rules
   - `super::` and `self::` would need similar visibility checks

5. **Edge Cases Confirmed**:
   - `test_pub_in_priv` shows you can't `use` a private module (`a::d`) even to access its public items
   - But you can `use` the item's full path if the item itself is public

### Expanded Visibility Factors:

1. **Path Segment Visibility**:
   - Each segment in `crate::a::b::public_func` must be visible:
     - `crate` - always visible
     - `a` - visible (root module)
     - `b` - visible (pub module)
     - `public_func` - visible (pub item)

2. **Re-export Visibility**:
   - If `b` did `pub use super::d::public_func_in_private_mod`, then:
     - Could use via `use crate::a::b::public_func_in_private_mod`
     - Original path (`a::d::`) would still be invalid outside

3. **pub(in path) Handling**:
   - For `pub(in some::path)` items:
     - Only visible within specified ancestor path
     - Need to check if current context is descendant of that path

4. **Crate Boundaries**:
   - `pub(crate)` items:
     - Visible anywhere in same crate
     - Invisible to other crates
   - Need to track crate boundaries in your graph

### Implementation Recommendations:

1. **Visibility Check Algorithm**:
   ```rust
   fn is_visible(item: &Item, context: &ModuleContext) -> bool {
       // 1. Check item's own visibility modifier
       // 2. Check each path segment from context to item is visible
       // 3. For pub(in path), verify context is within allowed scope
       // 4. For pub(crate), verify same crate
   }
   ```

2. **Import Path Generation**:
   ```rust
   fn get_import_paths(item: &Item, context: &ModuleContext) -> Vec<Vec<String>> {
       // 1. Find all possible paths to item (including through re-exports)
       // 2. Filter paths where all segments are visible from context
       // 3. Return remaining paths as valid import options
   }
   ```

3. **Context Representation**:
   ```rust
   struct ModuleContext {
       absolute_path: Vec<String>, // ["crate", "a", "b"]
       crate_id: Option<CrateId>,  // For cross-crate visibility
       // Other context like current use statements
   }
   ```

Would you like me to propose specific changes to the `CodeGraph` or `VisibilityResult` types to support this functionality? The existing `resolve_visibility` method seems like a good starting point but might need expansion.
