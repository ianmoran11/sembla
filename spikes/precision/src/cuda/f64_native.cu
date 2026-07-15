// Minimal CUDA double-precision reference for the precision spike.
// Compiled only when --features cuda is set and build.rs finds nvcc. No
// --use_fast_math is permitted; build.rs also disables FMA contraction.

#include <cuda_runtime.h>
#include <math.h>
#include <stdint.h>
#include <string.h>

namespace {

constexpr uint32_t kInfectious = 1u;
constexpr uint32_t kSusceptible = 0u;
constexpr uint32_t kInfectionRule = 0u;
constexpr uint32_t kNoWinner = 0xffffffffu;
constexpr uint32_t kPhiloxM0 = 0xd2511f53u;
constexpr uint32_t kPhiloxM1 = 0xcd9e8d57u;
constexpr uint32_t kPhiloxW0 = 0x9e3779b9u;
constexpr uint32_t kPhiloxW1 = 0xbb67ae85u;

struct Words4 {
  uint32_t x, y, z, w;
};

__device__ Words4 philox4x32_10(uint32_t seed_lo, uint32_t seed_hi,
                                 uint32_t tick, uint32_t rule_id,
                                 uint32_t entity_id, uint32_t draw_idx) {
  Words4 counter{tick, rule_id, entity_id, draw_idx};
  uint32_t key_x = seed_lo;
  uint32_t key_y = seed_hi;
  for (uint32_t round = 0; round < 10; ++round) {
    const uint32_t low0 = kPhiloxM0 * counter.x;
    const uint32_t high0 = __umulhi(kPhiloxM0, counter.x);
    const uint32_t low1 = kPhiloxM1 * counter.z;
    const uint32_t high1 = __umulhi(kPhiloxM1, counter.z);
    counter = Words4{high1 ^ counter.y ^ key_x, low1,
                     high0 ^ counter.w ^ key_y, low0};
    if (round != 9) {
      key_x += kPhiloxW0;
      key_y += kPhiloxW1;
    }
  }
  return counter;
}

__device__ double uniform_open_f64(Words4 words) {
  const uint64_t mantissa = (static_cast<uint64_t>(words.x) << 21) |
                            (static_cast<uint64_t>(words.y) >> 11);
  const double sample = (static_cast<double>(mantissa) + 0.5) *
                        1.1102230246251565e-16;
  return sample == 1.0 ? nextafter(1.0, 0.0) : sample;
}

__device__ bool contested(uint32_t entity) { return entity % 10u == 5u; }

__global__ void reduce_partial_kernel(
    uint32_t groups, const uint32_t* offsets, const uint32_t* health,
    const double* weights, double* partials) {
  const uint32_t partial = blockIdx.x * blockDim.x + threadIdx.x;
  if (partial >= groups * 2u) return;
  const uint32_t group = partial / 2u;
  const uint32_t start = offsets[group];
  const uint32_t end = offsets[group + 1u];
  const uint32_t midpoint = start + (end - start) / 2u;
  const uint32_t begin = partial % 2u == 0u ? start : midpoint;
  const uint32_t finish = partial % 2u == 0u ? midpoint : end;
  double sum = 0.0;
  for (uint32_t row = begin; row < finish; ++row) {
    if (health[row] == kInfectious) sum += weights[row];
  }
  partials[partial] = sum;
}

__global__ void reduce_finish_kernel(uint32_t groups, const double* partials,
                                     double* sums) {
  const uint32_t group = blockIdx.x * blockDim.x + threadIdx.x;
  if (group < groups) sums[group] = partials[group * 2u] + partials[group * 2u + 1u];
}

__global__ void map_kernel(
    uint32_t rows, uint32_t tick, uint32_t seed_lo, uint32_t seed_hi,
    double beta, double dt, const uint32_t* offsets,
    const uint32_t* employers, const uint32_t* health, const double* sums,
    double* races, uint32_t* fired) {
  const uint32_t row = blockIdx.x * blockDim.x + threadIdx.x;
  if (row >= rows) return;
  fired[row] = 0u;
  races[row] = CUDART_INF;
  if (health[row] != kSusceptible) return;
  const uint32_t group = employers[row];
  const double group_size = static_cast<double>(offsets[group + 1u] - offsets[group]);
  const double lambda = beta * sums[group] / group_size;
  if (!(lambda > 0.0)) return;
  const double uniform = uniform_open_f64(
      philox4x32_10(seed_lo, seed_hi, tick, kInfectionRule, row, 0u));
  const double time = -log(1.0 - uniform) / lambda;
  races[row] = time;
  if (time < dt && !contested(row)) fired[row] = 1u;
}

__global__ void argmin_kernel(uint32_t groups, double dt,
                              const uint32_t* offsets, const double* races,
                              uint32_t* winners, uint32_t* fired) {
  const uint32_t group = blockIdx.x * blockDim.x + threadIdx.x;
  if (group >= groups) return;
  uint64_t best_bits = UINT64_MAX;
  uint32_t best_rule = UINT32_MAX;
  uint32_t best_entity = kNoWinner;
  for (uint32_t row = offsets[group]; row < offsets[group + 1u]; ++row) {
    const double time = races[row];
    if (!contested(row) || !(time < dt)) continue;
    const uint64_t bits = static_cast<uint64_t>(__double_as_longlong(time));
    const bool better = bits < best_bits ||
        (bits == best_bits &&
         (kInfectionRule < best_rule ||
          (kInfectionRule == best_rule && row < best_entity)));
    if (better) {
      best_bits = bits;
      best_rule = kInfectionRule;
      best_entity = row;
    }
  }
  winners[group] = best_entity;
  if (best_entity != kNoWinner) fired[best_entity] = 1u;
}

}  // namespace

extern "C" int sembla_cuda_f64_probe(char* name, uint32_t name_capacity,
                                      int* fp32_to_fp64_ratio) {
  int count = 0;
  cudaError_t error = cudaGetDeviceCount(&count);
  if (error != cudaSuccess) return static_cast<int>(error);
  if (count == 0) return static_cast<int>(cudaErrorNoDevice);
  cudaDeviceProp properties{};
  error = cudaGetDeviceProperties(&properties, 0);
  if (error != cudaSuccess) return static_cast<int>(error);
  if (name != nullptr && name_capacity != 0u) {
    strncpy(name, properties.name, name_capacity - 1u);
    name[name_capacity - 1u] = '\0';
  }
  int ratio = 0;
  if (cudaDeviceGetAttribute(&ratio, cudaDevAttrSingleToDoublePrecisionPerfRatio, 0) !=
      cudaSuccess) {
    ratio = 0;
    (void)cudaGetLastError();
  }
  if (fp32_to_fp64_ratio != nullptr) *fp32_to_fp64_ratio = ratio;
  return static_cast<int>(cudaSuccess);
}

extern "C" const char* sembla_cuda_f64_error_string(int code) {
  return cudaGetErrorString(static_cast<cudaError_t>(code));
}

extern "C" int sembla_cuda_f64_run_tick(
    uint32_t rows, uint32_t groups, uint64_t seed, double beta, double dt,
    uint32_t tick, const uint32_t* host_offsets,
    const uint32_t* host_employers, const uint32_t* host_health,
    const double* host_weights, double* host_sums, uint32_t* host_winners,
    uint32_t* host_fired) {
  cudaError_t error = cudaSuccess;
  uint32_t *offsets = nullptr, *employers = nullptr, *health = nullptr;
  uint32_t *winners = nullptr, *fired = nullptr;
  double *weights = nullptr, *partials = nullptr, *sums = nullptr, *races = nullptr;

#define CUDA_TRY(expression) do { error = (expression); if (error != cudaSuccess) goto cleanup; } while (0)
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&offsets), (groups + 1ull) * sizeof(uint32_t)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&employers), rows * sizeof(uint32_t)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&health), rows * sizeof(uint32_t)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&weights), rows * sizeof(double)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&partials), groups * 2ull * sizeof(double)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&sums), groups * sizeof(double)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&races), rows * sizeof(double)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&winners), groups * sizeof(uint32_t)));
  CUDA_TRY(cudaMalloc(reinterpret_cast<void**>(&fired), rows * sizeof(uint32_t)));
  CUDA_TRY(cudaMemcpy(offsets, host_offsets, (groups + 1ull) * sizeof(uint32_t), cudaMemcpyHostToDevice));
  CUDA_TRY(cudaMemcpy(employers, host_employers, rows * sizeof(uint32_t), cudaMemcpyHostToDevice));
  CUDA_TRY(cudaMemcpy(health, host_health, rows * sizeof(uint32_t), cudaMemcpyHostToDevice));
  CUDA_TRY(cudaMemcpy(weights, host_weights, rows * sizeof(double), cudaMemcpyHostToDevice));

  {
    constexpr uint32_t threads = 256u;
    reduce_partial_kernel<<<(groups * 2u + threads - 1u) / threads, threads>>>(
        groups, offsets, health, weights, partials);
    CUDA_TRY(cudaGetLastError());
    reduce_finish_kernel<<<(groups + threads - 1u) / threads, threads>>>(groups, partials, sums);
    CUDA_TRY(cudaGetLastError());
    map_kernel<<<(rows + threads - 1u) / threads, threads>>>(
        rows, tick, static_cast<uint32_t>(seed), static_cast<uint32_t>(seed >> 32),
        beta, dt, offsets, employers, health, sums, races, fired);
    CUDA_TRY(cudaGetLastError());
    argmin_kernel<<<(groups + threads - 1u) / threads, threads>>>(
        groups, dt, offsets, races, winners, fired);
    CUDA_TRY(cudaGetLastError());
  }
  CUDA_TRY(cudaDeviceSynchronize());
  CUDA_TRY(cudaMemcpy(host_sums, sums, groups * sizeof(double), cudaMemcpyDeviceToHost));
  CUDA_TRY(cudaMemcpy(host_winners, winners, groups * sizeof(uint32_t), cudaMemcpyDeviceToHost));
  CUDA_TRY(cudaMemcpy(host_fired, fired, rows * sizeof(uint32_t), cudaMemcpyDeviceToHost));

cleanup:
  if (fired != nullptr) (void)cudaFree(fired);
  if (winners != nullptr) (void)cudaFree(winners);
  if (races != nullptr) (void)cudaFree(races);
  if (sums != nullptr) (void)cudaFree(sums);
  if (partials != nullptr) (void)cudaFree(partials);
  if (weights != nullptr) (void)cudaFree(weights);
  if (health != nullptr) (void)cudaFree(health);
  if (employers != nullptr) (void)cudaFree(employers);
  if (offsets != nullptr) (void)cudaFree(offsets);
#undef CUDA_TRY
  return static_cast<int>(error);
}
