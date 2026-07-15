// Double-single (df64) arithmetic using a normalized (hi, lo) f32 pair.
//
// These error-free transforms follow Knuth two-sum and Dekker splitting. Their
// ~48-bit result depends on the backend preserving the written f32 operation
// order. The pinned wgpu-hal Metal patch disables fast math at compilation, and
// the host behavior probe rejects contraction, reassociation, or lost residuals.

alias df64 = vec2<f32>;

fn df_from_f32(value: f32) -> df64 {
  return vec2<f32>(value, 0.0);
}

fn df_neg(value: df64) -> df64 {
  return vec2<f32>(-value.x, -value.y);
}

// Knuth two-sum: returns rounded a+b and its exact rounding residual.
fn two_sum(a: f32, b: f32) -> df64 {
  let sum = a + b;
  let virtual_b = sum - a;
  let error = (a - (sum - virtual_b)) + (b - virtual_b);
  return vec2<f32>(sum, error);
}

// Fast renormalization. The caller supplies |a| >= |b|.
fn quick_two_sum(a: f32, b: f32) -> df64 {
  let sum = a + b;
  let error = b - (sum - a);
  return vec2<f32>(sum, error);
}

// FMA-free Dekker product. 4097 = 2^12+1 splits a binary32 mantissa into
// non-overlapping high and low pieces.
fn two_prod(a: f32, b: f32) -> df64 {
  let product = a * b;

  let split_a = 4097.0 * a;
  let a_hi = split_a - (split_a - a);
  let a_lo = a - a_hi;

  let split_b = 4097.0 * b;
  let b_hi = split_b - (split_b - b);
  let b_lo = b - b_hi;

  let error = (((a_hi * b_hi - product) + a_hi * b_lo) + a_lo * b_hi) + a_lo * b_lo;
  return vec2<f32>(product, error);
}

fn df_add(a: df64, b: df64) -> df64 {
  let leading = two_sum(a.x, b.x);
  let residual = (leading.y + a.y) + b.y;
  return quick_two_sum(leading.x, residual);
}

fn df_mul(a: df64, b: df64) -> df64 {
  let leading = two_prod(a.x, b.x);
  let residual = ((leading.y + a.x * b.y) + a.y * b.x) + a.y * b.y;
  return quick_two_sum(leading.x, residual);
}

// One quotient plus a residual correction is sufficient for the precision
// needed by the hazard/race path.
fn df_div(a: df64, b: df64) -> df64 {
  let quotient = a.x / b.x;
  let residual = df_add(a, df_neg(df_mul(b, df_from_f32(quotient))));
  let correction = (residual.x + residual.y) / b.x;
  return quick_two_sum(quotient, correction);
}

// Normalized pairs are non-overlapping, so lexicographic (hi, lo) comparison
// is numeric comparison. This also handles the (+inf, 0) sentinel without the
// NaN that subtraction-based comparison would create.
fn df_less(a: df64, b: df64) -> bool {
  return a.x < b.x || (a.x == b.x && a.y < b.y);
}

fn df_equal(a: df64, b: df64) -> bool {
  return a.x == b.x && a.y == b.y;
}

// log(x) starts from the WGSL f32 logarithm y0. One Newton correction for
// exp(y)-x=0 adds (x-exp(y0))/exp(y0) in df64. The omitted quadratic term is
// O(r^2), around 1e-14 when the f32 intrinsic is within a few ulps.
fn df_log(x: df64) -> df64 {
  let estimate = log(x.x);
  let exp_estimate = exp(estimate);
  let residual = df_div(
    df_add(x, df_neg(df_from_f32(exp_estimate))),
    df_from_f32(exp_estimate),
  );
  return df_add(df_from_f32(estimate), residual);
}
