# Iterator Advice Regressed To Vec

- date: 2026-05-22 local
- family: model-communication-failures
- touched surface: `crates/ploke-tui/src/tools/cargo.rs`

## Trigger

The user challenged a proposed cleanup for cargo detail rendering:

```text
wow. just why? why do you insist on returning a `Vec`? Are you allergic to iterators?
```

## User-Visible Failure

The agent criticized an existing helper for allocating a `Vec<&str>`, then
suggested another helper that still collected into a `Vec<&str>`.

## What The Agent Did

The agent stayed inside the shape of the first patch instead of recognizing
that this was a display pipeline and should consume iterators directly. That
made the recommendation look mechanically local rather than idiomatic Rust.

## Skipped Discipline

The agent used Rust review language but did not fully apply the allocation and
iterator questions from `rust-review-discipline`.

## Why This Was Risky

This kind of answer teaches the wrong local style: allocate bounded vectors for
small display slices even when a borrowed iterator and direct rendering are
clearer. It also erodes trust because the stated critique and the proposed
replacement contradicted each other.

## Prevention Rule

When criticizing a helper for unnecessary collection, the replacement must
either return an iterator, accept an `IntoIterator`, or write directly to the
sink. Do not retain a `Vec` return shape unless ownership or multi-pass use is
part of the contract.
