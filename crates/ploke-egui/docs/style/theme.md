# Theme system (`ploke-egui`)

Operator UI colors come from **named palettes** applied on the cold path only. Theme tokens are `Copy` and stored in egui context temp data after `set_visuals`; the graph hot path does not allocate per frame for theming.

## Palettes

| ID | Label | Mode |
|----|-------|------|
| `tokyo_night` | Tokyo Night | dark (default) |
| `dracula` | Dracula | dark |
| `gruvbox_dark` | Gruvbox Dark | dark |
| `one_dark` | One Dark | dark |
| `gruvbox_light` | Gruvbox Light | light |
| `one_light` | One Light | light |

## Semantic tokens

Defined on [`PaletteTokens`](../../src/ui/theme/palette.rs):

- Chrome: `background`, `panel`, `text`, `text_muted`, `border`, `accent`
- Status: `success`, `warning`, `error`, `info`
- Graph edges ([`StatusColors`](../../src/ui/view/style.rs)): `graph_synthesized`, `graph_opened_from`, `graph_selected`, `graph_applied`, `graph_restored`, `graph_dropped`
- Verdict emphasis: `verdict_good`, `verdict_neutral`, `verdict_concerning`
- Diff inspector: `diff_add`, `diff_remove`, `diff_hunk`, `diff_meta`
- Badges: `badge_parent`, `badge_child`, `badge_text`

## Runtime

- [`AppTheme`](../../src/ui/theme/scheme.rs) lives on [`OperatorApp`](../../src/ui/app/mod.rs).
- Selector: top strip ([`shell/chrome.rs`](../../src/ui/app/shell/chrome.rs)).
- Persistence: eframe storage key `ploke-egui-theme` (native + WASM when storage is available).
- On change: `set_visuals` once, sync graph `StatusColors`, clear patch-diff and inspector text caches.

## Usage in UI code

```rust
let tokens = crate::ui::theme::tokens_from_ui(ui);
// use tokens.warning, tokens.success, etc.
```

Do not hardcode `Color32::from_rgb` for semantic states; extend `PaletteTokens` if a new role is needed.

## Related

- Autonomy loop: [`docs/active/agents/ploke-egui-design-autonomy/README.md`](../../../../docs/active/agents/ploke-egui-design-autonomy/README.md)
- Inspector copy rules: [`inspector-ux.md`](inspector-ux.md)
