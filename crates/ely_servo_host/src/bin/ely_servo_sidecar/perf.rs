//! Per-frame paint→encode→write stage timings for the live sidecar
//! loop, plus a tiny fixed-bucket histogram that aggregates the last N
//! frames so the main process can read out p50/p95/p99 latencies.
//!
//! Why this lives next to `live.rs`: the sidecar already owns the hot
//! loop. Sampling here costs one `Instant::now()` per stage boundary
//! (single rdtsc-ish syscall) and adds no allocations on the steady
//! state path. The aggregator carries fixed-size arrays — emitting a
//! summary is a constant-time walk over 64 buckets per stage.
//!
//! The buckets are log2-spaced from 1 µs up to ~17 s
//! (`1 << 64` ns / 1000). Every observation falls into exactly one
//! bucket; the percentile pass is linear in `BUCKET_COUNT` and walks
//! the running cumulative count until it crosses the requested
//! threshold. Linear interpolation inside a bucket gives a closer
//! number than "the bucket's lower bound" without bringing in a real
//! histogram crate. Karpathy heuristic: don't add a dep when 100 lines
//! of straight Rust covers the use case.

use std::time::{Duration, Instant};

const BUCKET_COUNT: usize = 64;

/// Per-frame stage timings captured by the live loop.
///
/// `total_ns` is recorded explicitly rather than summed so we keep
/// any per-frame overhead outside the three measured stages (e.g.
/// snapshot reads, has-visible-content checks) accounted for.
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

#[derive(Clone, Copy, Debug)]
struct StageHistogram {
    buckets: [u32; BUCKET_COUNT],
    count: u32,
}

impl StageHistogram {
    const fn new() -> Self {
        Self { buckets: [0; BUCKET_COUNT], count: 0 }
    }

    fn record(&mut self, ns: u64) {
        let bucket = bucket_for(ns);
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        self.count = self.count.saturating_add(1);
    }

    fn percentile_us(&self, percentile: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let target = ((self.count as f64) * percentile).ceil() as u32;
        let target = target.max(1).min(self.count);
        let mut running: u32 = 0;
        for (bucket, count) in self.buckets.iter().enumerate() {
            running = running.saturating_add(*count);
            if running >= target {
                return bucket_midpoint_us(bucket);
            }
        }
        bucket_midpoint_us(BUCKET_COUNT - 1)
    }

    fn reset(&mut self) {
        self.buckets = [0; BUCKET_COUNT];
        self.count = 0;
    }
}

fn bucket_for(ns: u64) -> usize {
    if ns == 0 {
        return 0;
    }
    let log = 64 - ns.leading_zeros() as usize;
    log.min(BUCKET_COUNT - 1)
}

fn bucket_midpoint_us(bucket: usize) -> u64 {
    if bucket == 0 {
        return 0;
    }
    let low_ns = 1u64.checked_shl((bucket - 1) as u32).unwrap_or(u64::MAX);
    let high_ns = 1u64.checked_shl(bucket as u32).unwrap_or(u64::MAX);
    let midpoint_ns = low_ns.saturating_add(high_ns) / 2;
    midpoint_ns / 1_000
}

/// Aggregates a rolling window of [`FrameStageTimings`] across N
/// frames, exposing one [`FramePerfSummary`] per window flush.
pub(super) struct FramePerfAggregator {
    window_size: u32,
    paint: StageHistogram,
    encode: StageHistogram,
    write: StageHistogram,
    total: StageHistogram,
    frames_in_window: u32,
    context_label: &'static str,
}

impl FramePerfAggregator {
    pub(super) const DEFAULT_WINDOW_SIZE: u32 = 60;

    pub(super) fn new(context_label: &'static str, window_size: u32) -> Self {
        Self {
            window_size: window_size.max(1),
            paint: StageHistogram::new(),
            encode: StageHistogram::new(),
            write: StageHistogram::new(),
            total: StageHistogram::new(),
            frames_in_window: 0,
            context_label,
        }
    }

    pub(super) fn record(&mut self, timings: FrameStageTimings) -> Option<FramePerfSummary> {
        self.paint.record(timings.paint_ns);
        self.encode.record(timings.encode_ns);
        self.write.record(timings.write_ns);
        self.total.record(timings.total_ns);
        self.frames_in_window = self.frames_in_window.saturating_add(1);
        if self.frames_in_window < self.window_size {
            return None;
        }
        let summary = FramePerfSummary {
            window: self.frames_in_window,
            context: self.context_label,
            paint_p50_us: self.paint.percentile_us(0.50),
            paint_p95_us: self.paint.percentile_us(0.95),
            paint_p99_us: self.paint.percentile_us(0.99),
            encode_p50_us: self.encode.percentile_us(0.50),
            encode_p95_us: self.encode.percentile_us(0.95),
            encode_p99_us: self.encode.percentile_us(0.99),
            write_p50_us: self.write.percentile_us(0.50),
            write_p95_us: self.write.percentile_us(0.95),
            write_p99_us: self.write.percentile_us(0.99),
            total_p50_us: self.total.percentile_us(0.50),
            total_p95_us: self.total.percentile_us(0.95),
            total_p99_us: self.total.percentile_us(0.99),
        };
        self.paint.reset();
        self.encode.reset();
        self.write.reset();
        self.total.reset();
        self.frames_in_window = 0;
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
    use super::{FramePerfAggregator, FrameStageTimings, bucket_for, bucket_midpoint_us};
    use std::time::Duration;

    #[test]
    fn bucket_for_handles_zero_and_small_values() {
        assert_eq!(bucket_for(0), 0);
        assert_eq!(bucket_for(1), 1);
        assert_eq!(bucket_for(2), 2);
        assert_eq!(bucket_for(3), 2);
        assert_eq!(bucket_for(4), 3);
    }

    #[test]
    fn bucket_midpoint_is_monotonic_increasing() {
        let mut last = 0;
        for bucket in 1..64 {
            let value = bucket_midpoint_us(bucket);
            assert!(value >= last, "bucket {bucket} midpoint regressed");
            last = value;
        }
    }

    #[test]
    fn aggregator_emits_summary_after_window_size_records() {
        let mut aggregator =
            FramePerfAggregator::new("software", FramePerfAggregator::DEFAULT_WINDOW_SIZE);
        for index in 0..(FramePerfAggregator::DEFAULT_WINDOW_SIZE - 1) {
            let result = aggregator.record(constant_timing());
            assert!(result.is_none(), "should not flush at frame {index}");
        }
        let summary = aggregator.record(constant_timing());
        let summary = summary.expect("aggregator must flush at window boundary");
        assert_eq!(summary.window, FramePerfAggregator::DEFAULT_WINDOW_SIZE);
        assert_eq!(summary.context, "software");
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
    fn aggregator_percentiles_track_increasing_paint_durations() {
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
        let summary = summary.expect("4-frame window must flush");
        assert!(
            summary.paint_p50_us < summary.paint_p99_us,
            "p99 must dominate p50 for increasing samples: {summary:?}"
        );
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
