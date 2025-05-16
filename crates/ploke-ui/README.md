# ploke-ui: A UI crate for ploke

This is the UI crate for the `ploke` project, and provides an `egui` UI to interact with the database containing the parsed code graph of the target rust source code.

## TODO

### Short-term

- [ ] Implement table of returned results from the `cozo` query.
- [ ] Add default queries in a dropdown menu.
  - [ ] Add tabs for different kinds of common queries, tbd.
- [ ] Add syntax highlighting for datalog queries
  - [ ] Look into the [syntect] crate for syntax highlighting through [egui_extras].
- [ ] Make the table interactions awesome
  - [ ] Add visual responsiveness to table, such as:
    - [ ] highlight cell on click
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
      - [ ] select cell(s) and press <Ctrl-1> or other num to highlight all selected cells in color
      - [ ] allow right-click options to apply to all cells in the same control group.
      - [ ] click-drag on cell item in control group to extend control group selection (controversial?)
      - [ ] (maybe) for any selected cells from a control group in the query, auto-update query text
        to reflect changes in control groups

### Longer term (dependent on other crates)
- [ ] Integrate query-builder into ui

## TODO from other crates

### ploke-db

- [ ] Implement query-builder


[egui_extras]:https://docs.rs/egui_extras/latest/egui_extras/index.html
[syntect]:https://docs.rs/syntect/latest/syntect/
