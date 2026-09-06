import hashlib,json,os,statistics,subprocess,time
from pathlib import Path
root=Path('/tmp/sembla-cuda-priorities-20260906')
out=root/'cpu-control-followup';out.mkdir(exist_ok=False)
state=root/'local-loading/1000000/initial.state';model=Path(str(state)+'.model.json')
records=[]
for repeat in range(4):
    trees={}
    for arm in (['candidate','baseline'] if repeat%2==0 else ['baseline','candidate']):
        prefix=out/f'{repeat}-{arm}'
        cmd=[str(root/arm/'target/release/sembla'),'sweep',str(model),'--population',str(state),'--seed','9009','--draws','5','--ticks','24','--noise','independent','--backend','cpu','--enable','grouped-observations','--export-pairs',str(prefix)+'-pairs.csv','--timing-json',str(prefix)+'-timing.json','--out',str(prefix)]
        Path(str(prefix)+'-command.json').write_text(json.dumps(cmd,indent=2)+'\n')
        start=time.perf_counter()
        with Path(str(prefix)+'-stdout').open('wb') as so,Path(str(prefix)+'-stderr').open('wb') as se:
            p=subprocess.Popen(cmd,stdout=so,stderr=se)
            _,status,usage=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status);assert p.returncode==0,(arm,repeat)
        native=json.loads(Path(str(prefix)+'-timing.json').read_text())
        row=dict(arm=arm,repeat=repeat,warmup=repeat==0,whole_ms=(time.perf_counter()-start)*1000,peak_rss_bytes=usage.ru_maxrss,native=native)
        records.append(row);(out/'raw.json').write_text(json.dumps(records,indent=2)+'\n');print(arm,repeat,row['whole_ms'],flush=True)
        trees[arm]={p.relative_to(prefix).as_posix():p.read_bytes() for p in prefix.rglob('*') if p.is_file()}
    assert trees['baseline']==trees['candidate']
    assert Path(str(out/f'{repeat}-baseline')+'-pairs.csv').read_bytes()==Path(str(out/f'{repeat}-candidate')+'-pairs.csv').read_bytes()
summary={arm:{'whole_ms':statistics.median(r['whole_ms'] for r in records if r['arm']==arm and not r['warmup']),'draw_total_ms':statistics.median(sum(d['wall_time_ms'] for d in r['native']['draw_timings']) for r in records if r['arm']==arm and not r['warmup'])} for arm in ['baseline','candidate']}
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True)
