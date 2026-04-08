use kursal_core::first_contact::otp;
use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
pub struct BenchmarkProgress {
    pub current: usize,
    pub total: usize,
    pub elapsed_ms: u64,
}

#[derive(Clone, serde::Serialize)]
pub struct BenchmarkResult {
    pub iterations: usize,
    pub average_per_iteration_ms: f64,
    pub average_with_threading_ms: f64,
    pub total_ms: f64,
    pub iterations_per_second: f64,
}

static BENCHMARK_CANCELLED: AtomicBool = AtomicBool::new(false);
static BENCHMARK_RUNNING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn run_otp_benchmark(
    app: AppHandle,
    iterations: usize,
) -> Result<BenchmarkResult, String> {
    if BENCHMARK_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Benchmark is already running".to_string());
    }

    BENCHMARK_CANCELLED.store(false, Ordering::SeqCst);

    let completed = Arc::new(AtomicUsize::new(0));
    let cancelled = Arc::new(AtomicBool::new(false));
    let app_clone = app.clone();
    let completed_clone = completed.clone();
    let cancelled_clone = cancelled.clone();

    let progress_handle = tauri::async_runtime::spawn(async move {
        let start = Instant::now();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            if cancelled_clone.load(Ordering::SeqCst) {
                break;
            }

            let current = completed_clone.load(Ordering::SeqCst);
            let elapsed_ms = u64::try_from(start.elapsed().as_millis())
                .map_err(|err| err.to_string())
                .unwrap_or(0);
            let _ = app_clone.emit(
                "benchmark-progress",
                BenchmarkProgress {
                    current,
                    total: iterations,
                    elapsed_ms,
                },
            );

            if current >= iterations {
                break;
            }
        }
    });

    let overall_start = Instant::now();

    let result: Result<Vec<Duration>, String> = tokio::task::spawn_blocking({
        let completed = completed.clone();
        move || {
            let times: Result<Vec<Duration>, String> = (0..iterations)
                .into_par_iter()
                .map(|_| {
                    if BENCHMARK_CANCELLED.load(Ordering::SeqCst) {
                        return Err("Benchmark cancelled".to_string());
                    }

                    let duration = {
                        let start = Instant::now();

                        let otp = otp::generate_otp().map_err(|e| {
                            BENCHMARK_CANCELLED.store(true, Ordering::SeqCst);
                            format!("OTP generation failed: {:?}", e)
                        })?;

                        let _hash = otp::hash_otp(&otp).map_err(|e| {
                            BENCHMARK_CANCELLED.store(true, Ordering::SeqCst);
                            format!("OTP hashing failed: {:?}", e)
                        })?;

                        start.elapsed()
                    };

                    completed.fetch_add(1, Ordering::SeqCst);
                    Ok(duration)
                })
                .collect();
            times
        }
    })
    .await
    .map_err(|_| "Benchmark thread panicked".to_string())?;

    let total_time = overall_start.elapsed();

    cancelled.store(true, Ordering::SeqCst);
    let _ = progress_handle.await;

    let _ = app.emit(
        "benchmark-progress",
        BenchmarkProgress {
            current: iterations,
            total: iterations,
            elapsed_ms: u64::try_from(total_time.as_millis()).map_err(|err| err.to_string())?,
        },
    );

    BENCHMARK_RUNNING.store(false, Ordering::SeqCst);

    let times = result?;
    let sum: Duration = times.iter().sum();
    let average_per_iteration = sum / u32::try_from(iterations).map_err(|err| err.to_string())?;
    let average_with_threading =
        total_time / u32::try_from(iterations).map_err(|err| err.to_string())?;
    let iterations_per_second = iterations as f64 / total_time.as_secs_f64();

    Ok(BenchmarkResult {
        iterations,
        average_per_iteration_ms: average_per_iteration.as_secs_f64() * 1000.0,
        average_with_threading_ms: average_with_threading.as_secs_f64() * 1000.0,
        total_ms: total_time.as_secs_f64() * 1000.0,
        iterations_per_second,
    })
}

#[tauri::command]
pub fn cancel_benchmark() -> Result<(), String> {
    if !BENCHMARK_RUNNING.load(Ordering::SeqCst) {
        return Err("No benchmark is running".to_string());
    }
    BENCHMARK_CANCELLED.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn is_benchmark_running() -> bool {
    BENCHMARK_RUNNING.load(Ordering::SeqCst)
}
