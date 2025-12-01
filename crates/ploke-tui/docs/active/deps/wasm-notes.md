# On deps for wasm

## Deps that need to be changed for wasm

- `mpatch`
  - source https://docs.rs/mpatch/1.3.3/mpatch/
  - uses "parallel" feature by default, but this won't work if the target doesn't support threading, doc calls out `wasm-unknown-unknown` for this.
  - modify dependency to: `mpatch = { version = "1.3.3", default-features = false }`
