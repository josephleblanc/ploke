# Table of Contents (ToC) Generation Instructions

Purpose
- Create and maintain a memory file `table_of_contents.md` summarizing every file in `crates/ploke-tui/src` for fast navigation during tasks.

Iterative Procedure (per file)
1) Read the file.
2) Add a summary to the memory with:
   - Relative file path and canonical module path (e.g., `ploke_tui::app::view::components::model_browser`).
   - Purpose of the file and its most closely related file(s).
   - Other relevant notes (key types/functions, responsibilities, caveats, feature flags).
3) Review existing ToC entries; determine if any need updates.
   - If yes, read the target file before editing its summary; only update if still warranted.
   - If no, continue.
4) Compact conversation usage (avoid long outputs; store details in memory, keep chat minimal).
5) Restate these instructions and intention to follow them succinctly.
6) Move to the next file and repeat until all files under `src` are covered.

Finalization
- After all files are present in `table_of_contents.md`, review the ToC for inconsistencies or gaps.
  - If inconsistencies/questions remain, create a doc for user review capturing the questions.
  - Otherwise, create a new "workflows" document describing cross-file user interaction workflows in ploke-tui.

Notes
- Keep each summary short and specific; link related modules/files.
- Prefer stable, typed descriptions (avoid ambiguity).
- Update ToC incrementally as code evolves.
