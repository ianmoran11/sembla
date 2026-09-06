"""Three paired, warmed CUDA repeats after the frozen collector completes."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import tarfile
import time

home = Path.home()
primary = home / 'demographic-bench'
source_repo = home / 'sembla'
repo = home / 'cuda-throughput-final'
work = home / 'cuda-throughput-warm-repeats'
binaries = {'baseline': home / 'cuda-throughput-baseline-sembla',
            'candidate': repo / 'target/release/sembla'}
commits = {'baseline': '697307b55bb83a08dbc49e362d4c812088360be2',
           'candidate': 'd584369a7b2c850997073ae70dfdda937774885d'}
environment = dict(os.environ)
environment['PATH'] = str(home/'.cargo/bin') + ':/usr/local/cuda/bin:' + environment.get('PATH', '')
environment['LD_LIBRARY_PATH'] = '/usr/local/cuda/lib64:' + environment.get('LD_LIBRARY_PATH', '')


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def compare(left, right):
    files = lambda root: {p.relative_to(root) for p in root.rglob('*') if p.is_file()}
    names = files(left)
    assert names == files(right), (left, right, 'file sets')
    for name in names:
        assert (left/name).read_bytes() == (right/name).read_bytes(), (left, right, name)
    return len(names)


# The original collector must be paused locally before this script is launched.
# A separate bounded local guard resumes that collector even if this call fails.
deadline = time.monotonic() + 600
while True:
    status_file = home / 'bench.status'
    status = status_file.read_text().strip() if status_file.exists() else ''
    if status == 'SEMBLA_BENCH_COMPLETE':
        break
    if 'FAILED' in status or time.monotonic() >= deadline:
        raise SystemExit('Original collector did not complete; no extra measurements run')
    time.sleep(10)

assert not work.exists(), 'repeat evidence already exists; refusing to replace it'
profiled_commit = 'f84bb05b4d7d3c9fb4d6ce829570569ab42ac408'
assert (primary/'repository-commit.txt').read_text().strip() == profiled_commit
assert subprocess.check_output(['git', '-C', str(source_repo), 'rev-parse', 'HEAD'], text=True).strip() == profiled_commit
assert (primary/'sweep/baseline-commit.txt').read_text().strip() == commits['baseline']
work.mkdir()
with (work/'final-build.log').open('wb') as log:
    subprocess.run(['git','-C',str(source_repo),'fetch','origin','codex/cuda-throughput-followup'], stdout=log, stderr=subprocess.STDOUT, check=True, env=environment)
    subprocess.run(['git','-C',str(source_repo),'worktree','add','--detach',str(repo),commits['candidate']], stdout=log, stderr=subprocess.STDOUT, check=True, env=environment)
    subprocess.run(['cargo','build','--locked','--release','-p','sembla-cli','--features','cuda'], cwd=repo, stdout=log, stderr=subprocess.STDOUT, check=True, env=environment)
with (work/'final-corpus.log').open('wb') as log:
    corpus_env = dict(environment, SEMBLA_REQUIRE_CUDA='1', SEMBLA_CUDA_EVIDENCE_DIR=str(work/'differential-corpus'))
    subprocess.run(['bash','crates/sembla-cuda/scripts/run-differential-corpus.sh'], cwd=repo, stdout=log, stderr=subprocess.STDOUT, check=True, env=corpus_env)
    subprocess.run(['cargo','build','--locked','--release','-p','sembla-cli','--features','cuda'], cwd=repo, stdout=log, stderr=subprocess.STDOUT, check=True, env=environment)
binary_hashes = {arm: digest(binary) for arm, binary in binaries.items()}
assert binary_hashes['baseline'] == (primary/'sweep/baseline-binary.sha256').read_text().split()[0]
shutil.copyfile(__file__, work/'protocol.py')
(work/'provenance.json').write_text(json.dumps({
    'commits': commits, 'binary_sha256': binary_hashes,
    'profiled_candidate_commit': profiled_commit,
    'profiled_candidate_binary_sha256': (primary/'sweep/current-binary.sha256').read_text().split()[0],
    'final_change': 'restore late retained-state copy order; generated kernels and execution unchanged',
    'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'protocol': 'one untimed draw per binary then three paired 20-draw sweeps per scale; order alternates',
}, indent=2)+'\n')
schedule = []
summary = {}

for scale in [1000000, 10000000]:
    directory = work/str(scale)
    directory.mkdir()
    state = directory/'initial.state'
    model = directory/'initial.state.model.json'
    synth = [str(binaries['candidate']), 'synth-state', '--model',
             str(repo/'fixtures/demographic/benchmark/demographic_slots.full.json'),
             '--slots', str(scale), '--areas', '4', '--present-fraction', '0.8',
             '--streams', 'birth:600,overseas:250,internal:150', '--seed', '9009', '--out', str(state)]
    with (directory/'synth.stdout').open('wb') as out, (directory/'synth.stderr').open('wb') as err:
        subprocess.run(synth, stdout=out, stderr=err, env=environment, check=True)
    for path, name in [(state, 'state'), (model, 'model')]:
        actual = digest(path)
        assert actual == (primary/f'sweep/{scale}/{name}.sha256').read_text().split()[0], name
        (directory/f'{name}.sha256').write_text(f'{actual}  {path.name}\n')

    if scale == 10000000:
        gate = work/'final-gate'; gate.mkdir()
        gate_model = json.loads((repo/'fixtures/demographic/benchmark/demographic_slots.no-grouped.json').read_text())
        for box in gate_model['boxes']:
            for table in box['tables']:
                if table['name'] in ['person_slot','slot_resource']:
                    table['size_hint'] = scale
        gate_model_path = gate/'no-grouped.json'
        gate_model_path.write_text(json.dumps(gate_model,separators=(',',':'))+'\n')
        expected = next(line.split()[0] for line in (primary/'models.sha256').read_text().splitlines() if line.endswith('no-grouped.json'))
        assert digest(gate_model_path) == expected
        gate_times = {'cpu':[], 'cuda':[]}
        for repetition in range(1,4):
            for backend in ['cuda','cpu']:
                label = f'{backend}-{repetition}'
                command = [str(binaries['candidate']),'run',str(gate_model_path),'--seed','9009','--population',str(state),'--backend',backend,'--ticks','24','--out',str(gate/(label+'.csv'))]
                with (gate/(label+'.stdout')).open('wb') as stdout, (gate/(label+'.stderr')).open('wb') as stderr:
                    subprocess.run(['/usr/bin/time','-f','{"wall_seconds":%e,"peak_rss_kib":%M}','-o',str(gate/(label+'.time.json')),*command],stdout=stdout,stderr=stderr,env=environment,check=True)
                gate_times[backend].append(json.loads((gate/(label+'.time.json')).read_text())['wall_seconds'])
                for suffix in ['.csv','.csv.summaries.csv','.stdout']:
                    assert (gate/(label+suffix)).read_bytes() == (primary/'runs'/('cuda-1'+suffix)).read_bytes(), (label,suffix)
                print('final gate',label,gate_times[backend][-1],flush=True)
        ratio = statistics.median(gate_times['cpu']) / statistics.median(gate_times['cuda'])
        assert ratio >= 3
        (gate/'summary.json').write_text(json.dumps({'candidate_commit':commits['candidate'],'binary_sha256':binary_hashes['candidate'],'model_sha256':expected,'times_seconds':gate_times,'cpu_median_over_cuda_median':ratio,'three_times_gate_passed':True,'scientific_outputs_match_primary_gate':True},indent=2)+'\n')
        # A separate instrumented command checks the corrected phase accounting.
        command = [str(binaries['candidate']),'run',str(gate_model_path),'--seed','9009','--population',str(state),'--backend','cuda','--ticks','24','--out',str(gate/'lifecycle.csv'),'--lifecycle-timing-json',str(gate/'lifecycle.json')]
        with (gate/'lifecycle.stdout').open('wb') as stdout, (gate/'lifecycle.stderr').open('wb') as stderr:
            subprocess.run(command,stdout=stdout,stderr=stderr,env=environment,check=True)
        lifecycle=json.loads((gate/'lifecycle.json').read_text())
        assert abs(sum(lifecycle['phases_ms'].values())-lifecycle['total_ms']) < .001
        construction=lifecycle['cuda_construction_ms']
        assert abs(sum(v for k,v in construction.items() if k!='total')-construction['total']) < .001
        for suffix in ['.csv','.csv.summaries.csv','.stdout']:
            assert (gate/('lifecycle'+suffix)).read_bytes() == (primary/'runs'/('cuda-1'+suffix)).read_bytes()

    def run(arm, label, draws):
        output = directory/label
        pairs = directory/(label+'.pairs.csv')
        command = [str(binaries[arm]), 'sweep', str(model), '--population', str(state),
                   '--seed', '9009', '--draws', str(draws), '--ticks', '24',
                   '--noise', 'independent', '--backend', 'cuda', '--enable', 'grouped-observations',
                   '--export-pairs', str(pairs), '--timing-json', str(directory/(label+'.native-timing.json')),
                   '--out', str(output)]
        entry = {'scale': scale, 'arm': arm, 'label': label,
                 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 'command': command}
        schedule.append(entry)
        (work/'schedule.json').write_text(json.dumps(schedule, indent=2)+'\n')
        with (directory/(label+'.stdout')).open('wb') as out, (directory/(label+'.stderr')).open('wb') as err:
            started = time.perf_counter_ns()
            subprocess.run(['/usr/bin/time', '-f', '{"wall_seconds":%e,"peak_rss_kib":%M}', '-o', str(directory/(label+'.resource.json')), *command], stdout=out, stderr=err, env=environment, check=True)
            elapsed_ms = (time.perf_counter_ns()-started)/1_000_000
        entry['wall_time_ms'] = elapsed_ms
        (work/'schedule.json').write_text(json.dumps(schedule, indent=2)+'\n')
        print(f'{scale} {label}: {elapsed_ms:.3f} ms', flush=True)
        return elapsed_ms

    for arm in binaries:
        run(arm, 'warmup-'+arm, 1)
    compare(directory/'warmup-baseline', directory/'warmup-candidate')
    times = {arm: [] for arm in binaries}
    verified = []
    for repetition in range(1, 4):
        order = ['baseline', 'candidate'] if repetition % 2 else ['candidate', 'baseline']
        for arm in order:
            label = f'{arm}-{repetition}'
            times[arm].append(run(arm, label, 20))
            count = compare(primary/f'sweep/{scale}/baseline-cuda', directory/label)
            assert (primary/f'sweep/{scale}/baseline-cuda-pairs.csv').read_bytes() == (directory/(label+'.pairs.csv')).read_bytes()
            verified.append({'label': label, 'byte_identical_files': count, 'pairs_equal': True})
    baseline = statistics.median(times['baseline'])
    candidate = statistics.median(times['candidate'])
    summary[str(scale)] = {'whole_process_ms': times, 'median_baseline_ms': baseline,
                           'median_candidate_ms': candidate,
                           'median_wall_reduction_percent': 100*(1-candidate/baseline),
                           'paired_wall_reduction_percent': [100*(1-c/b) for b,c in zip(times['baseline'],times['candidate'])],
                           'verified_against_primary_baseline_cuda': verified}
    (work/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    state.unlink()
    model.unlink()

(work/'README.md').write_text('''# Warmed CUDA repeats

Additive measurements on the same approved H100, after the unchanged primary
corpus, profiles, adjacent sweep pairs and frozen gate completed. The local
collector was paused with a bounded automatic-resume guard during collection.

Each binary gets a one-draw warmup at each scale before three paired 20-draw,
24-tick sweeps. Pair order alternates baseline/candidate, candidate/baseline,
baseline/candidate. Both arms use the native timing option. Reported whole
process intervals use Python perf_counter_ns around subprocess.run; they include
process startup and exit (including the external time wrapper), exclude state
synthesis, and retain native phase data plus peak RSS from /usr/bin/time.
The baseline binary and regenerated inputs are SHA256-matched to the primary run.
The final candidate restores the original late retained-state copy order after
the primary run exposed temporary upload-buffer overlap. It is built in a
separate clean checkout, runs the full hardware corpus and repeats the unchanged
three CPU/CUDA gate pairs at 10M/24 ticks. All six gate outputs match the primary
gate; the final candidate binary hash and commit are recorded in provenance.json.
The primary checkout and its original artifacts remain unchanged.
Every timed tree and pair export must match the primary baseline CUDA artifacts
byte-for-byte, including the full manifest. No provenance fields are normalized.

These three repeats characterize this workload on this host, not a general
confidence interval over all models or hardware. protocol.py reproduces the
procedure; schedule.json records command order and timing, and summary.json
contains all intervals and median reductions.
''')
files = sorted(p for p in work.rglob('*') if p.is_file())
(work/'SHA256SUMS').write_text(''.join(f'{digest(p)}  {p.relative_to(work).as_posix()}\n' for p in files))
with tarfile.open(home/'cuda-throughput-warm-repeats.tar.gz', 'w:gz') as archive:
    archive.add(work, arcname=work.name)
print(json.dumps(summary, indent=2), flush=True)
