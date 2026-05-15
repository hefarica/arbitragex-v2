//! Z-GPU — invariantes del GPU EVM Coprocessor.

use gpu_evm_rs::{GpuAddress, GpuError, GpuEvmCoprocessor, StubGpuBackend, MAX_BATCH};
use std::time::Duration;

fn addr(b: u8) -> GpuAddress {
    GpuAddress::from_bytes([b; 20])
}

fn coproc(stub: StubGpuBackend, ms: u64) -> GpuEvmCoprocessor<StubGpuBackend> {
    GpuEvmCoprocessor::new(stub, Duration::from_millis(ms)).expect("ready")
}

#[tokio::test]
async fn z_gpu_01_batch_ok_filtra_yield_positivo() {
    let coproc = coproc(StubGpuBackend::default(), 500);
    let routes = vec![
        vec![addr(0x10), addr(0x20)],  // suma = 16+16+...+32+32... = par
        vec![addr(0x11), addr(0x21)],  // suma impar → descartado
    ];
    let res = coproc.simulate_batch(routes).await.unwrap();
    // Cada hop = 20 bytes iguales => suma por hop = byte*20; total par↔suma par
    // 0x10*20 + 0x20*20 = par → incluido; 0x11*20 + 0x21*20 = par tambien
    // Validación: TODOS los results tienen yield > 0
    for r in &res {
        assert!(r.topological_yield_wei > 0, "yield debe ser > 0 cuando incluido");
        assert_eq!(r.gas_estimate, 42_000);
    }
}

#[tokio::test]
async fn z_gpu_02_backend_no_ready_rechazado_en_constructor() {
    let stub = StubGpuBackend::new(false);
    match GpuEvmCoprocessor::new(stub, Duration::from_millis(100)) {
        Ok(_) => panic!("se esperaba ContextLost"),
        Err(e) => assert_eq!(e, GpuError::ContextLost),
    }
}

#[tokio::test]
async fn z_gpu_03_batch_vacio_rechazado() {
    let coproc = coproc(StubGpuBackend::default(), 500);
    let err = coproc.simulate_batch(vec![]).await.unwrap_err();
    assert_eq!(err, GpuError::BatchEmpty);
}

#[tokio::test]
async fn z_gpu_04_batch_demasiado_grande_rechazado() {
    let coproc = coproc(StubGpuBackend::default(), 500);
    let big: Vec<Vec<GpuAddress>> = (0..(MAX_BATCH + 1))
        .map(|_| vec![addr(0x01), addr(0x02)])
        .collect();
    let err = coproc.simulate_batch(big).await.unwrap_err();
    assert!(matches!(err, GpuError::BatchTooLarge(_)));
}

#[tokio::test]
async fn z_gpu_05_ruta_con_un_hop_rechazada() {
    let coproc = coproc(StubGpuBackend::default(), 500);
    let err = coproc
        .simulate_batch(vec![vec![addr(0x01)]])
        .await
        .unwrap_err();
    match err {
        GpuError::InvalidRoute { idx, .. } => assert_eq!(idx, 0),
        _ => panic!("se esperaba InvalidRoute"),
    }
}

#[tokio::test]
async fn z_gpu_06_ruta_demasiado_larga_rechazada() {
    let coproc = coproc(StubGpuBackend::default(), 500);
    let r = vec![addr(0x01); 7];
    let err = coproc.simulate_batch(vec![r]).await.unwrap_err();
    assert!(matches!(err, GpuError::InvalidRoute { .. }));
}

#[tokio::test]
async fn z_gpu_07_timeout_se_dispara() {
    let stub = StubGpuBackend::default();
    stub.set_delay(Duration::from_millis(300)).await;
    let coproc = coproc(stub, 30);
    let err = coproc
        .simulate_batch(vec![vec![addr(0x10), addr(0x20)]])
        .await
        .unwrap_err();
    assert!(matches!(err, GpuError::Timeout(_)));
}

#[tokio::test]
async fn z_gpu_08_context_lost_se_propaga_desde_backend() {
    let stub = StubGpuBackend::default();
    stub.force_error(GpuError::ContextLost).await;
    let coproc = coproc(stub, 500);
    let err = coproc
        .simulate_batch(vec![vec![addr(0x10), addr(0x20)]])
        .await
        .unwrap_err();
    assert_eq!(err, GpuError::ContextLost);
}

#[tokio::test]
async fn z_gpu_09_determinismo_misma_ruta_mismo_yield() {
    let coproc = coproc(StubGpuBackend::default(), 500);
    let r = vec![vec![addr(0x10), addr(0x20)]];
    let a = coproc.simulate_batch(r.clone()).await.unwrap();
    let b = coproc.simulate_batch(r).await.unwrap();
    assert_eq!(a, b, "el stub debe ser determinístico");
}
