 ## Use Statement Semantics

 ### Path Resolution
 1. **Absolute Paths**
```rust
use std::collections::HashMap;
```
- Stored as exact path segments: `["std", "collections", "HashMap"] 

 2. **Relative Paths**
```rust
use self::module::Item;
use super::parent::Type;
```
- `self`/`super` preserved verbatim for later resolution

 ### Special Cases
 | Case          | Example                                  | Representation        |
 |---------------|------------------------------------------|-----------------------|
 | Aliases       | `use std::fmt as Fmt`                    | `visible_name: "Fmt"`, `original_name: Some("fmt")` |
 | Globs         | `use std::prelude::*`                    | `is_glob: true`, `visible_name: "*"` |
 | Nested Groups | `use std::sync::{Arc, atomic::AtomicBool}` | Stored  separate statements | 
