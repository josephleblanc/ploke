# Design Philosophy: Static Dispatch via Monomorphization

## Core Principle

In the Ploke project, particularly within the `syn_parser` crate and its handling of code graph elements and identifiers, we prioritize **static dispatch** achieved through **monomorphization** wherever feasible. This means leveraging Rust's generic system and trait bounds to resolve method calls and generate specialized code at compile time, rather than relying on dynamic dispatch mechanisms (like trait objects with vtables) at runtime.

## Connection to the Typed ID System

Our strictly typed ID system (e.g., `ModuleNodeId`, `FunctionNodeId`, `PrimaryNodeId`, `AnyNodeId`) is a cornerstone of this philosophy.

1.  **Distinct Types:** Each specific ID (e.g., `FunctionNodeId`) is a unique type (a newtype wrapper).
2.  **Marker Traits:** Traits like `PrimaryNodeIdTrait`, `AssociatedItemNodeIdTrait` categorize these specific IDs.
3.  **Generic Functions:** We write generic functions that operate on IDs, using trait bounds to constrain the types they accept. For example:
    ```rust
    // Example from relations.rs
    impl SyntacticRelation {
        // T must be a type that implements PrimaryNodeIdTrait
        pub fn contains_target<T: PrimaryNodeIdTrait>(&self, src: ModuleNodeId) -> Option<T> {
            match self {
                Self::Contains { source: s, target } if *s == src => {
                    // target is PrimaryNodeId
                    // Attempt conversion using TryFrom<PrimaryNodeId> for T
                    T::try_from(*target).ok()
                }
                _ => None,
            }
        }
    }
    ```

## Monomorphization Explained

When the Rust compiler encounters a call to a generic function like `contains_target::<FunctionNodeId>(...)`, it performs **monomorphization**:

*   It effectively creates a *new, specialized version* of that function specifically for the concrete type `FunctionNodeId`.
*   All generic type parameters (`T`) are replaced with the concrete type (`FunctionNodeId`).
*   Trait bound checks (`T: PrimaryNodeIdTrait`) are verified at compile time.
*   Method calls within the generic function (like `T::try_from`) are resolved to the specific implementation for the concrete type (`impl TryFrom<PrimaryNodeId> for FunctionNodeId`).

The compiler generates separate, optimized code for each concrete type the generic function is used with (e.g., one version for `FunctionNodeId`, another for `StructNodeId`, etc.).

## Benefits

1.  **Compile-Time Type Safety:** The compiler guarantees that only valid ID types (those satisfying the trait bounds) can be used with the generic function. Type mismatches are caught early. The use of `TryFrom` within the function ensures that even if a `PrimaryNodeId` is passed, it's explicitly checked against the *requested* specific type `T`.
2.  **Performance:** Static dispatch avoids the runtime overhead associated with dynamic dispatch (like vtable lookups). The specialized, monomorphized code can often be inlined and optimized further by the compiler, leading to performance comparable to hand-written non-generic code.
3.  **Clarity of Intent:** Generic functions with clear trait bounds explicitly document the *capabilities* required of the types they operate on. When calling such a function with a specific type argument (e.g., `::<FunctionNodeId>`), the intent is unambiguous.

## Contrast with Dynamic Dispatch

The alternative would be to use trait objects (e.g., `&dyn PrimaryNodeIdTrait`). While powerful for heterogeneous collections or situations requiring runtime flexibility, dynamic dispatch incurs a runtime cost (vtable lookup) and often provides fewer compile-time guarantees about the specific underlying type. We prefer static dispatch for the core graph operations where performance and type rigidity are paramount.

## Tradeoffs

*   **Compile Time:** Monomorphization can increase compile times as the compiler generates code for each concrete type instantiation.
*   **Binary Size:** Generating multiple specialized versions of functions can potentially increase the final binary size, although optimizations often mitigate this.

## Conclusion

By embracing static dispatch via monomorphization, especially in conjunction with our typed ID system, we aim to build a `syn_parser` that is:

*   **Robust:** Catching type errors at compile time.
*   **Performant:** Avoiding runtime dispatch overhead for critical operations.
*   **Maintainable:** Providing clear type constraints and leveraging the compiler for verification.

This approach aligns with Rust's philosophy of zero-cost abstractions and compile-time guarantees.
