import hashlib,json,statistics,sys
from pathlib import Path
root=Path(sys.argv[1])
for manifest in ['SHA256SUMS','SHA256SUMS.remote']:
 n=0
 for line in (root/manifest).read_text().splitlines():
  digest,name=line.split('  ',1);p=root/name
  assert p.resolve().is_relative_to(root.resolve()),name
  assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,(manifest,name)
  n+=1
 print(manifest,'verified',n)

def compare(left,right,normalize=False):
 paths=lambda p: {x.relative_to(p) for x in p.rglob('*') if x.is_file()}
 files=paths(left);assert files==paths(right),(left,right,'file sets')
 for rel in files:
  a,b=(left/rel).read_bytes(),(right/rel).read_bytes()
  if normalize and rel.as_posix()=='run-manifest.json':
   a,b=json.loads(a),json.loads(b)
   for d in (a,b):d['backend_identity']={'kind':'normalized-for-cpu-cuda-parity'}
  assert a==b,(left,right,rel)
 return len(files)
summary={}
for scale in [1000000,10000000]:
 p=root/'sweep'/str(scale)
 comparisons={}
 for backend in ['cpu','cuda']:
  comparisons[backend+'_before_after']=compare(p/('baseline-'+backend),p/('current-'+backend))
  assert (p/('baseline-'+backend+'-pairs.csv')).read_bytes()==(p/('current-'+backend+'-pairs.csv')).read_bytes()
 for arm in ['baseline','current']:
  comparisons[arm+'_cpu_cuda']=compare(p/(arm+'-cpu'),p/(arm+'-cuda'),True)
  assert (p/(arm+'-cpu-pairs.csv')).read_bytes()==(p/(arm+'-cuda-pairs.csv')).read_bytes()
 measurements={}
 for backend in ['cpu','cuda']:
  arms=[]
  for arm in ['baseline','current']:
   label=f'{arm}-{backend}-{scale}'
   d=json.loads((root/'sweep'/(label+'.draw-timing.json')).read_text())
   assert len(d['draw_timings'])==20
   assert all(row['k']==i and row['wall_time_ms']>0 for i,row in enumerate(d['draw_timings']))
   arms.append(d)
   measurements[arm+'_'+backend]={**json.loads((root/'sweep'/(label+'.time.json')).read_text()),'whole_sweep_ms':d['whole_sweep_wall_time_ms'],'first_draw_ms':d['draw_zero_including_setup_wall_time_ms'],'median_later_draw_ms':d['median_later_draw_wall_time_ms']}
  measurements[backend+'_later_paired_ratio_median']=statistics.median(b['wall_time_ms']/a['wall_time_ms'] for a,b in zip(arms[0]['draw_timings'][1:],arms[1]['draw_timings'][1:]))
 summary[str(scale)]={'comparisons':comparisons,'measurements':measurements}
assert 'PASS: comparator rejected' in (root/'sweep/1000000/negative-control.txt').read_text()
runs=root/'runs'
for suffix in ['.csv','.csv.summaries.csv','.stdout']:
 reference=(runs/('cuda-1'+suffix)).read_bytes()
 assert reference
 for arm in ['cpu','cuda']:
  for replicate in range(1,4):
   assert (runs/(f'{arm}-{replicate}'+suffix)).read_bytes()==reference,(arm,replicate,suffix)
print('Frozen gate: results, summaries, and execution hash tuples byte-identical across six runs')
print(json.dumps(summary,indent=2))

assert (root/'repository-commit.txt').read_text().strip() == 'f84bb05b4d7d3c9fb4d6ce829570569ab42ac408'
assert (root/'sweep/baseline-commit.txt').read_text().strip() == '697307b55bb83a08dbc49e362d4c812088360be2'
assert (root/'differential-corpus/exit-code.txt').read_text().strip() == '0'
optimization_log=(root/'differential-corpus/optimization-corpus.log').read_text()
assert 'optimized_reductions_match_cpu_for_group_sizes_and_fused_resets ... ok' in optimization_log
assert '1 passed; 0 failed' in optimization_log
for grouped in ['', 'grouped-']:
 for backend in ['cpu', 'cuda']:
  name=f'lifecycle-{grouped}{backend}.json'
  doc=json.loads((root/'profile'/name).read_text())
  assert doc['schema']=='sembla-run-lifecycle-timing-v1'
  assert doc['backend']==backend
  assert all(v >= 0 for v in doc['phases_ms'].values())
  assert abs(sum(doc['phases_ms'].values())-doc['total_ms']) < 0.001
  construction=doc['cuda_construction_ms']
  if backend=='cuda':
   assert all(v >= 0 for v in construction.values())
   assert abs(sum(v for k,v in construction.items() if k!='total')-construction['total']) < 0.001
   assert doc['nvrtc_cache_hit'] is False
  else:
   assert construction is None
print('Candidate identity, optimization corpus and lifecycle phase reconciliation verified')
profile=root/'profile'
for grouped in ['', 'grouped-']:
 suffixes=['.csv','.csv.summaries.csv','.stdout']
 if grouped:
  suffixes.extend('.grouped.'+name+'.csv' for name in ['deaths_cells','population_cells','vacancy_cells'])
 for suffix in suffixes:
  assert (profile/('profile-'+grouped+'cpu'+suffix)).read_bytes() == (profile/('profile-'+grouped+'cuda'+suffix)).read_bytes(),(grouped,suffix)
print('Grouped and no-grouped profile results, summaries, execution hashes and grouped sidecars match CPU')
if len(sys.argv)>2:
 Path(sys.argv[2]).write_text(json.dumps(summary,indent=2)+'\n')
