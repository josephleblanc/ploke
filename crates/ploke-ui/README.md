# ploke-ui: A UI crate for ploke

This is the UI crate for the `ploke` project, and provides an `egui` UI to interact with the database containing the parsed code graph of the target rust source code.

## Crate Overview

### File Structure

```ls
code_graph_navigator_ui/
├── Cargo.toml
└── src/
    ├── main.rs                     // Entry point, eframe setup
    ├── app.rs                      // Main eframe App struct and its impl
    │
    ├── ui/                         // eframe specific UI components and layouts
    │   ├── mod.rs
    │   ├── file_panel.rs           // UI for file selection, parsing control
    │   ├── query_panel.rs          // UI for Datalog query input and execution
    │   ├── results_panel.rs        // UI for displaying query results in a table
    │   └── common_widgets.rs       // Reusable small widgets (e.g., status indicators)
    │
    ├── state/                      // Application state management
    │   ├── mod.rs
    │   └── app_state.rs            // Defines the main AppState struct and related enums/data
    │
    ├── core/                       // Backend logic
    │   ├── mod.rs
    │   ├── parser_mod.rs           // Wrapper/interface for syn-based parser
    │   ├── graph_transformer.rs    // Transforms parser output to CozoDB structures
    │   ├── cozo_db_manager.rs      // Manages CozoDB instance, schema, data i/o, queries
    │   └── types.rs                // Common data types (e.g., for parsed nodes, query results)
    │
    └── channels.rs                 // For communication between UI and background threads
    └── error.rs                    // Custom error types for the application
```

## TODO

### Project Organization

- [✔] Create an initial organized file structure.
  - [ ] Review and refactor file structure.
- [ ] Reorganize `main.rs` into newly added files

### Short-term

- [✔] Implement table of returned results from the `cozo` query.

#### Overall Organization

- [ ] Implement tab-like selection of window components
  - [ ] Add a "Builder" vs "Custom" Query box

#### Query Box
- [ ] Execute query on `<Shift-Enter>`
- [ ] Add default queries in a dropdown menu.
  - [ ] Add tabs for different kinds of common queries, tbd.
- [ ] Add syntax highlighting for datalog queries
  - [ ] Look into the [syntect] crate for syntax highlighting through [egui_extras].

**Fix**

- [ ] Stutter in `Selected Items:` on presseing `<Enter>` within `Query` code box.

#### Table area
- [ ] Make the table interactions awesome
  - [ ] Add visual responsiveness to table, such as:
    - [✔] highlight cell on click
    - [ ] click-drag to select cells
  - [ ] Add functional responsiveness to table
    - [ ] right click on cell with popup options
      - [ ] copy single cell
      - [ ] copy selected cells
      - [ ] query for matches of cell value
      - [ ] fetch contents of referenced item using span to read byte location from file
    - [ ] click-drag to select multiple cells
      - [ ] do cool things with multiple cells?
    - [ ] implement control groups for cell selection,
      - [ ] select cell(s) and press `<Ctrl-1>` or other num to highlight all selected cells in color
      - [ ] allow right-click options to apply to all cells in the same control group.
      - [ ] click-drag on cell item in control group to extend control group selection (controversial?)
      - [ ] (maybe) for any selected cells from a control group in the query, auto-update query text
        to reflect changes in control groups

#### Error Handling
- [ ] Consider switching to `miette` for error handling, or at least include crate so I can forward the error messages from cozo.
- [ ] Forward some db query errors to the UI.

### Longer term (dependent on other crates)
- [ ] Integrate query-builder into ui

## TODO from other crates

### ploke-db

- [ ] Implement query-builder


[egui_extras]:https://docs.rs/egui_extras/latest/egui_extras/index.html
[syntect]:https://docs.rs/syntect/latest/syntect/
