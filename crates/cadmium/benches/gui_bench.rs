use std::hint::black_box;

use cadmium::{
    Column, Container, ForEach, IntoElement, Length, Text, TextEngine, VirtualRow, VirtualizedList,
    WidgetArena, create_selector, create_signal, flush_effects,
};
use criterion::{Criterion, criterion_group, criterion_main};

fn build_keyed_rows(n: usize) -> (cadmium::Signal<Vec<usize>>, Vec<cadmium::Signal<usize>>) {
    let ids: Vec<usize> = (0..n).collect();
    let row_signals: Vec<_> = (0..n).map(create_signal).collect();
    let list_sig = create_signal(ids);
    (list_sig, row_signals)
}

fn bench_create_1k(c: &mut Criterion) {
    let mut text = TextEngine::new();
    c.bench_function("create_1k_rows", |b| {
        b.iter(|| {
            let (list_sig, row_sigs) = build_keyed_rows(1_000);
            let row_sigs_ui = row_sigs.clone();
            let mut arena = WidgetArena::new();
            let elem = ForEach::new(
                list_sig,
                |&id| id as u64,
                move |&id| {
                    let s = row_sigs_ui[id];
                    Container::row()
                        .height(Length::Px(28.0))
                        .child(Text::reactive(move || format!("Row #{}", s.get())))
                        .into_element()
                },
            )
            .into_element();
            arena.mount(Column::new().child(elem).into_element(), &mut text);
            arena.bench_layout_pass(1280.0, 800.0, &mut text);
            black_box(&arena);
            arena.clear();
            list_sig.dispose();
            for s in row_sigs {
                s.dispose();
            }
        });
    });
}

fn bench_select_and_update_10k(c: &mut Criterion) {
    let mut text = TextEngine::new();
    let (list_sig, row_sigs) = build_keyed_rows(10_000);
    let row_sigs_ui = row_sigs.clone();
    let selected = create_signal(0_usize);
    let sel = create_selector(selected);
    let mut arena = WidgetArena::new();

    let sel_ui = sel.clone();
    let elem = ForEach::new(
        list_sig,
        |&id| id as u64,
        move |&id| {
            let s = row_sigs_ui[id];
            let sel_row = sel_ui.clone();
            Container::row()
                .width(Length::Px(400.0))
                .height(Length::Px(28.0))
                .child(Text::reactive(move || {
                    if sel_row.is_selected(id) {
                        format!("> Selected #{}", s.get())
                    } else {
                        format!("Row #{}", s.get())
                    }
                }))
                .into_element()
        },
    )
    .into_element();

    arena.mount(Column::new().child(elem).into_element(), &mut text);
    arena.bench_layout_pass(1280.0, 800.0, &mut text);
    assert!(arena.node_count() > 10_000, "ForEach mount regression");

    let mut toggle = 0usize;
    c.bench_function("select_row_10k", |b| {
        b.iter(|| {
            toggle = (toggle + 1) % 10_000;
            selected.set(toggle);
            flush_effects();
            arena.bench_sync_and_layout(1280.0, 800.0, &mut text);
        });
    });

    c.bench_function("update_every_10th_10k", |b| {
        b.iter(|| {
            for (i, s) in row_sigs.iter().enumerate().step_by(10) {
                s.update(|v| *v = v.wrapping_add(i + 1));
            }
            flush_effects();
            arena.bench_sync_and_layout(1280.0, 800.0, &mut text);
        });
    });

    arena.clear();
    sel.dispose();
    selected.dispose();
    list_sig.dispose();
    for s in row_sigs {
        s.dispose();
    }
}

fn bench_virtualized_100k_layout(c: &mut Criterion) {
    let mut text = TextEngine::new();
    let mut arena = WidgetArena::new();
    let list = VirtualizedList::new(100_000, 48.0, |idx| VirtualRow {
        title: format!("Key user:session:{idx:06}"),
        subtitle: "hash • 14 fields • TTL 3600s".into(),
        badge: "HASH".into(),
    });
    arena.mount(Column::new().child(list).into_element(), &mut text);
    arena.bench_layout_pass(1280.0, 800.0, &mut text);

    c.bench_function("scroll_step_100k_virtual", |b| {
        b.iter(|| {
            arena.bench_scroll_virtual_step(160.0, &mut text);
        });
    });
}

criterion_group!(
    benches,
    bench_create_1k,
    bench_select_and_update_10k,
    bench_virtualized_100k_layout
);
criterion_main!(benches);
