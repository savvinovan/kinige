//! A request body read with a clock on it (S4c-2, the S4c-1 carry-over). The body limits of S4c-1 bound how much a client may send, not
//! how slowly: a client that sends a byte a minute held a connection and a pool slot for as long as it liked. Two rules cut it off:
//! no chunk may take longer than `body_idle_ms`, and after `body_rate_grace_ms` the average rate may not fall below `body_min_rate`
//! bytes a second. Used by the push body and the blob upload; both answer `408` with the bytes kept so far (a blob upload resumes).

use crate::config::Config;
use axum::body::{Body, Bytes};
use http_body_util::BodyExt;
use std::time::Duration;
use tokio::time::{Instant, timeout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyError {
    /// Too slow: an idle gap or an average rate under the minimum.
    Timeout,
    /// The connection or the framing failed.
    Broken,
}

pub struct GuardedBody {
    body: Body,
    idle: Duration,
    grace: Duration,
    min_rate: u64,
    started: Instant,
    received: u64,
}

impl GuardedBody {
    pub fn new(body: Body, config: &Config) -> GuardedBody {
        GuardedBody {
            body,
            idle: Duration::from_millis(config.body_idle_ms),
            grace: Duration::from_millis(config.body_rate_grace_ms),
            min_rate: config.body_min_rate,
            started: Instant::now(),
            received: 0,
        }
    }

    /// The next non-empty chunk, `None` at the end of the body.
    pub async fn next_chunk(&mut self) -> Result<Option<Bytes>, BodyError> {
        loop {
            let frame = timeout(self.idle, self.body.frame())
                .await
                .map_err(|_| BodyError::Timeout)?;
            let frame = match frame {
                None => return Ok(None),
                Some(Err(_)) => return Err(BodyError::Broken),
                Some(Ok(frame)) => frame,
            };
            let Ok(data) = frame.into_data() else {
                continue;
            };
            if data.is_empty() {
                continue;
            }
            self.received = self
                .received
                .saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
            self.check_rate()?;
            return Ok(Some(data));
        }
    }

    fn check_rate(&self) -> Result<(), BodyError> {
        let elapsed = self.started.elapsed();
        if elapsed <= self.grace {
            return Ok(());
        }
        let millis = u64::try_from(elapsed.as_millis())
            .unwrap_or(u64::MAX)
            .max(1);
        let rate = self.received.saturating_mul(1000) / millis;
        if rate < self.min_rate {
            Err(BodyError::Timeout)
        } else {
            Ok(())
        }
    }
}
