extern "C" __global__ void sembla_count_fired(const unsigned char* wins, const unsigned long long* candidate_offsets, unsigned long long candidate_count, unsigned long long rule_count, unsigned long long rule, unsigned long long* fired_counts, unsigned int* effect_active) {
  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
  unsigned long long begin = candidate_offsets[rule];
  unsigned long long end = rule + 1ULL < rule_count ? candidate_offsets[rule + 1ULL] : candidate_count;
  unsigned long long local = 0ULL;
  for (unsigned long long candidate = begin + worker; candidate < end; candidate += (unsigned long long)gridDim.x * blockDim.x)
    local += wins[candidate] != 0U;
  extern __shared__ unsigned long long partials[];
  partials[threadIdx.x] = local;
  __syncthreads();
  for (unsigned int stride = blockDim.x / 2U; stride != 0U; stride /= 2U) {
    if (threadIdx.x < stride) partials[threadIdx.x] += partials[threadIdx.x + stride];
    __syncthreads();
  }
  if (threadIdx.x == 0U && partials[0] != 0ULL) {
    atomicAdd(fired_counts + rule, partials[0]);
    atomicOr(effect_active + rule, 1U);
  }
}
