use std::{
    collections::BTreeMap,
    fmt::Write,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use faber_runtime::{ExecutionStepResult, TaskGroupResult, TaskResult};

/// Counters behind `GET /metrics`, in the Prometheus text format.
#[derive(Default)]
pub struct Metrics {
    /// `/execute` responses by HTTP status.
    responses: Mutex<BTreeMap<u16, u64>>,
    /// Tasks by outcome.
    tasks: Mutex<BTreeMap<String, u64>>,
    cache_hits: AtomicU64,
    execution_micros: AtomicU64,
    executions: AtomicU64,
}

impl Metrics {
    pub fn record_response(&self, status: u16, elapsed: Duration) {
        if let Ok(mut responses) = self.responses.lock() {
            *responses.entry(status).or_default() += 1;
        }
        let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
        self.execution_micros.fetch_add(micros, Ordering::Relaxed);
        self.executions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_tasks(&self, result: &TaskGroupResult) {
        let Ok(mut tasks) = self.tasks.lock() else {
            return;
        };
        let mut count = |task: &TaskResult| {
            let stats = match task {
                TaskResult::Completed { stats, .. } | TaskResult::Failed { stats, .. } => stats,
            };
            let outcome = serde_json::to_value(&stats.outcome)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| "unknown".to_string());
            *tasks.entry(outcome).or_default() += 1;
        };
        for step in result {
            match step {
                ExecutionStepResult::Single(task) => count(task),
                ExecutionStepResult::Parallel(parallel) => parallel.iter().for_each(&mut count),
            }
        }
    }

    pub fn render(&self, slots_total: usize, slots_in_use: usize, cache_entries: usize) -> String {
        let mut out = String::new();
        fn metric(out: &mut String, name: &str, kind: &str, help: &str) {
            let _ = writeln!(out, "# HELP {name} {help}\n# TYPE {name} {kind}");
        }

        metric(
            &mut out,
            "faber_execute_responses_total",
            "counter",
            "Responses to /execute by HTTP status.",
        );
        if let Ok(responses) = self.responses.lock() {
            for (status, count) in responses.iter() {
                let _ = writeln!(
                    out,
                    "faber_execute_responses_total{{status=\"{status}\"}} {count}"
                );
            }
        }
        metric(
            &mut out,
            "faber_tasks_total",
            "counter",
            "Sandboxed tasks by outcome.",
        );
        if let Ok(tasks) = self.tasks.lock() {
            for (outcome, count) in tasks.iter() {
                let _ = writeln!(out, "faber_tasks_total{{outcome=\"{outcome}\"}} {count}");
            }
        }
        metric(
            &mut out,
            "faber_execute_duration_seconds",
            "summary",
            "Time spent handling /execute requests.",
        );
        let seconds = self.execution_micros.load(Ordering::Relaxed) as f64 / 1e6;
        let _ = writeln!(out, "faber_execute_duration_seconds_sum {seconds}");
        let _ = writeln!(
            out,
            "faber_execute_duration_seconds_count {}",
            self.executions.load(Ordering::Relaxed)
        );
        metric(
            &mut out,
            "faber_execution_slots",
            "gauge",
            "Task slots configured with MAX_CONCURRENCY.",
        );
        let _ = writeln!(out, "faber_execution_slots {slots_total}");
        metric(
            &mut out,
            "faber_execution_slots_in_use",
            "gauge",
            "Task slots held by running requests.",
        );
        let _ = writeln!(out, "faber_execution_slots_in_use {slots_in_use}");
        metric(
            &mut out,
            "faber_cache_hits_total",
            "counter",
            "Requests answered from the result cache.",
        );
        let _ = writeln!(
            out,
            "faber_cache_hits_total {}",
            self.cache_hits.load(Ordering::Relaxed)
        );
        metric(
            &mut out,
            "faber_cache_entries",
            "gauge",
            "Results held in the result cache.",
        );
        let _ = writeln!(out, "faber_cache_entries {cache_entries}");
        out
    }
}
