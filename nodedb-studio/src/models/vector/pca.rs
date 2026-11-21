//! Two-dimensional projection in plain Rust: a seeded record sample, then
//! principal component analysis by power iteration.
//!
//! PCA is linear. It keeps the two directions of greatest variance and drops
//! the rest, so distances in the plane are not distances between the
//! original vectors. The screen says so.

/// The algorithm the `Projection` records, so a stored result names the code
/// that made it.
pub const PCA_VERSION: &str = "pca-power-iteration/1";

/// Power iterations stop at this many even when they have not settled.
const MAX_ITERATIONS: usize = 60;
/// Two successive directions this close (1 minus |cos|) count as settled.
const SETTLED: f64 = 1e-7;

/// One step of splitmix64. Cheap, seedable, and the same on every platform.
fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Why a projection has no output.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PcaError {
    #[error("PCA needs at least {needed} vectors. The sample holds {got}.")]
    TooFewVectors { needed: usize, got: usize },
    #[error("A vector has {got} values. The first one has {expected}.")]
    RaggedInput { expected: usize, got: usize },
    #[error("The projection was canceled.")]
    Canceled,
}

/// The sample ids for one projection: `forced` first, then a seeded random
/// choice from `1..=population`, `k` ids in all, ascending.
///
/// The same population, `k`, seed, and forced list always give the same ids.
/// Forced ids outside the population are ignored, and the sample is never
/// shorter than asked unless the population is.
pub fn sample_ids(population: u64, k: usize, seed: u64, forced: &[u64]) -> Vec<u64> {
    let k = k.min(usize::try_from(population).unwrap_or(usize::MAX));
    let mut chosen: std::collections::HashSet<u64> = std::collections::HashSet::with_capacity(k);
    for id in forced {
        if (1..=population).contains(id) && chosen.len() < k {
            chosen.insert(*id);
        }
    }
    // Rejection draws: k is at most the resource cap, so the loop is short
    // even when the sample is the whole population.
    let mut state = seed ^ 0xD1B5_4A32_D192_ED03;
    while chosen.len() < k {
        chosen.insert(1 + splitmix(&mut state) % population);
    }
    let mut chosen: Vec<u64> = chosen.into_iter().collect();
    chosen.sort_unstable();
    chosen
}

/// The two leading components of a sample.
#[derive(Debug, Clone, PartialEq)]
pub struct Pca {
    /// One `(x, y)` per input vector, scaled so the larger extent is 1.
    pub scores: Vec<[f32; 2]>,
    /// Share of the sample's total variance each axis keeps.
    pub explained: [f32; 2],
}

fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum()
}

fn normalize(v: &mut [f64]) -> f64 {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    norm
}

/// `Xᵀ(X v)` for centered rows `data` of width `dim`.
fn gram_times(data: &[f32], dim: usize, v: &[f64], out: &mut [f64]) {
    let v32: Vec<f32> = v.iter().map(|x| *x as f32).collect();
    out.fill(0.0);
    for row in data.chunks_exact(dim) {
        let s = dot(row, &v32);
        for (o, x) in out.iter_mut().zip(row) {
            *o += s * f64::from(*x);
        }
    }
}

/// Project `vectors` (one row per record, `dim` values each, flat) onto
/// their two leading principal components.
///
/// `seed` starts the power iteration, so a run repeats exactly. `canceled`
/// is polled between iterations: when it returns true the run stops with
/// `PcaError::Canceled` and no output.
pub fn project_2d(
    vectors: &[f32],
    dim: usize,
    seed: u64,
    canceled: &dyn Fn() -> bool,
) -> Result<Pca, PcaError> {
    let n = vectors.len().checked_div(dim).unwrap_or(0);
    if n < 3 {
        return Err(PcaError::TooFewVectors { needed: 3, got: n });
    }
    if !vectors.len().is_multiple_of(dim) {
        return Err(PcaError::RaggedInput {
            expected: dim,
            got: vectors.len() % dim,
        });
    }

    // Center on the mean.
    let mut mean = vec![0.0f64; dim];
    for row in vectors.chunks_exact(dim) {
        for (m, x) in mean.iter_mut().zip(row) {
            *m += f64::from(*x);
        }
    }
    for m in &mut mean {
        *m /= n as f64;
    }
    let mut data: Vec<f32> = Vec::with_capacity(vectors.len());
    for row in vectors.chunks_exact(dim) {
        data.extend(
            row.iter()
                .zip(&mean)
                .map(|(x, m)| (f64::from(*x) - m) as f32),
        );
    }
    let total: f64 = data
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        / (n as f64 - 1.0);

    let mut state = seed ^ 0x2545_F491_4F6C_DD1D;
    let mut axes: Vec<Vec<f64>> = Vec::with_capacity(2);
    let mut variance = [0.0f64; 2];
    let mut work = vec![0.0f64; dim];
    for var in &mut variance {
        let mut v: Vec<f64> = (0..dim)
            .map(|_| (splitmix(&mut state) >> 11) as f64 / (1u64 << 53) as f64 - 0.5)
            .collect();
        normalize(&mut v);
        for _ in 0..MAX_ITERATIONS {
            if canceled() {
                return Err(PcaError::Canceled);
            }
            gram_times(&data, dim, &v, &mut work);
            // Deflate: keep the new direction orthogonal to the ones found.
            for prev in &axes {
                let d: f64 = work.iter().zip(prev).map(|(a, b)| a * b).sum();
                for (w, p) in work.iter_mut().zip(prev) {
                    *w -= d * p;
                }
            }
            let norm = normalize(&mut work);
            if norm == 0.0 {
                break;
            }
            let cos: f64 = work.iter().zip(&v).map(|(a, b)| a * b).sum();
            v.copy_from_slice(&work);
            if 1.0 - cos.abs() < SETTLED {
                break;
            }
        }
        // The sign of an axis is arbitrary. Fix it: the largest loading is positive.
        let big = v
            .iter()
            .copied()
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(0.0);
        if big < 0.0 {
            for x in &mut v {
                *x = -*x;
            }
        }
        let axis32: Vec<f32> = v.iter().map(|x| *x as f32).collect();
        let scores_sq: f64 = data
            .chunks_exact(dim)
            .map(|r| dot(r, &axis32).powi(2))
            .sum();
        *var = scores_sq / (n as f64 - 1.0);
        axes.push(v);
    }

    let a0: Vec<f32> = axes[0].iter().map(|x| *x as f32).collect();
    let a1: Vec<f32> = axes[1].iter().map(|x| *x as f32).collect();
    let mut scores: Vec<[f32; 2]> = data
        .chunks_exact(dim)
        .map(|r| [dot(r, &a0) as f32, dot(r, &a1) as f32])
        .collect();
    // One scale for both axes keeps the shape the variance gave it.
    let extent = scores
        .iter()
        .flat_map(|s| s.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    if extent > 0.0 {
        for s in &mut scores {
            s[0] /= extent;
            s[1] /= extent;
        }
    }
    let share = |v: f64| if total > 0.0 { (v / total) as f32 } else { 0.0 };
    Ok(Pca {
        scores,
        explained: [share(variance[0]), share(variance[1])],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never() -> bool {
        false
    }

    /// Rows with large variance along dimension 0, less along 1, almost none after.
    fn stretched(n: usize, dim: usize, seed: u64) -> Vec<f32> {
        let mut state = seed;
        let mut out = Vec::with_capacity(n * dim);
        for _ in 0..n {
            for k in 0..dim {
                let u = (splitmix(&mut state) >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
                let scale = match k {
                    0 => 10.0,
                    1 => 4.0,
                    _ => 0.2,
                };
                out.push((u * scale) as f32);
            }
        }
        out
    }

    fn correlation(a: &[f64], b: &[f64]) -> f64 {
        let n = a.len() as f64;
        let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
        let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
        let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
        let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
        cov / (va * vb).sqrt()
    }

    #[test]
    fn sample_has_the_requested_size_and_distinct_ids_in_range() {
        let ids = sample_ids(1_000, 200, 42, &[]);
        assert_eq!(ids.len(), 200);
        let mut dedup = ids.clone();
        dedup.dedup();
        assert_eq!(dedup.len(), 200);
        assert!(ids.iter().all(|i| (1..=1_000).contains(i)));
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn sample_repeats_for_one_seed_and_changes_for_another() {
        assert_eq!(
            sample_ids(10_000, 50, 7, &[]),
            sample_ids(10_000, 50, 7, &[])
        );
        assert_ne!(
            sample_ids(10_000, 50, 7, &[]),
            sample_ids(10_000, 50, 8, &[])
        );
    }

    #[test]
    fn forced_ids_stay_in_and_the_size_holds() {
        let ids = sample_ids(1_108_492, 100, 42, &[103, 5]);
        assert_eq!(ids.len(), 100);
        assert!(ids.contains(&103) && ids.contains(&5));
    }

    #[test]
    fn a_sample_as_large_as_the_population_is_the_population() {
        let ids = sample_ids(20, 20, 3, &[4]);
        assert_eq!(ids, (1..=20).collect::<Vec<u64>>());
        assert_eq!(sample_ids(20, 50, 3, &[]).len(), 20);
    }

    #[test]
    fn forced_ids_outside_the_population_are_ignored() {
        let ids = sample_ids(10, 5, 1, &[0, 11, 3]);
        assert_eq!(ids.len(), 5);
        assert!(ids.contains(&3));
    }

    #[test]
    fn the_first_axis_finds_the_widest_direction() {
        let data = stretched(400, 16, 5);
        let pca = project_2d(&data, 16, 1, &never).expect("projects");
        assert_eq!(pca.scores.len(), 400);
        assert!(pca.explained[0] > pca.explained[1]);
        assert!(pca.explained[0] > 0.7, "got {:?}", pca.explained);
        // The x score follows dimension 0 closely.
        let col: Vec<f64> = data.chunks(16).map(|r| f64::from(r[0])).collect();
        let xs: Vec<f64> = pca.scores.iter().map(|s| f64::from(s[0])).collect();
        assert!(correlation(&col, &xs).abs() > 0.99);
        let max0 = pca.scores.iter().map(|s| s[0].abs()).fold(0.0, f32::max);
        assert!((max0 - 1.0).abs() < 1e-5, "larger extent scales to 1");
    }

    #[test]
    fn a_run_repeats_for_one_seed() {
        let data = stretched(200, 12, 9);
        let a = project_2d(&data, 12, 42, &never).expect("a");
        let b = project_2d(&data, 12, 42, &never).expect("b");
        assert_eq!(a, b);
    }

    #[test]
    fn scores_have_zero_mean_on_each_axis() {
        let data = stretched(300, 10, 2);
        let pca = project_2d(&data, 10, 4, &never).expect("projects");
        for axis in 0..2 {
            let mean: f32 = pca.scores.iter().map(|s| s[axis]).sum::<f32>() / 300.0;
            assert!(mean.abs() < 1e-3, "axis {axis} mean {mean}");
        }
    }

    #[test]
    fn axes_are_uncorrelated() {
        let data = stretched(500, 10, 11);
        let pca = project_2d(&data, 10, 4, &never).expect("projects");
        let cross: f32 = pca.scores.iter().map(|s| s[0] * s[1]).sum::<f32>() / 500.0;
        let var0: f32 = pca.scores.iter().map(|s| s[0] * s[0]).sum::<f32>() / 500.0;
        assert!(cross.abs() < 0.05 * var0.max(1e-6), "cross {cross}");
    }

    #[test]
    fn fewer_than_three_vectors_is_an_error() {
        let err = project_2d(&[1.0, 2.0, 3.0, 4.0], 2, 1, &never).expect_err("too few");
        assert_eq!(err, PcaError::TooFewVectors { needed: 3, got: 2 });
    }

    #[test]
    fn a_ragged_input_is_an_error() {
        let err = project_2d(&[1.0; 10], 3, 1, &never).expect_err("ragged");
        assert!(matches!(err, PcaError::RaggedInput { .. }));
    }

    #[test]
    fn a_canceled_run_returns_no_output() {
        let data = stretched(100, 8, 3);
        let err = project_2d(&data, 8, 1, &|| true).expect_err("canceled");
        assert_eq!(err, PcaError::Canceled);
    }
}
