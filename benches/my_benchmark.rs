use {
    hex::FromHex,
    criterion::{
        criterion_group,
        criterion_main,
        Criterion
    },
    ws_gonzale::dataframe::mask_payload_mut
};

fn unmasking_payload_mut(c: &mut Criterion) {
    c.bench_function("mask_payload_mut", |b| {
        let hex_dump = "81 9c 9d f4 e4 dc cf 9b 87 b7 bd 9d 90 fc ea 9d 90 b4 bd bc b0 91 d1 c1 c4 8b f8 96 b7 b3 fe 9f 81 a8".replace(" ", "");
        let mut binary_vec: Vec<u8> = Vec::from_hex(hex_dump).expect("Invalid Hex String");
        b.iter(|| {
            let _ = mask_payload_mut(&mut binary_vec, [0, 1, 1, 0]);
        })
    });
}
fn bench_dataframe(c: &mut Criterion) {
    c.bench_function("Buffer to dataframe::flat", |b| {
        b.iter(|| {
            let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
            let _: ws_gonzale::flat::Dataframe = buffer.into();
        })
    });
    c.bench_function("Buffer to dataframe::structured", |b| {
        b.iter(|| {
            let buffer: Vec<u8> = vec![129, 139, 90, 212, 118, 181, 18, 177, 26, 217, 53, 244, 33, 218, 40, 184, 18];
            let _: ws_gonzale::structered::Dataframe = buffer.into();
        })
    });
}


criterion_group!(benches, unmasking_payload_mut, bench_dataframe);
criterion_main!(benches);