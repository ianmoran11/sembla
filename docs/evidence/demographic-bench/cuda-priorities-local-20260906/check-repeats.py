import os, subprocess, tempfile
from pathlib import Path
source=Path('spikes/precision/infra-hyperstack/run-demographic-benchmark.sh').read_text()
a=source.index('    # Keep original build checkouts alive for native timing');b=source.index('    # The synthesized state must live',a)
block=source[a:b]
with tempfile.TemporaryDirectory(prefix='sembla-repeat-smoke-') as tmp:
 root=Path(tmp)
 for arm in ('baseline','current'):
  binary=root/arm;binary.mkdir()
  script=binary/'sembla';script.write_text('''#!/usr/bin/env python3
import os, pathlib, sys
if len(sys.argv)==1:
 print("--timing-json");raise SystemExit(2)
a=sys.argv[1:]
assert pathlib.Path(a[a.index('--population')+1]).is_file()
assert pathlib.Path(__file__).parent.is_dir()
assert a[a.index('--draws')+1]=='20' and a[a.index('--ticks')+1]=='24'
out=pathlib.Path(a[a.index('--out')+1]);out.mkdir()
(out/'result.csv').write_text('changed' if os.getenv('CORRUPT') and out.name=='1-current' else 'same')
pathlib.Path(a[a.index('--export-pairs')+1]).write_text('pairs')
pathlib.Path(a[a.index('--timing-json')+1]).write_text('{}')
''');script.chmod(0o755)
 for corrupt in (False,True):
  work=root/str(corrupt);work.mkdir();(work/'state').touch();scale=work/'1000000';scale.mkdir()
  for arm in ('baseline','current'):
   out=scale/f'{arm}-cuda';out.mkdir();(out/'result.csv').write_text('same');(scale/f'{arm}-cuda-pairs.csv').write_text('pairs')
  setup='''set -euo pipefail
SWEEP_DIR="$WORK"
sweep_scale=1000000
scale_dir="$WORK/1000000"
sweep_state="$WORK/state"
sweep_model=model.json
SEED=9009
sweep_stamp() { printf '%s\\n' "$1" >> "$WORK/schedule"; }
sweep_measure_observed() { shift 3; "$@"; }
timeout() { shift 3; "$@"; }
'''
  env={**os.environ,'WORK':str(work),'BIN':str(root/'current/sembla'),'BASELINE_BIN':str(root/'baseline/sembla')}
  if corrupt:env['CORRUPT']='1'
  result=subprocess.run(['bash','-c',setup+block],env=env,capture_output=True,text=True)
  labels=(work/'schedule').read_text().splitlines()
  assert (result.returncode!=0)==corrupt,(result.stderr,labels)
  if not corrupt:
   assert [x.split('/')[-1] for x in labels]==['0-current','0-baseline','1-baseline','1-current','2-current','2-baseline','3-baseline','3-current']
  else:assert len(labels)==4
  print(f'corrupted_sidecar={corrupt} arms_run={len(labels)} exit_code={result.returncode}: PASS')
