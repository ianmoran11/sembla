import hashlib,json,statistics,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve()
expected_candidate=sys.argv[2]
expected_baseline='d93c8a301f1fc6e0e44ec6ed06504c5b45ff363e'
for manifest in ['SHA256SUMS.remote']:
    n=0
    for line in (root/manifest).read_text().splitlines():
        digest,name=line.split('  ',1);p=root/name
        assert p.resolve().is_relative_to(root),name
        assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,(manifest,name)
        n+=1
    print(manifest,'verified',n)
assert (root/'repository-commit.txt').read_text().strip()==expected_candidate
assert (root/'sweep/baseline-commit.txt').read_text().strip()==expected_baseline
assert (root/'differential-corpus/exit-code.txt').read_text().strip()=='0'
assert (root/'differential-corpus/commit.txt').read_text().strip()==expected_candidate
assert not (root/'differential-corpus/worktree-status.txt').read_text().strip()
diagnostic=(root/'differential-corpus/diagnostic-corpus.log').read_text()
assert sum(line.startswith('diagnostic_case=') for line in diagnostic.splitlines())==20
assert sum(line.startswith('recovery_beyond_grid=') for line in diagnostic.splitlines())==4
memcheck=(root/'differential-corpus/memcheck.log').read_text()
assert 'ERROR SUMMARY: 0 errors' in memcheck
assert 'test result: ok. 4 passed; 0 failed;' in memcheck
assert sum(line.startswith('diagnostic_case=') for line in memcheck.splitlines())==20
assert sum(line.startswith('recovery_beyond_grid=') for line in memcheck.splitlines())==4

def compare(left,right,normalize=False):
    paths=lambda p:{x.relative_to(p) for x in p.rglob('*') if x.is_file()}
    files=paths(left);assert files==paths(right),(left,right,'file sets')
    assert files
    for rel in files:
        a,b=(left/rel).read_bytes(),(right/rel).read_bytes()
        if normalize and rel.as_posix()=='run-manifest.json':
            a,b=json.loads(a),json.loads(b)
            for d in (a,b):d['backend_identity']={'kind':'normalized-for-cpu-cuda-parity'}
        assert a==b,(left,right,rel)
    return len(files)

def measurement(p):
    native=json.loads(Path(str(p)+'-native-timing.json').read_text())
    external=json.loads(Path(str(p)+'.draw-timing.json').read_text())
    process=json.loads(Path(str(p)+'.time.json').read_text())
    assert len(native['draw_timings'])==len(external['draw_timings'])==20
    assert all(row['k']==i and row['wall_time_ms']>0 for i,row in enumerate(native['draw_timings']))
    totals={}
    for key in ['pageable_dtoh_host_api_ms','cpu_sha256_ms','final_state_seam_total_ms']:
        totals[key]=sum(row['final_state'][key] for row in native['draw_timings'])
    return {**process,'external_whole_ms':external['whole_sweep_wall_time_ms'],'native_whole_ms':native['whole_sweep_wall_time_ms'],'native_setup_ms':native['setup_wall_time_ms'],'draw_total_ms':sum(row['wall_time_ms'] for row in native['draw_timings']),'outside_backend_construction_and_draws_ms':native['whole_sweep_wall_time_ms']-native['setup_wall_time_ms']-sum(row['wall_time_ms'] for row in native['draw_timings']),'median_later_draw_ms':statistics.median(row['wall_time_ms'] for row in native['draw_timings'][1:]),'final_state_totals':totals}

summary={'candidate':expected_candidate,'baseline':expected_baseline,'scales':{}}
for scale in [1000000,10000000]:
    p=root/'sweep'/str(scale);comparisons={};repeats=[]
    for backend in ['cpu','cuda']:
        comparisons[backend+'_before_after']=compare(p/('baseline-'+backend),p/('current-'+backend))
        assert (p/('baseline-'+backend+'-pairs.csv')).read_bytes()==(p/('current-'+backend+'-pairs.csv')).read_bytes()
    for arm in ['baseline','current']:
        comparisons[arm+'_cpu_cuda']=compare(p/(arm+'-cpu'),p/(arm+'-cuda'),True)
        assert (p/(arm+'-cpu-pairs.csv')).read_bytes()==(p/(arm+'-cuda-pairs.csv')).read_bytes()
    for repeat in range(4):
        arms={}
        for arm in ['baseline','current']:
            prefix=p/'repeats'/f'{repeat}-{arm}'
            comparisons[f'{repeat}-{arm}']=compare(p/(arm+'-cuda'),prefix)
            assert Path(str(prefix)+'-pairs.csv').read_bytes()==(p/(arm+'-cuda-pairs.csv')).read_bytes()
            arms[arm]=measurement(prefix)
        compare(p/'repeats'/f'{repeat}-baseline',p/'repeats'/f'{repeat}-current')
        repeats.append({'repeat':repeat,'warmup':repeat==0,**arms})
    measured=repeats[1:];medians={}
    for arm in ['baseline','current']:
        medians[arm]={key:statistics.median(row[arm][key] for row in measured) for key in ['external_whole_ms','native_whole_ms','native_setup_ms','draw_total_ms','outside_backend_construction_and_draws_ms','median_later_draw_ms','peak_rss_kib']}
    reduction=100*(1-medians['current']['external_whole_ms']/medians['baseline']['external_whole_ms'])
    primary={}
    for arm in ['baseline','current']:
        for backend in ['cpu','cuda']:
            name=f'{arm}-{backend}-{scale}'
            primary[name]=json.loads((root/'sweep'/f'{name}.time.json').read_text())
    summary['scales'][str(scale)]={'comparisons':comparisons,'primary':primary,'repeats':repeats,'medians_excluding_warmup':medians,'median_whole_time_reduction_percent':reduction,'paired_reductions_percent':[100*(1-row['current']['external_whole_ms']/row['baseline']['external_whole_ms']) for row in measured]}
assert 'PASS: comparator rejected' in (root/'sweep/1000000/negative-control.txt').read_text()
runs=root/'runs'
for suffix in ['.csv','.csv.summaries.csv','.stdout']:
    reference=(runs/('cuda-1'+suffix)).read_bytes();assert reference
    for arm in ['cpu','cuda']:
        for replicate in range(1,4):assert (runs/(f'{arm}-{replicate}'+suffix)).read_bytes()==reference,(arm,replicate,suffix)
print('Frozen gate: outputs and hash tuples match across all six runs')
for grouped in ['', 'grouped-']:
    for backend in ['cpu','cuda']:
        doc=json.loads((root/'profile'/f'lifecycle-{grouped}{backend}.json').read_text())
        assert doc['schema']=='sembla-run-lifecycle-timing-v1'
        assert all(v>=0 for v in doc['phases_ms'].values())
        assert abs(sum(doc['phases_ms'].values())-doc['total_ms'])<0.001
if len(sys.argv)>3:
    Path(sys.argv[3]).write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
