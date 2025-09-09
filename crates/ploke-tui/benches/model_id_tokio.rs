use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use tokio::runtime::Runtime;
use fxhash::FxBuildHasher;
use std::collections::HashMap;

mod common;
use common::{DIntern, GLOBAL_RODEO, gen_repeated_inputs, preseed_global_rodeo};

async fn end2end_intern_async_with<F>(
    inputs: &[String],
    tasks: usize,
    build: F,
) -> HashMap<DIntern, (u32, String), FxBuildHasher>
where
    F: Copy + Send + Sync + 'static + Fn(&DIntern, &lasso::ThreadedRodeo) -> String,
{
    use tokio::task::JoinSet;
    let chunk = inputs.len().div_ceil(tasks);
    let mut set = JoinSet::new();

    for part in inputs.chunks(chunk) {
        let part = part.to_vec();
        set.spawn(async move {
            let rodeo = &*GLOBAL_RODEO;
            let mut local: HashMap<DIntern, (u32, String), FxBuildHasher> =
                HashMap::with_capacity_and_hasher(part.len(), FxBuildHasher::default());
            for s in &part {
                if let Some(id) = DIntern::parse(rodeo, black_box(s)) {
                    let url = build(&id, rodeo);
                    let e = local.entry(id).or_insert((0, url));
                    e.0 += 1;
                }
            }
            local
        });
    }

    // reduce
    let mut merged: HashMap<DIntern, (u32, String), FxBuildHasher> =
        HashMap::with_capacity_and_hasher(inputs.len(), FxBuildHasher::default());
    while let Some(res) = set.join_next().await {
        let sub = res.unwrap();
        for (k, (n, url)) in sub {
            let e = merged.entry(k).or_insert((0, url));
            e.0 += n;
        }
    }
    merged
}

fn bench_tokio_end2end(c: &mut Criterion) {
    // dataset: repeated to highlight interning benefits
    let inputs = gen_repeated_inputs(1_000, 100); // 100k total
    preseed_global_rodeo(&inputs);                // warm once

    let rt = Runtime::new().unwrap();

    for &tasks in &[1usize, 5, 10, 50] {
        // Baseline
        c.bench_with_input(
            BenchmarkId::new("tokio_intern+baseline", tasks),
            &tasks,
            |b, &tasks| {
                b.to_async(&rt).iter(|| async {
                    let map = end2end_intern_async_with(&inputs, tasks, DIntern::endpoints_url).await;
                    black_box(map);
                })
            },
        );

        // Concat-known
        c.bench_with_input(
            BenchmarkId::new("tokio_intern+concat_known", tasks),
            &tasks,
            |b, &tasks| {
                b.to_async(&rt).iter(|| async {
                    let map = end2end_intern_async_with(&inputs, tasks, DIntern::endpoints_url_concat_known).await;
                    black_box(map);
                })
            },
        );

        // Prealloc + write
        c.bench_with_input(
            BenchmarkId::new("tokio_intern+prealloc_write", tasks),
            &tasks,
            |b, &tasks| {
                b.to_async(&rt).iter(|| async {
                    let map = end2end_intern_async_with(&inputs, tasks, DIntern::endpoints_url_prealloc_write).await;
                    black_box(map);
                })
            },
        );

        // Replace
        c.bench_with_input(
            BenchmarkId::new("tokio_intern+replace", tasks),
            &tasks,
            |b, &tasks| {
                b.to_async(&rt).iter(|| async {
                    let map = end2end_intern_async_with(&inputs, tasks, DIntern::endpoints_url_replace).await;
                    black_box(map);
                })
            },
        );
    }
}

criterion_group!(tokio_model_benches, bench_tokio_end2end);
criterion_main!(tokio_model_benches);
