//! Per-stream bandwidth cap ("bandwidth_limit" switch): a token bucket and a
//! body wrapper that paces each chunk of a response through it.

use axum::body::{Body, Bytes};
use futures_core::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::{Instant, Sleep};

/// Default cap when the switch has no saved config.
pub const DEFAULT_MBPS: f64 = 20.0;
const MIN_MBPS: f64 = 0.5;
const MAX_MBPS: f64 = 10_000.0;
/// Large chunks are paced in pieces of at most this size so the output stays smooth.
const MAX_PIECE: usize = 64 * 1024;

/// Bytes per second for a cap in megabits per second (clamped to a sane range).
pub fn bytes_per_second(mbps: f64) -> f64 {
    let mbps = if mbps.is_finite() { mbps } else { DEFAULT_MBPS };
    mbps.clamp(MIN_MBPS, MAX_MBPS) * 1_000_000.0 / 8.0
}

/// Classic token bucket. Tokens are bytes; the bucket holds half a second of
/// traffic so playback can start with a short burst, then settles at `rate`.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    rate: f64,
    capacity: f64,
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(bytes_per_second: f64, now: Instant) -> Self {
        let rate = bytes_per_second.max(1.0);
        let capacity = (rate / 2.0).max(16.0 * 1024.0);
        Self {
            rate,
            capacity,
            tokens: capacity,
            last: now,
        }
    }

    /// Spends `bytes` and returns how long the caller must wait before sending
    /// them so the long-run rate stays at or under the cap.
    pub fn take(&mut self, bytes: usize, now: Instant) -> Duration {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.tokens -= bytes as f64;
        if self.tokens >= 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(-self.tokens / self.rate)
        }
    }
}

/// A byte stream that releases each chunk only when the bucket allows it.
pub struct Throttled<S> {
    inner: S,
    bucket: TokenBucket,
    held: Option<Bytes>,
    rest: Option<Bytes>,
    sleep: Option<Pin<Box<Sleep>>>,
}

impl<S> Throttled<S> {
    pub fn new(inner: S, bytes_per_second: f64) -> Self {
        Self {
            inner,
            bucket: TokenBucket::new(bytes_per_second, Instant::now()),
            held: None,
            rest: None,
            sleep: None,
        }
    }
}

impl<S, E> Throttled<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    /// The next piece to send: leftovers of a split chunk first, then the inner stream.
    fn next_piece(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Bytes, E>>> {
        let chunk = match self.rest.take() {
            Some(rest) => rest,
            None => match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => chunk,
                other => return other,
            },
        };
        if chunk.len() > MAX_PIECE {
            let mut chunk = chunk;
            let piece = chunk.split_to(MAX_PIECE);
            self.rest = Some(chunk);
            Poll::Ready(Some(Ok(piece)))
        } else {
            Poll::Ready(Some(Ok(chunk)))
        }
    }
}

impl<S, E> Stream for Throttled<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    type Item = Result<Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        if let Some(sleep) = this.sleep.as_mut() {
            if sleep.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            this.sleep = None;
            if let Some(chunk) = this.held.take() {
                return Poll::Ready(Some(Ok(chunk)));
            }
        }
        match this.next_piece(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                let wait = this.bucket.take(chunk.len(), Instant::now());
                if wait.is_zero() {
                    return Poll::Ready(Some(Ok(chunk)));
                }
                let mut sleep = Box::pin(tokio::time::sleep(wait));
                if sleep.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(Some(Ok(chunk)));
                }
                this.held = Some(chunk);
                this.sleep = Some(sleep);
                Poll::Pending
            }
            other => other,
        }
    }
}

/// Wraps a response body so it is sent at no more than `bytes_per_second`.
pub fn limit_body(body: Body, bytes_per_second: f64) -> Body {
    Body::from_stream(Throttled::new(body.into_data_stream(), bytes_per_second))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mbps_converts_to_bytes_and_clamps() {
        assert_eq!(bytes_per_second(8.0), 1_000_000.0);
        assert_eq!(bytes_per_second(20.0), 2_500_000.0);
        assert_eq!(bytes_per_second(0.0), 62_500.0);
        assert_eq!(bytes_per_second(f64::NAN), 2_500_000.0);
    }

    #[test]
    fn bucket_allows_a_burst_then_paces_to_the_rate() {
        let start = Instant::now();
        let mut bucket = TokenBucket::new(100_000.0, start);
        // Half a second of burst (50 KB) is free.
        assert_eq!(bucket.take(50_000, start), Duration::ZERO);
        // The next 10 KB must wait 0.1 s.
        assert_eq!(bucket.take(10_000, start), Duration::from_millis(100));
        // After that wait has passed, another 10 KB costs another 0.1 s.
        let later = start + Duration::from_millis(100);
        assert_eq!(bucket.take(10_000, later), Duration::from_millis(100));
        // A long idle refills only up to the burst capacity.
        let idle = later + Duration::from_secs(60);
        assert_eq!(bucket.take(60_000, idle), Duration::from_millis(100));
    }

    struct Chunks(std::vec::IntoIter<Bytes>);

    impl Stream for Chunks {
        type Item = Result<Bytes, std::io::Error>;
        fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.0.next().map(Ok))
        }
    }

    async fn drain<S: Stream + Unpin>(mut stream: S) -> Vec<S::Item> {
        let mut out = Vec::new();
        while let Some(item) = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await
        {
            out.push(item);
        }
        out
    }

    #[tokio::test]
    async fn throttled_stream_takes_as_long_as_the_cap_requires() {
        // 16 chunks of 16 KiB = 256 KiB at 256 KiB/s with a 128 KiB burst: 0.5 s.
        let chunks: Vec<Bytes> = (0..16).map(|_| Bytes::from(vec![7u8; 16 * 1024])).collect();
        let started = Instant::now();
        let out = drain(Throttled::new(Chunks(chunks.into_iter()), 256.0 * 1024.0)).await;
        let elapsed = started.elapsed();
        assert_eq!(out.len(), 16);
        let total: usize = out.iter().map(|chunk| chunk.as_ref().unwrap().len()).sum();
        assert_eq!(total, 256 * 1024);
        assert!(
            elapsed >= Duration::from_millis(490) && elapsed <= Duration::from_millis(1_500),
            "took {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn limited_body_keeps_every_byte() {
        let payload: Vec<u8> = (0..300_000u32).map(|n| (n % 251) as u8).collect();
        let body = limit_body(Body::from(payload.clone()), 300_000.0);
        let started = Instant::now();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        assert_eq!(bytes.as_ref(), payload.as_slice());
        // One 300 KB chunk, paced in 64 KiB pieces after a 150 KB burst: 0.5 s.
        assert!(started.elapsed() >= Duration::from_millis(490));
    }
}
