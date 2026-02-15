# Deciding path forward / defining problem

## My description 
hmmm... Let's see. So the big picture is like this: We want to be able to more or less "scope" any given area of the user's git repo, so when we are querying for the code snippets later, we can easily tell whether a given piece of code is valid.

However, because we are also providing the option to include user dependencies in the database, even code that might be marked as `pub(crate)` could be out of scope for the user. For example, 

### Problem Space
Code snippets and/or span locations in the database will fall into one of these broad categories:
1. In scope: The code snippet uses items that can immediately be included in the target location.
  - User-code Example: `fn somefunc() {}` in the same file and not across `mod` boundaries.
  - Dependency Example: `pub SomeStruct {}`, in a file where `use some_dep::intermediate_mod::*` makes it available already.
2. In scope with import: The code is valid for use in the target location, but only after adding a `use` statement.
  - User-code Example: `pub(crate) enum ExampleEnum {}` located anywhere in the user's code. 
    - Should **not** be falsely flagged for import when no import is required (case 1), as that would result in errors. (This happens aaaallll the time and its super annoying).
  - Dependency Example: `pub trait ApiTrait` with an `impl ApiStruct { fn something() {} }`. Note that the trait is `pub` while the `fn` is not (our tests currently capture this correctly I think).
3. Cannot be brought into scope: This is code that is not accessible to the user, and any attempt to use it will result in an error.
  - User-code Example: I don't think there is actually any code like this. In any case it doesn't happen often.
    - Maybe stuff behind cfg flags in the wrong circumstances? We won't handle macro stuff for now, so if that is the only example we are punting this one as too edge for the MVP.
    - Oh shoot I may have run into this lately. Can you even import functions from the `tests` directory inside a project? This might be something to watch out for.
  - Dependency Example: All code not declared `pub` or inheriting `pub`, which means `pub(crate)` is accurately detected as out of scope.
    - It seems intuitive to think that this code should not be included at all, however there has been research showing including code that cannot be used in scope can be helpful in helping the LLM understand what the code can and cannot do.
    - Should be included in the database, distinguishable with a query, and will likely be optionally included with specific headers telling the LLM about the scoping issues. We'll sort that out later when we start implementing the LLM interface and context builder, but for now we just want to make sure the data on out of scope code is available.

### Implementation
Now, to actually handle all of these cases, we are going to need a few things:
1. Correctly resolved visibility (what we are discussing)
  - visibility modifiers `pub` and other modifiers (implemnted)
  - `use` statement resolution (implemented)
2. Correctly resolved file and folder structure (not implemented)
  - connected to (1.), e.g. `pub(super)` should only work as applicable. (not implemented)
  - Could get complicated with workspaces. (not implemented)
3. A marker for user code vs. dependencies. (not implemented)
  - Can simply tell a dependency (anything imported) from user code (anything not imported), not considering workspaces (not implemented)
  - Could get complex if we consider "user code" to include edge cases, e.g. dependencies that are being imported from a nearby file. However, that will be a someday/maybe feature not for MVP. (not implemented, not planned)
  - Could get complicated with workspaces. (not implemented, possibly not for MVP)

Now, we have correctly detected visibility in our recent phase1 and phase2 refactors. We want to make sure our visibility resolution correctly resolves visibility, but we also need to be aware of the other features we are going to have to develop, and make sure we design the visibility resolution in a way that will play nice with the other components we have not yet implemented.

Does all that make sense? I don't think our currently proposed plans really take all of that into account, and now that we are at a stage of our refactor where our decisions will affect future development we need to start taking the wider picture into account.

First, I'd like you to just check through my logic above and determine whether this seems like a reasonable description of the problem. Then I'd like to get your take on whether or not our current implementation framework of the visibility is adequate for working well with other features we would like to implement, but which we have not worked out any specifics on. Finally, I'd like you to recommend whether we should finish visibility resolution now or stop working on phase3 and switch to implementing file tracking.

File tracking is interesting, because we have access to the `Span` type through the `proc_macro2` library, and it is already integrated with `syn`, so it should be a fairly straightforward implementation for a single-threaded program. It might get a little more hairy if we want to ensure it works across threads, but our concurrency approach is still so nascent that this worry could be premature.

Dependency tracking also should be fairly trivial to implement in the simple case, and even versioning shouldn't be too hard.

So what do you think? I've laid out how I see the case, but I don't want you to just agree with me for the sake of it or some shit, just tell me what you think.

## LLM Review
You've done an excellent job outlining the complex visibility/scoping requirements. Let me analyze the situation systematically:

### Validation of Your Problem Analysis
Your breakdown is logically sound and covers the critical cases. A few nuanced observations:
1. **Edge Cases You Mentioned**:
   - `tests/` directory visibility is Rust-limited - items there are unreachable from `src/` without `pub` + path hacks
   - Workspace visibility is indeed thorny (`pub(in path)` across crates)

2. **Dependency Nuance**:
   - The "user code vs dependency" distinction may need refinement later for:
     - Dev-dependencies
     - Optional features
   - But agreed these can be post-MVP

### Visibility Resolution Evaluation
The current implementation has these **strengths** for future integration:
1. **Granular Visibility Tracking**:
   - Already distinguishes `pub`, `pub(crate)`, `pub(super)` etc.
   - Use statement resolution handles basic import paths

2. **Extension Points**:
   - `VisibilityResult` enum is designed for additional states
   - Module path tracking exists (feature-gated)

**Gaps for Future Needs**:
1. **File System Context**:
   - No knowledge of physical file locations
   - Can't resolve `pub(in path)` without filesystem data

2. **Workspace Awareness**:
   - Current design assumes single-crate context
   - No crate boundary tracking

### Implementation Recommendation

**Proposed Path Forward**:

1. **Complete Phase 3 Visibility Resolution** with these adjustments:
   - Modify `VisibilityResult` to explicitly track:
     ```rust
     pub enum VisibilityResult {
         Direct,
         NeedsUse(Vec<String>),
         OutOfScope {
             reason: OutOfScopeReason, // New enum
             allowed_scopes: Vec<String> // For pub(in path)
         },
         NeedsCfg, // For cfg-gated items
     }
     ```
   - Document as "single-crate resolution" with TODOs for future expansion

2. **Parallelize File Tracking Spike**:
   - Create experimental branch adding:
     ```rust
     #[derive(Debug)]
     pub struct FileLocation {
         pub path: PathBuf,
         pub in_workspace: bool,
         pub is_dependency: bool,
     }
     ```
   - Test integration with existing visibility system

**Rationale**:
- Gets us to MVP faster with working visibility
- Allows incremental addition of filesystem awareness
- Prevents blocking on complex workspace cases
- File tracking can evolve independently

### Suggested Immediate Actions

1. **Phase 3 Finalization**:
```rust
// Add to visibility_resolution.md
## Limitations (MVP Scope)
- Single-crate resolution only
- Physical file locations not considered
- Workspace visibility not handled
```

2. **File Tracking Spike**:
```bash
git checkout -b feat/file-tracking
# Create crates/ingest/syn_parser/src/parser/file_context.rs
```

Would you like me to:
1. Draft the revised Phase 3 implementation?
2. Propose a file tracking design?
3. Or focus on another aspect?

This approach gives us deployable visibility resolution while leaving the door open for the more complex workspace/file system integration later.
