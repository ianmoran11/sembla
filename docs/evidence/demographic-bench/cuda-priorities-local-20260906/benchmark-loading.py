import hashlib, json, os, platform, statistics, subprocess, time
from pathlib import Path
ROOT=Path('/tmp/sembla-cuda-priorities-20260906')
OUT=ROOT/'local-loading-one-tick'; OUT.mkdir(exist_ok=False)
bins={arm: ROOT/arm/'target/release/sembla' for arm in ('baseline','candidate')}
records=[]
for scale in (1_000_000,10_000_000):
    work=OUT/str(scale);work.mkdir()
    state=work/'initial.state'
    subprocess.run([str(bins['candidate']),'synth-state','--model',str(ROOT/'candidate/fixtures/demographic/benchmark/demographic_slots.full.json'),'--slots',str(scale),'--areas','4','--present-fraction','0.8','--streams','birth:600,overseas:250,internal:150','--seed','9009','--out',str(state)],check=True,stdout=subprocess.DEVNULL)
    digest=hashlib.file_digest(state.open('rb'),'sha256').hexdigest()
    model=Path(str(state)+'.model.json')
    for repeat in range(4):
        results={}
        for arm in (('baseline','candidate') if repeat%2 else ('candidate','baseline')):
            run=work/f'{repeat}-{arm}';run.mkdir(); artifacts=run/'outputs';artifacts.mkdir()
            cmd=[str(bins[arm]),'run',str(model),'--population',str(state),'--seed','9009','--ticks','1','--backend','cpu','--enable','grouped-observations','--out',str(artifacts/'results.csv'),'--lifecycle-timing-json',str(run/'lifecycle.json')]
            (run/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
            started=time.perf_counter()
            with (run/'stdout').open('wb') as stdout,(run/'stderr').open('wb') as stderr:
                p=subprocess.Popen(cmd,stdout=stdout,stderr=stderr)
                _,status,usage=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status)
                assert p.returncode==0,(arm,scale,repeat,(run/'stderr').read_text())
            wall=(time.perf_counter()-started)*1000
            lifecycle=json.loads((run/'lifecycle.json').read_text())
            record=dict(scale=scale,repeat=repeat,warmup=repeat==0,arm=arm,whole_process_ms=wall,peak_rss_bytes=usage.ru_maxrss,**lifecycle)
            records.append(record)
            results[arm]={p.name:p.read_bytes() for p in artifacts.iterdir()}
            (OUT/'raw.json').write_text(json.dumps(records,indent=2)+'\n')
            print(f'scale={scale} repeat={repeat} arm={arm} wall_ms={wall:.1f} peak_rss_MiB={usage.ru_maxrss/2**20:.1f}',flush=True)
        assert results['baseline']==results['candidate'],(scale,repeat,'artifact mismatch')
    assert hashlib.file_digest(state.open('rb'),'sha256').hexdigest()==digest
    (work/'state-sha256.txt').write_text(digest+'\n')
    state.unlink()
summary={'platform':platform.platform(),'cpu':subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip(),'baseline':(ROOT/'baseline-commit').read_text().strip(),'candidate':(ROOT/'candidate-commit').read_text().strip(),'protocol':'CPU run, one tick, grouped observations, warmup excluded, three alternating measured pairs per scale. This measures host loading, not CUDA execution.','all_output_trees_byte_identical':True,'scales':{}}
for scale in (1_000_000,10_000_000):
    arms={}
    for arm in bins:
        rows=[r for r in records if r['scale']==scale and r['arm']==arm and not r['warmup']]
        arms[arm]={'whole_process_ms':statistics.median(r['whole_process_ms'] for r in rows),'peak_rss_bytes':statistics.median(r['peak_rss_bytes'] for r in rows),'phases_ms':{k:statistics.median(r['phases_ms'][k] for r in rows) for k in rows[0]['phases_ms']}}
    summary['scales'][str(scale)]=arms
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
