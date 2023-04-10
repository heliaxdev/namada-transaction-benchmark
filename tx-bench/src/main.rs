use criterion::{criterion_group, criterion_main, Criterion};
mod tx;

pub fn benchmark(c: &mut Criterion) {
    tx::transfer(c);
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
