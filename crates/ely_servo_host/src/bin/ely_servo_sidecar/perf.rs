//! Per-frame paint→encode→write stage timings for the live sidecar
//! loop, plus a tiny fixed-bucket histogram that aggregates the last N
//! frames so the main process can read out p50/p95/p99 latencies.
//!
//! Why this lives next to `live.rs`: the sidecar already owns the hot
//! loop. Sampling here costs one `Instant::now()` per stage boundary
//! (single rdtsc-ish syscall) and adds no allocations on the steady
//! state path. The aggregator carries fixed-size arrays — emitting a
//! summary is a constant-time walk over `BUCKET_COUNT` buckets per
//! stage.
//!
//! The buckets cover the physical range of a sidecar frame: 1 µs up
//! to ~262 ms, in power-of-2 µs steps. Bucket 0 is an underflow
//! sentinel for sub-microsecond samples, bucket `BUCKET_COUNT - 1` is
//! an overflow sentinel for anything past the top edge. The size is
//! chosen to fit the problem rather than the integer width — a 64-bit
//! log2 layout would leave ~40 dead buckets above 100 ms.

use std::time::{Duration, Instant};

/// 1 underflow + 18 doublings from 1 µs to 262 144 µs + 1 overflow.
/// Top edge sits at ~262 ms, two orders of magnitude past a 60 fps
/// budget, which is enough headroom for a stalled frame without
/// wasting buckets on hours-long outliers.
const BUCKET_COUNT: usize = 20;

/// Number of doubling buckets above the underflow sentinel. Bucket
/// `i` for `i` in `1..=DOUBLING_BUCKETS` covers `[2^(i-1), 2^i)` µs.
const DOUBLING_BUCKETS: usize = 18;

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

/// Maps an observed nanosecond count to a bucket index. Bucket 0 is
/// the `<1 µs` underflow sentinel; bucket `BUCKET_COUNT - 1` catches
/// any sample past the top doubling edge.
fn bucket_for(ns: u64) -> usize {
    if ns < 1_000 {
        return 0;
    }
    let us = ns / 1_000;
    // `us >= 1` here, so `64 - leading_zeros` is the position of the
    // top set bit (1-indexed). That index doubles as the bucket
    // number for `[2^(i-1), 2^i) µs`.
    let bucket = 64 - us.leading_zeros() as usize;
    bucket.min(BUCKET_COUNT - 1)
}

/// Returns a representative microsecond value for a bucket. For
/// doubling buckets that's the geometric midpoint `1.5 * 2^(i-1)`;
/// underflow reports 0 µs (which is honest — samples here are
/// genuinely sub-microsecond), and overflow reports the lower edge of
/// the overflow band.
fn bucket_midpoint_us(bucket: usize) -> u64 {
    if bucket == 0 {
        return 0;
    }
    if bucket >= BUCKET_COUNT - 1 {
        // Overflow band starts at `2^DOUBLING_BUCKETS` µs.
        return 1u64 << DOUBLING_BUCKETS;
    }
    let low_us = 1u64 << (bucket - 1);
    let high_us = 1u64 << bucket;
    (low_us + high_us) / 2
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
    use super::{
        BUCKET_COUNT, DOUBLING_BUCKETS, FramePerfAggregator, FrameStageTimings, bucket_for,
        bucket_midpoint_us,
    };
    use std::time::Duration;

    #[test]
    fn bucket_for_routes_sub_microsecond_samples_to_underflow() {
        assert_eq!(bucket_for(0), 0);
        assert_eq!(bucket_for(1), 0);
        assert_eq!(bucket_for(999), 0);
    }

    #[test]
    fn bucket_for_walks_doublings_from_one_microsecond() {
        assert_eq!(bucket_for(1_000), 1);
        assert_eq!(bucket_for(1_999), 1);
        assert_eq!(bucket_for(2_000), 2);
        assert_eq!(bucket_for(3_999), 2);
        assert_eq!(bucket_for(4_000), 3);
    }

    #[test]
    fn bucket_for_saturates_above_top_edge() {
        let top_edge_us = 1u64 << DOUBLING_BUCKETS;
        let beyond_ns = (top_edge_us + 1) * 1_000;
        assert_eq!(bucket_for(beyond_ns), BUCKET_COUNT - 1);
        assert_eq!(bucket_for(u64::MAX), BUCKET_COUNT - 1);
    }

    #[test]
    fn bucket_midpoint_is_monotonic_increasing() {
        let mut last = 0;
        for bucket in 1..BUCKET_COUNT {
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
