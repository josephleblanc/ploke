use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

fn bench_keymap_normal_nav(c: &mut Criterion) {
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    c.bench_function("keymap_normal_nav_j", |b| {
        b.iter(|| {
            let _ = black_box(to_action(Mode::Normal, key, CommandStyle::NeoVim));
        })
    });
}

fn bench_keymap_page_down(c: &mut Criterion) {
    let key = KeyEvent::new(KeyCode::Char('J'), KeyModifiers::NONE);
    c.bench_function("keymap_normal_page_down_J", |b| {
        b.iter(|| {
            let _ = black_box(to_action(Mode::Normal, key, CommandStyle::NeoVim));
        })
    });
}

criterion_group!(
    benches,
    bench_keymap_to_action,
    bench_keymap_normal_nav,
    bench_keymap_page_down
);
criterion_main!(benches);
