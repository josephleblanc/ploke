/*!
Input handling scaffolding.

Phase 1 introduces a dedicated keymap that translates KeyEvent to high-level
Actions for the App to handle. This keeps the input loop simple and enables
clean testing.

See: `input::keymap` for the Action enum and mapping.
*/

pub mod keymap;
