# Bugs that are still alive

## Timing issue on indexing

After indexing, for the first time I encountered this error, despite having run all tests recently and not making changes to the files in `ploke-embed`:
```
The application panicked (crashed).
Message:  Failed to shutdown CallbackManager via shutdown send: "SendError(..)"
Location: /home/brasides/code/second_aider_dir/ploke/crates/ingest/ploke-embed/src/indexer.rs:298

Backtrace omitted. Run with RUST_BACKTRACE=1 environment variable to display it.
Run with RUST_BACKTRACE=full to include source snippets.
```

This happened after indexing `/index start tests/fixture_crates/fixture_tracking_hash`. I may or may not have immediately entered another command.
