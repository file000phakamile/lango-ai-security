//! Real statistical drift calculations — Population Stability Index and
//! KL-divergence — over two equal-length probability distributions. This is
//! standard, well-defined math (not a heuristic); what's simplified in v0.1
//! is *when* it runs: there's no scheduled job yet, so `seed.rs` computes
//! these once at seed time over the entity-type distribution of each
//! synthetic week vs. a baseline week, rather than a live weekly batch job.
//! See docs/ARCHITECTURE.md.

const EPSILON: f64 = 1e-4;

/// Population Stability Index: sum((actual_i - expected_i) * ln(actual_i / expected_i))
/// over matching bins of two distributions that must each sum to ~1.0.
/// Common interpretation: <0.10 no significant shift, 0.10-0.20 moderate
/// shift, >=0.20 significant shift (the threshold this product alerts on).
pub fn population_stability_index(baseline: &[f64], current: &[f64]) -> f64 {
    assert_eq!(baseline.len(), current.len(), "distributions must be the same length");
    baseline
        .iter()
        .zip(current.iter())
        .map(|(&b, &c)| {
            let b = b.max(EPSILON);
            let c = c.max(EPSILON);
            (c - b) * (c / b).ln()
        })
        .sum()
}

/// KL-divergence: sum(p_i * ln(p_i / q_i)), measuring how much distribution
/// P diverges from reference distribution Q. Not symmetric — order matters.
pub fn kl_divergence(p: &[f64], q: &[f64]) -> f64 {
    assert_eq!(p.len(), q.len(), "distributions must be the same length");
    p.iter()
        .zip(q.iter())
        .map(|(&pi, &qi)| {
            let pi = pi.max(EPSILON);
            let qi = qi.max(EPSILON);
            pi * (pi / qi).ln()
        })
        .sum()
}

/// Normalizes a vector of raw counts into a probability distribution
/// (each bin / total), with every bin floored at a small epsilon so
/// PSI/KL never divide by a true zero.
pub fn normalize_counts(counts: &[u32]) -> Vec<f64> {
    let total: u32 = counts.iter().sum();
    if total == 0 {
        return vec![1.0 / counts.len() as f64; counts.len()];
    }
    counts
        .iter()
        .map(|&c| (c as f64 / total as f64).max(EPSILON))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_distributions_have_zero_drift() {
        let d = vec![0.25, 0.25, 0.25, 0.25];
        assert!(population_stability_index(&d, &d).abs() < 1e-9);
        assert!(kl_divergence(&d, &d).abs() < 1e-9);
    }

    #[test]
    fn shifted_distribution_has_positive_psi() {
        let baseline = vec![0.25, 0.25, 0.25, 0.25];
        let shifted = vec![0.7, 0.1, 0.1, 0.1];
        assert!(population_stability_index(&baseline, &shifted) > 0.20);
    }
}
