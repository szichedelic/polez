//! SIMD-friendly DSP primitives.
//!
//! These functions are structured to enable LLVM auto-vectorization on all
//! targets. They process data in chunks of 4 (SSE/NEON width) or 8 (AVX width)
//! with explicit accumulators so the compiler can emit SIMD instructions.

/// Multiply two f32 slices element-wise, writing to `out`.
/// Used for STFT window application.
#[inline]
pub fn multiply_into(a: &[f32], b: &[f32], out: &mut [f32]) {
    let len = a.len().min(b.len()).min(out.len());
    let chunks = len / 4;
    let remainder = len % 4;

    for i in 0..chunks {
        let base = i * 4;
        out[base] = a[base] * b[base];
        out[base + 1] = a[base + 1] * b[base + 1];
        out[base + 2] = a[base + 2] * b[base + 2];
        out[base + 3] = a[base + 3] * b[base + 3];
    }

    let tail = chunks * 4;
    for i in 0..remainder {
        out[tail + i] = a[tail + i] * b[tail + i];
    }
}

/// Compute sum of squares of an f32 slice (for RMS).
/// Uses 4 accumulators to enable vectorized reduction.
#[inline]
pub fn sum_of_squares(data: &[f32]) -> f64 {
    let mut acc0: f64 = 0.0;
    let mut acc1: f64 = 0.0;
    let mut acc2: f64 = 0.0;
    let mut acc3: f64 = 0.0;

    let chunks = data.len() / 4;
    for i in 0..chunks {
        let base = i * 4;
        let v0 = data[base] as f64;
        let v1 = data[base + 1] as f64;
        let v2 = data[base + 2] as f64;
        let v3 = data[base + 3] as f64;
        acc0 += v0 * v0;
        acc1 += v1 * v1;
        acc2 += v2 * v2;
        acc3 += v3 * v3;
    }

    let tail = chunks * 4;
    for &v in &data[tail..] {
        acc0 += (v as f64) * (v as f64);
    }

    acc0 + acc1 + acc2 + acc3
}

/// Find the maximum absolute value in an f32 slice (for peak detection).
#[inline]
pub fn max_abs(data: &[f32]) -> f32 {
    let mut m0: f32 = 0.0;
    let mut m1: f32 = 0.0;
    let mut m2: f32 = 0.0;
    let mut m3: f32 = 0.0;

    let chunks = data.len() / 4;
    for i in 0..chunks {
        let base = i * 4;
        m0 = m0.max(data[base].abs());
        m1 = m1.max(data[base + 1].abs());
        m2 = m2.max(data[base + 2].abs());
        m3 = m3.max(data[base + 3].abs());
    }

    let tail = chunks * 4;
    for &v in &data[tail..] {
        m0 = m0.max(v.abs());
    }

    m0.max(m1).max(m2.max(m3))
}

/// Scale a slice in-place by a constant factor.
#[inline]
pub fn scale_inplace(data: &mut [f32], factor: f32) {
    let chunks = data.len() / 4;
    for i in 0..chunks {
        let base = i * 4;
        data[base] *= factor;
        data[base + 1] *= factor;
        data[base + 2] *= factor;
        data[base + 3] *= factor;
    }

    let tail = chunks * 4;
    for v in &mut data[tail..] {
        *v *= factor;
    }
}

/// Add `src` into `dst` element-wise (overlap-add accumulation).
#[inline]
pub fn add_into(dst: &mut [f32], src: &[f32]) {
    let len = dst.len().min(src.len());
    let chunks = len / 4;

    for i in 0..chunks {
        let base = i * 4;
        dst[base] += src[base];
        dst[base + 1] += src[base + 1];
        dst[base + 2] += src[base + 2];
        dst[base + 3] += src[base + 3];
    }

    let tail = chunks * 4;
    for i in 0..(len - tail) {
        dst[tail + i] += src[tail + i];
    }
}

/// Multiply-accumulate: `dst[i] += a[i] * b[i]` for overlap-add with windowing.
#[inline]
pub fn multiply_accumulate(dst: &mut [f32], a: &[f32], b: &[f32]) {
    let len = dst.len().min(a.len()).min(b.len());
    let chunks = len / 4;

    for i in 0..chunks {
        let base = i * 4;
        dst[base] += a[base] * b[base];
        dst[base + 1] += a[base + 1] * b[base + 1];
        dst[base + 2] += a[base + 2] * b[base + 2];
        dst[base + 3] += a[base + 3] * b[base + 3];
    }

    let tail = chunks * 4;
    for i in 0..(len - tail) {
        dst[tail + i] += a[tail + i] * b[tail + i];
    }
}

/// Squared accumulate: `dst[i] += a[i] * a[i]` (for window_sum in ISTFT).
#[inline]
pub fn square_accumulate(dst: &mut [f32], a: &[f32]) {
    let len = dst.len().min(a.len());
    let chunks = len / 4;

    for i in 0..chunks {
        let base = i * 4;
        dst[base] += a[base] * a[base];
        dst[base + 1] += a[base + 1] * a[base + 1];
        dst[base + 2] += a[base + 2] * a[base + 2];
        dst[base + 3] += a[base + 3] * a[base + 3];
    }

    let tail = chunks * 4;
    for i in 0..(len - tail) {
        dst[tail + i] += a[tail + i] * a[tail + i];
    }
}

/// Divide `data[i] /= divisor[i]` where divisor > threshold, else leave unchanged.
#[inline]
pub fn divide_where_above(data: &mut [f32], divisor: &[f32], threshold: f32) {
    let len = data.len().min(divisor.len());
    for i in 0..len {
        if divisor[i] > threshold {
            data[i] /= divisor[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply_into() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![0.5, 0.5, 0.5, 0.5, 0.5];
        let mut out = vec![0.0; 5];
        multiply_into(&a, &b, &mut out);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[4] - 2.5).abs() < 1e-6);
    }

    #[test]
    fn test_sum_of_squares() {
        let data = vec![3.0, 4.0];
        let result = sum_of_squares(&data);
        assert!((result - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_max_abs() {
        let data = vec![0.1, -0.9, 0.3, 0.5, -0.2];
        assert!((max_abs(&data) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_scale_inplace() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        scale_inplace(&mut data, 2.0);
        assert!((data[0] - 2.0).abs() < 1e-6);
        assert!((data[4] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_add_into() {
        let mut dst = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let src = vec![0.5, 0.5, 0.5, 0.5, 0.5];
        add_into(&mut dst, &src);
        assert!((dst[0] - 1.5).abs() < 1e-6);
        assert!((dst[4] - 5.5).abs() < 1e-6);
    }

    #[test]
    fn test_multiply_accumulate() {
        let mut dst = vec![0.0; 5];
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0, 2.0, 2.0, 2.0, 2.0];
        multiply_accumulate(&mut dst, &a, &b);
        assert!((dst[0] - 2.0).abs() < 1e-6);
        assert!((dst[4] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_square_accumulate() {
        let mut dst = vec![0.0; 3];
        let a = vec![3.0, 4.0, 5.0];
        square_accumulate(&mut dst, &a);
        assert!((dst[0] - 9.0).abs() < 1e-6);
        assert!((dst[1] - 16.0).abs() < 1e-6);
    }

    #[test]
    fn test_divide_where_above() {
        let mut data = vec![10.0, 20.0, 30.0];
        let div = vec![2.0, 0.0, 5.0];
        divide_where_above(&mut data, &div, 1e-10);
        assert!((data[0] - 5.0).abs() < 1e-6);
        assert!((data[1] - 20.0).abs() < 1e-6); // unchanged
        assert!((data[2] - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_empty_inputs() {
        let empty: Vec<f32> = vec![];
        assert!(sum_of_squares(&empty) == 0.0);
        assert!(max_abs(&empty) == 0.0);
        scale_inplace(&mut [], 2.0);
        multiply_into(&[], &[], &mut []);
        add_into(&mut [], &[]);
    }
}
