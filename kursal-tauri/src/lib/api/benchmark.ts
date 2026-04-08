import { invoke } from '@tauri-apps/api/core';

export interface BenchmarkProgress {
    current: number;
    total: number;
    elapsed_ms: number;
}

export interface BenchmarkResult {
    iterations: number;
    average_per_iteration_ms: number;
    average_with_threading_ms: number;
    total_ms: number;
    iterations_per_second: number;
}

export async function runOtpBenchmark(iterations: number): Promise<BenchmarkResult> {
    return invoke('run_otp_benchmark', { iterations });
}

export async function cancelBenchmark(): Promise<void> {
    return invoke('cancel_benchmark');
}

export async function isBenchmarkRunning(): Promise<boolean> {
    return invoke('is_benchmark_running');
}
