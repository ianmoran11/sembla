// Minimal CUDA double-precision reference for the precision spike.
// Compiled only when --features cuda is set and build.rs finds nvcc. No
// --use_fast_math is permitted; build.rs also disables FMA contraction.

#include <cuda_runtime.h>
#include <math.h>
#include <stddef.h>
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
constexpr uint32_t kThreads = 256u;

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

struct TickConfig {
  uint32_t rows;
  uint32_t groups;
  uint64_t seed;
  double beta;
  double dt;
  uint32_t tick;
};

struct DeviceBuffers {
  uint32_t* offsets;
  uint32_t* employers;
  uint32_t* health;
  uint32_t* winners;
  uint32_t* fired;
  double* weights;
  double* partials;
  double* sums;
  double* races;
};

cudaError_t allocate_and_upload(
    const TickConfig& config, const uint32_t* host_offsets,
    const uint32_t* host_employers, const uint32_t* host_health,
    const double* host_weights, DeviceBuffers* buffers) {
  cudaError_t error = cudaSuccess;
  if (config.rows == 0u || config.groups == 0u || config.groups > config.rows ||
      config.groups > UINT32_MAX / 2u || host_offsets == nullptr ||
      host_employers == nullptr || host_health == nullptr ||
      host_weights == nullptr || buffers == nullptr) {
    return cudaErrorInvalidValue;
  }
#define CUDA_ALLOC(pointer, bytes) do { \
  if (error == cudaSuccess) { \
    error = cudaMalloc(reinterpret_cast<void**>(&(pointer)), (bytes)); \
  } \
} while (0)
  CUDA_ALLOC(buffers->offsets,
             (static_cast<size_t>(config.groups) + 1u) * sizeof(uint32_t));
  CUDA_ALLOC(buffers->employers,
             static_cast<size_t>(config.rows) * sizeof(uint32_t));
  CUDA_ALLOC(buffers->health,
             static_cast<size_t>(config.rows) * sizeof(uint32_t));
  CUDA_ALLOC(buffers->weights,
             static_cast<size_t>(config.rows) * sizeof(double));
  CUDA_ALLOC(buffers->partials,
             static_cast<size_t>(config.groups) * 2u * sizeof(double));
  CUDA_ALLOC(buffers->sums,
             static_cast<size_t>(config.groups) * sizeof(double));
  CUDA_ALLOC(buffers->races,
             static_cast<size_t>(config.rows) * sizeof(double));
  CUDA_ALLOC(buffers->winners,
             static_cast<size_t>(config.groups) * sizeof(uint32_t));
  CUDA_ALLOC(buffers->fired,
             static_cast<size_t>(config.rows) * sizeof(uint32_t));
#undef CUDA_ALLOC
#define CUDA_COPY(destination, source, bytes) do { \
  if (error == cudaSuccess) { \
    error = cudaMemcpy((destination), (source), (bytes), cudaMemcpyHostToDevice); \
  } \
} while (0)
  CUDA_COPY(buffers->offsets, host_offsets,
            (static_cast<size_t>(config.groups) + 1u) * sizeof(uint32_t));
  CUDA_COPY(buffers->employers, host_employers,
            static_cast<size_t>(config.rows) * sizeof(uint32_t));
  CUDA_COPY(buffers->health, host_health,
            static_cast<size_t>(config.rows) * sizeof(uint32_t));
  CUDA_COPY(buffers->weights, host_weights,
            static_cast<size_t>(config.rows) * sizeof(double));
#undef CUDA_COPY
  return error;
}

cudaError_t release_buffers(DeviceBuffers* buffers) {
  if (buffers == nullptr) return cudaSuccess;
  cudaError_t first = cudaSuccess;
#define CUDA_FREE(pointer) do { \
  if ((pointer) != nullptr) { \
    const cudaError_t free_error = cudaFree(pointer); \
    if (first == cudaSuccess && free_error != cudaSuccess) first = free_error; \
    (pointer) = nullptr; \
  } \
} while (0)
  CUDA_FREE(buffers->fired);
  CUDA_FREE(buffers->winners);
  CUDA_FREE(buffers->races);
  CUDA_FREE(buffers->sums);
  CUDA_FREE(buffers->partials);
  CUDA_FREE(buffers->weights);
  CUDA_FREE(buffers->health);
  CUDA_FREE(buffers->employers);
  CUDA_FREE(buffers->offsets);
#undef CUDA_FREE
  return first;
}

cudaError_t launch_reduction(const TickConfig& config,
                             const DeviceBuffers& buffers) {
  reduce_partial_kernel<<<(config.groups * 2u + kThreads - 1u) / kThreads,
                            kThreads>>>(config.groups, buffers.offsets,
                                       buffers.health, buffers.weights,
                                       buffers.partials);
  cudaError_t error = cudaGetLastError();
  if (error != cudaSuccess) return error;
  reduce_finish_kernel<<<(config.groups + kThreads - 1u) / kThreads,
                           kThreads>>>(config.groups, buffers.partials,
                                      buffers.sums);
  return cudaGetLastError();
}

cudaError_t launch_map(const TickConfig& config,
                       const DeviceBuffers& buffers) {
  map_kernel<<<(config.rows + kThreads - 1u) / kThreads, kThreads>>>(
      config.rows, config.tick, static_cast<uint32_t>(config.seed),
      static_cast<uint32_t>(config.seed >> 32), config.beta, config.dt,
      buffers.offsets, buffers.employers, buffers.health, buffers.sums,
      buffers.races, buffers.fired);
  return cudaGetLastError();
}

cudaError_t launch_argmin(const TickConfig& config,
                          const DeviceBuffers& buffers) {
  argmin_kernel<<<(config.groups + kThreads - 1u) / kThreads, kThreads>>>(
      config.groups, config.dt, buffers.offsets, buffers.races,
      buffers.winners, buffers.fired);
  return cudaGetLastError();
}

cudaError_t launch_tick(const TickConfig& config,
                        const DeviceBuffers& buffers) {
  cudaError_t error = launch_reduction(config, buffers);
  if (error == cudaSuccess) error = launch_map(config, buffers);
  if (error == cudaSuccess) error = launch_argmin(config, buffers);
  return error;
}

cudaError_t copy_outputs(const TickConfig& config,
                         const DeviceBuffers& buffers, double* host_sums,
                         uint32_t* host_winners, uint32_t* host_fired) {
  if (host_sums == nullptr || host_winners == nullptr || host_fired == nullptr) {
    return cudaErrorInvalidValue;
  }
  cudaError_t error = cudaMemcpy(
      host_sums, buffers.sums,
      static_cast<size_t>(config.groups) * sizeof(double),
      cudaMemcpyDeviceToHost);
  if (error == cudaSuccess) {
    error = cudaMemcpy(host_winners, buffers.winners,
                       static_cast<size_t>(config.groups) * sizeof(uint32_t),
                       cudaMemcpyDeviceToHost);
  }
  if (error == cudaSuccess) {
    error = cudaMemcpy(host_fired, buffers.fired,
                       static_cast<size_t>(config.rows) * sizeof(uint32_t),
                       cudaMemcpyDeviceToHost);
  }
  return error;
}

cudaError_t destroy_events(cudaEvent_t events[4]) {
  cudaError_t first = cudaSuccess;
  for (uint32_t index = 0; index < 4u; ++index) {
    if (events[index] != nullptr) {
      const cudaError_t error = cudaEventDestroy(events[index]);
      if (first == cudaSuccess && error != cudaSuccess) first = error;
      events[index] = nullptr;
    }
  }
  return first;
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
  const TickConfig config{rows, groups, seed, beta, dt, tick};
  DeviceBuffers buffers{};
  cudaError_t error = allocate_and_upload(config, host_offsets, host_employers,
                                           host_health, host_weights, &buffers);
  if (error == cudaSuccess) error = launch_tick(config, buffers);
  if (error == cudaSuccess) error = cudaDeviceSynchronize();
  if (error == cudaSuccess) {
    error = copy_outputs(config, buffers, host_sums, host_winners, host_fired);
  }
  const cudaError_t cleanup_error = release_buffers(&buffers);
  if (error == cudaSuccess) error = cleanup_error;
  return static_cast<int>(error);
}

extern "C" int sembla_cuda_f64_benchmark(
    uint32_t rows, uint32_t groups, uint64_t seed, double beta, double dt,
    uint32_t tick, uint32_t warmup_ticks, uint32_t measured_ticks,
    const uint32_t* host_offsets, const uint32_t* host_employers,
    const uint32_t* host_health, const double* host_weights,
    float* host_total_ms, float* host_reduce_ms, float* host_argmin_ms,
    double* host_sums, uint32_t* host_winners, uint32_t* host_fired) {
  if (warmup_ticks != 10u || measured_ticks != 100u ||
      host_total_ms == nullptr || host_reduce_ms == nullptr ||
      host_argmin_ms == nullptr) {
    return static_cast<int>(cudaErrorInvalidValue);
  }

  const TickConfig config{rows, groups, seed, beta, dt, tick};
  DeviceBuffers buffers{};
  cudaEvent_t events[4] = {nullptr, nullptr, nullptr, nullptr};
  cudaError_t error = allocate_and_upload(config, host_offsets, host_employers,
                                           host_health, host_weights, &buffers);
  for (uint32_t index = 0; index < 4u && error == cudaSuccess; ++index) {
    error = cudaEventCreate(&events[index]);
  }

  // Immutable workload inputs and device allocations are retained across all
  // warmup and measured ticks. A single synchronization drains the warmups.
  for (uint32_t sample = 0; sample < warmup_ticks && error == cudaSuccess;
       ++sample) {
    error = launch_tick(config, buffers);
  }
  if (error == cudaSuccess) error = cudaDeviceSynchronize();

  for (uint32_t sample = 0; sample < measured_ticks && error == cudaSuccess;
       ++sample) {
    error = cudaEventRecord(events[0]);
    if (error == cudaSuccess) error = launch_reduction(config, buffers);
    if (error == cudaSuccess) error = cudaEventRecord(events[1]);
    if (error == cudaSuccess) error = launch_map(config, buffers);
    if (error == cudaSuccess) error = cudaEventRecord(events[2]);
    if (error == cudaSuccess) error = launch_argmin(config, buffers);
    if (error == cudaSuccess) error = cudaEventRecord(events[3]);
    if (error == cudaSuccess) error = cudaEventSynchronize(events[3]);
    if (error == cudaSuccess) {
      error = cudaEventElapsedTime(&host_total_ms[sample], events[0], events[3]);
    }
    if (error == cudaSuccess) {
      error = cudaEventElapsedTime(&host_reduce_ms[sample], events[0], events[1]);
    }
    if (error == cudaSuccess) {
      error = cudaEventElapsedTime(&host_argmin_ms[sample], events[2], events[3]);
    }
  }

  if (error == cudaSuccess) {
    error = copy_outputs(config, buffers, host_sums, host_winners, host_fired);
  }
  const cudaError_t event_cleanup_error = destroy_events(events);
  const cudaError_t buffer_cleanup_error = release_buffers(&buffers);
  if (error == cudaSuccess) error = event_cleanup_error;
  if (error == cudaSuccess) error = buffer_cleanup_error;
  return static_cast<int>(error);
}
