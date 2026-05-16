//! Per-frame paint→encode→write stage timings for the live sidecar
//! loop, plus exact fixed-window percentile summaries so the main
//! process can read out p50/p95/p99 latencies.
//!
//! Why this lives next to `live.rs`: the sidecar already owns the hot
//! loop. Sampling here costs one `Instant::now()` per stage boundary.
//! Each stage keeps one preallocated window of nanosecond samples and
//! sorts only at the 60-frame summary boundary, so steady-state record
//! cost stays a single push per stage while p95 remains exact enough
//! for 120 fps gates.

use std::time::{Duration, Instant};

/// Per-frame stage timings captured by the live loop.
///
/// `total_ns` is the real wall-clock span from request arrival to the
/// stdout flush returning, so it captures every byte of overhead
/// outside paint/encode/write (snapshot reads, JSON parse, scratch
/// allocations). It is measured at the loop boundary, not summed.
#[derive(Clone, Copy, Debug)]
pub(super) struct FrameStageTimings {
    pub paint_ns: u64,
    pub encode_ns: u64,
    pub write_ns: u64,
    pub total_ns: u64,
}

impl FrameStageTimings {
    pub(super) fn from_durations(
        paint: Duration,
        encode: Duration,
        write: Duration,
        total: Duration,
    ) -> Self {
        Self {
            paint_ns: duration_to_ns(paint),
            encode_ns: duration_to_ns(encode),
            write_ns: duration_to_ns(write),
            total_ns: duration_to_ns(total),
        }
    }
}

fn duration_to_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

/// Saturating elapsed-ns helper. `Instant::elapsed` is monotonic but
/// the cast can still overflow on the (impossible) hour-long frame.
pub(super) fn elapsed_ns(start: Instant) -> u64 {
    duration_to_ns(start.elapsed())
}

#[derive(Debug)]
struct StageSamples {
    values: Vec<u64>,
}

impl StageSamples {
    fn new(window_size: usize) -> Self {
        Self { values: Vec::with_capacity(window_size) }
    }

    fn record(&mut self, ns: u64) {
        self.values.push(ns);
    }

    fn len(&self) -> usize {
        self.values.len()
    }

    fn percentiles_us(&self) -> StagePercentiles {
        let mut sorted = self.values.clone();
        sorted.sort_unstable();
        StagePercentiles {
            p50: percentile_us(&sorted, 0.50),
            p95: percentile_us(&sorted, 0.95),
            p99: percentile_us(&sorted, 0.99),
        }
    }

    fn reset(&mut self) {
        self.values.clear();
    }
}

#[derive(Clone, Copy)]
struct StagePercentiles {
    p50: u64,
    p95: u64,
    p99: u64,
}

fn percentile_us(sorted_ns: &[u64], percentile: f64) -> u64 {
    if sorted_ns.is_empty() {
        return 0;
    }
    let target = ((sorted_ns.len() as f64) * percentile).ceil() as usize;
    let index = target.max(1).min(sorted_ns.len()) - 1;
    ns_to_us_ceil(sorted_ns[index])
}

fn ns_to_us_ceil(ns: u64) -> u64 {
    ns.div_ceil(1_000)
}

/// Aggregates a rolling window of [`FrameStageTimings`] across N
/// frames, exposing one [`FramePerfSummary`] per window flush.
pub(super) struct FramePerfAggregator {
    window_size: usize,
    paint: StageSamples,
    encode: StageSamples,
    write: StageSamples,
    total: StageSamples,
    context_label: &'static str,
}

impl FramePerfAggregator {
    pub(super) const DEFAULT_WINDOW_SIZE: u32 = 60;

    pub(super) fn new(context_label: &'static str, window_size: u32) -> Self {
        let window_size = usize::try_from(window_size.max(1)).unwrap_or(usize::MAX);
        Self {
            window_size,
            paint: StageSamples::new(window_size),
            encode: StageSamples::new(window_size),
            write: StageSamples::new(window_size),
            total: StageSamples::new(window_size),
            context_label,
        }
    }

    pub(super) fn record(&mut self, timings: FrameStageTimings) -> Option<FramePerfSummary> {
        self.paint.record(timings.paint_ns);
        self.encode.record(timings.encode_ns);
        self.write.record(timings.write_ns);
        self.total.record(timings.total_ns);
        if self.paint.len() < self.window_size {
            return None;
        }
        let paint = self.paint.percentiles_us();
        let encode = self.encode.percentiles_us();
        let write = self.write.percentiles_us();
        let total = self.total.percentiles_us();
        let summary = FramePerfSummary {
            window: u32::try_from(self.paint.len()).unwrap_or(u32::MAX),
            context: self.context_label,
            paint_p50_us: paint.p50,
            paint_p95_us: paint.p95,
            paint_p99_us: paint.p99,
            encode_p50_us: encode.p50,
            encode_p95_us: encode.p95,
            encode_p99_us: encode.p99,
            write_p50_us: write.p50,
            write_p95_us: write.p95,
            write_p99_us: write.p99,
            total_p50_us: total.p50,
            total_p95_us: total.p95,
            total_p99_us: total.p99,
        };
        self.paint.reset();
        self.encode.reset();
        self.write.reset();
        self.total.reset();
        Some(summary)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(super) struct FramePerfSummary {
    pub window: u32,
    pub context: &'static str,
    pub paint_p50_us: u64,
    pub paint_p95_us: u64,
    pub paint_p99_us: u64,
    pub encode_p50_us: u64,
    pub encode_p95_us: u64,
    pub encode_p99_us: u64,
    pub write_p50_us: u64,
    pub write_p95_us: u64,
    pub write_p99_us: u64,
    pub total_p50_us: u64,
    pub total_p95_us: u64,
    pub total_p99_us: u64,
}

#[cfg(test)]
mod tests {
    use super::{FramePerfAggregator, FrameStageTimings, percentile_us};
    use std::time::Duration;

    #[test]
    fn percentile_us_uses_nearest_rank_and_ceils_microseconds() {
        let sorted_ns = [1, 1_000, 1_001];
        assert_eq!(percentile_us(&sorted_ns, 0.50), 1);
        assert_eq!(percentile_us(&sorted_ns, 0.95), 2);
        assert_eq!(percentile_us(&sorted_ns, 0.99), 2);
    }

    #[test]
    fn aggregator_emits_summary_after_window_size_records() -> Result<(), &'static str> {
        let mut aggregator =
            FramePerfAggregator::new("software", FramePerfAggregator::DEFAULT_WINDOW_SIZE);
        for index in 0..(FramePerfAggregator::DEFAULT_WINDOW_SIZE - 1) {
            let result = aggregator.record(constant_timing());
            assert!(result.is_none(), "should not flush at frame {index}");
        }
        let summary = aggregator
            .record(constant_timing())
            .ok_or("aggregator must flush at window boundary")?;
        assert_eq!(summary.window, FramePerfAggregator::DEFAULT_WINDOW_SIZE);
        assert_eq!(summary.context, "software");
        Ok(())
    }

    #[test]
    fn aggregator_resets_after_flush_so_next_window_starts_fresh() {
        let mut aggregator = FramePerfAggregator::new("hardware", 2);
        let _ = aggregator.record(constant_timing());
        let summary = aggregator.record(constant_timing());
        assert!(summary.is_some(), "expected first flush");
        let after_flush = aggregator.record(constant_timing());
        assert!(after_flush.is_none(), "aggregator must zero counters after flush");
    }

    #[test]
    fn aggregator_percentiles_track_increasing_paint_durations() -> Result<(), &'static str> {
        let mut aggregator = FramePerfAggregator::new("software", 4);
        let paint_durations_us = [10u64, 100, 1_000, 10_000];
        let mut summary = None;
        for paint_us in paint_durations_us {
            summary = aggregator.record(FrameStageTimings::from_durations(
                Duration::from_micros(paint_us),
                Duration::from_micros(1),
                Duration::from_micros(1),
                Duration::from_micros(paint_us + 2),
            ));
        }
        let summary = summary.ok_or("4-frame window must flush")?;
        assert_eq!(summary.paint_p50_us, 100);
        assert_eq!(summary.paint_p95_us, 10_000);
        assert_eq!(summary.paint_p99_us, 10_000);
        assert_eq!(summary.total_p50_us, 102);
        assert_eq!(summary.total_p95_us, 10_002);
        assert_eq!(summary.total_p99_us, 10_002);
        Ok(())
    }

    fn constant_timing() -> FrameStageTimings {
        FrameStageTimings::from_durations(
            Duration::from_micros(2_000),
            Duration::from_micros(500),
            Duration::from_micros(100),
            Duration::from_micros(2_600),
        )
    }
}
