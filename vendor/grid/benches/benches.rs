use criterion::{criterion_group, criterion_main, Criterion};
use grid::grid;
use grid::Grid;
use rand::Rng;

const SIZE: usize = 1_000;

fn init_vec_vec() -> Vec<Vec<u8>> {
    vec![vec![0; SIZE]; SIZE]
}

fn init_grid() -> Grid<u8> {
    Grid::init(SIZE, SIZE, 0)
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::rng();
    let mut rand = || rng.random_range(0..SIZE);

    // Init macro
    c.bench_function("vecvec_init_macro", |b| {
        b.iter(|| {
            vec![
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            ]
        })
    });
    c.bench_function("grid_init_macro", |b| {
        b.iter(|| {
            grid![[0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]
            [0,1,2,3,4,5,6,7,8,9]]
        })
    });
    c.bench_function("grid_from_vec", |b| {
        b.iter(|| {
            let vec = vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7,
                8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5,
                6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3,
                4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            ];
            Grid::from_vec(vec, 10)
        })
    });

    // Constructor
    c.bench_function("vecvec_init", |b| b.iter(|| vec![vec![0; SIZE]; SIZE]));
    c.bench_function("grid_init", |b| b.iter(|| Grid::init(SIZE, SIZE, 0)));

    // Index
    c.bench_function("vecvec_idx", |b| {
        let vec_vec = init_vec_vec();
        b.iter_batched(
            || (rand(), rand()),
            |(x, y)| {
                let _v = vec_vec[x][y];
            },
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_idx", |b| {
        let grid = init_grid();
        b.iter_batched(
            || (rand(), rand()),
            |(x, y)| grid[(x, y)],
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("vecvec_get", |b| {
        let vec_vec = init_vec_vec();
        b.iter_batched(
            || (rand(), rand()),
            |(x, y)| {
                let _v = vec_vec.get(x).unwrap().get(y).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_get", |b| {
        let grid = init_grid();
        b.iter_batched(
            || (rand(), rand()),
            |(x, y)| {
                let _v = grid.get(x, y).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    //Set
    c.bench_function("vecvec_set", |b| {
        let mut vec_vec = init_vec_vec();
        b.iter_batched(
            || (rand(), rand()),
            |(x, y)| vec_vec[x][y] = 42,
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_set", |b| {
        let mut g = init_grid();
        b.iter_batched(
            || (rand(), rand()),
            |(x, y)| g[(x, y)] = 42,
            criterion::BatchSize::SmallInput,
        )
    });

    // Push
    c.bench_function("grid_push_row", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.push_row(vec![0; SIZE]),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_push_col", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.push_col(vec![0; SIZE]),
            criterion::BatchSize::SmallInput,
        )
    });

    // Pop
    c.bench_function("grid_pop_row", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.pop_row(),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_pop_col", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.pop_col(),
            criterion::BatchSize::SmallInput,
        )
    });

    // Remove
    c.bench_function("grid_remove_row", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.remove_row(2),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_remove_col", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.remove_col(2),
            criterion::BatchSize::SmallInput,
        )
    });

    // Rotation
    c.bench_function("grid_rotate_left", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.rotate_left(),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_rotate_right", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.rotate_right(),
            criterion::BatchSize::SmallInput,
        )
    });
    c.bench_function("grid_rotate_half", |b| {
        let grid = init_grid();
        b.iter_batched(
            || grid.clone(),
            |mut g| g.rotate_half(),
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
