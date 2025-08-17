use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ploke_tui::app::input::keymap::to_action;
use ploke_tui::app::types::Mode;
use ploke_tui::user_config::CommandStyle;

fn bench_keymap_to_action(c: &mut Criterion) {
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    c.bench_function("keymap_insert_char", |b| {
        b.iter(|| {
            let _ = black_box(to_action(Mode::Insert, key, CommandStyle::NeoVim));
        })
    });
}

criterion_group!(benches, bench_keymap_to_action);
criterion_main!(benches);
