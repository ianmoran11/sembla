extern "C" __global__ void sembla_count_deferred(const unsigned char* deferred, unsigned long long candidate_count, unsigned long long table_count, unsigned long long deferred_table, unsigned long long table, unsigned long long* deferred_counts) {
  unsigned long long worker = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
  unsigned long long local = 0ULL;
  for (unsigned long long candidate = worker; candidate < candidate_count; candidate += (unsigned long long)gridDim.x * blockDim.x) local += deferred[candidate * table_count + deferred_table] != 0U;
  extern __shared__ unsigned long long deferred_partials[];
  deferred_partials[threadIdx.x] = local;
  __syncthreads();
  for (unsigned int stride = blockDim.x / 2U; stride != 0U; stride /= 2U) {
    if (threadIdx.x < stride) deferred_partials[threadIdx.x] += deferred_partials[threadIdx.x + stride];
    __syncthreads();
  }
  if (threadIdx.x == 0U) atomicAdd(deferred_counts + table, deferred_partials[0]);
}
