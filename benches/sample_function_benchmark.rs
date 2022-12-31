use criterion::{black_box, criterion_main, Criterion};
use std::time::Duration;
use tokio::runtime::Runtime;

use drax::transport::buffer::var_num::{size_var_int, size_var_long};
use drax::transport::buffer::DraxWriteExt;

fn benchmark_variable_numbers(c: &mut Criterion) {
    let mut group = c.benchmark_group("Variable Number Benchmarks");
    for iter_size in [10, 1_000, 100_000, 10_000_000] {
        for var_int in [20, 32767, i32::MAX] {
            group.bench_with_input(
                format!("Write {} Var Ints with value {}", iter_size, var_int),
                &(iter_size, var_int),
                |b, (iter_size, var_int)| {
                    b.to_async(Runtime::new().unwrap()).iter(|| {
                        Box::pin(async move {
                            let mut buffer = Vec::with_capacity(size_var_int(*var_int));
                            for _ in 0..*iter_size {
                                buffer.write_var_int(*var_int).await.unwrap();
                                buffer.clear();
                            }
                        })
                    });
                },
            );
        }
        for var_long in [20, 32767, i32::MAX as i64, i64::MAX] {
            group.bench_with_input(
                format!("Write {} Var Longs with value {}", iter_size, var_long),
                &(iter_size, var_long),
                |b, (iter_size, var_long)| {
                    b.to_async(Runtime::new().unwrap()).iter(|| {
                        Box::pin(async move {
                            let mut buffer = Vec::with_capacity(size_var_long(*var_long));
                            for _ in 0..*iter_size {
                                buffer.write_var_long(*var_long).await.unwrap();
                                buffer.clear();
                            }
                        })
                    });
                },
            );
        }
    }
}

pub fn benches() {
    let mut criterion = Criterion::default().measurement_time(Duration::from_secs(10));
    benchmark_variable_numbers(&mut criterion);
}

criterion_main!(benches);
