use {
    hex::FromHex,
    criterion::{
        criterion_group,
        criterion_main,
        Criterion
    },
    ws_gonzale::dataframe::{
        mask_payload_mut,
        structered,
        flat
    }
};

fn bench_one(c: &mut Criterion) {
    c.bench_function("Speedy unmasking", |b| {
        let hex_dump = "81 9c 9d f4 e4 dc cf 9b 87 b7 bd 9d 90 fc ea 9d 90 b4 bd bc b0 91 d1 c1 c4 8b f8 96 b7 b3 fe 9f 81 a8".replace(" ", "");
        let mut binary_vec: Vec<u8> = Vec::from_hex(hex_dump).expect("Invalid Hex String");
        b.iter(|| {
            let _ = mask_payload_mut(&mut binary_vec, [0, 1, 1, 0]);
        })
    });
}
fn bench_mut(c: &mut Criterion) {
    c.bench_function("Speedy unmasking vec", |b| {
        let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
        let dataframe = flat::Dataframe::new(buffer);
        b.iter(|| {
            let _ = dataframe.get_unmasked_payload();
        })
    });
}
fn bench_two(c: &mut Criterion) {
    c.bench_function("Speedy conversion of slice to dataframe", |b| {
        let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
        let buffer = buffer.as_slice();
        b.iter(|| {
            let _: structered::Dataframe = buffer.into();
        })
    });
}


criterion_group!(benches, bench_one, bench_two, bench_mut);
criterion_main!(benches);