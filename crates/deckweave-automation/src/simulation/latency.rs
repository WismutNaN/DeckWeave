// SPDX-License-Identifier: GPL-3.0-or-later
//! Small percentile summary used as a repeatable baseline, not a hard SLA.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyPercentiles {
    pub samples: usize,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
}

impl LatencyPercentiles {
    pub(super) fn from_samples(mut samples: Vec<u64>) -> Self {
        samples.sort_unstable();
        Self {
            samples: samples.len(),
            p50: percentile(&samples, 50),
            p95: percentile(&samples, 95),
            p99: percentile(&samples, 99),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationLatency {
    pub ingress_to_dispatch_ns: LatencyPercentiles,
    pub queue_wait_ns: LatencyPercentiles,
    pub scheduler_step_ns: LatencyPercentiles,
    pub provider_completion_ticks: LatencyPercentiles,
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let rank = (samples.len() - 1).saturating_mul(percentile) / 100;
    samples[rank]
}
