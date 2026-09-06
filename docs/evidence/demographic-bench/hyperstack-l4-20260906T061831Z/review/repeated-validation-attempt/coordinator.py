"""Bounded additive measurements before the existing collector tears down."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tarfile
import time

PID = 97373
SSH = '/tmp/cuda-throughput-ssh'
PAUSE = Path('/tmp/cuda-throughput-collector-paused.json')
DONE = Path('/tmp/cuda-throughput-repeat-coordinator.done')
DEST = Path('/tmp/cuda-throughput-extra-evidence')


def collector_command():
    return subprocess.check_output(['ps', '-p', str(PID), '-o', 'command='], text=True).strip()


def log(message):
    print(datetime.datetime.now(datetime.timezone.utc).isoformat(), message, flush=True)


assert not PAUSE.exists() and not DONE.exists() and not DEST.exists()
assert collector_command() == 'bash run-demographic-benchmark.sh'
with open('/tmp/cuda-throughput-warm-repeats.py', 'rb') as script:
    subprocess.run([SSH, 'cat > ~/cuda-throughput-warm-repeats.py'], stdin=script, check=True, timeout=30)
log('Protocol uploaded; waiting for the final ageing gate replicate')
deadline = time.monotonic() + 6300
while time.monotonic() < deadline:
    result = subprocess.run([SSH, 'if test -f ~/demographic-bench/runs/ageing-full-3.time.json; then echo READY; else cat ~/bench.status 2>/dev/null || true; fi'], capture_output=True, text=True, timeout=30)
    if result.returncode == 0 and result.stdout.strip() == 'READY':
        break
    if 'FAILED' in result.stdout or 'COMPLETE' in result.stdout:
        raise SystemExit('Primary completed or failed before coordination; collector left alone')
    time.sleep(30)
else:
    raise SystemExit('Primary stage wait expired; collector left alone')

assert collector_command() == 'bash run-demographic-benchmark.sh'
DEST.mkdir()
PAUSE.write_text(json.dumps({'pid': PID, 'paused_epoch': time.time(), 'command': collector_command()})+'\n')
os.kill(PID, signal.SIGSTOP)
log('Collector paused; independent guard will resume it within 15 minutes')
try:
    with (DEST/'remote-run.log').open('wb') as output:
        subprocess.run([SSH, 'timeout --kill-after=30 840 python3 ~/cuda-throughput-warm-repeats.py'], stdout=output, stderr=subprocess.STDOUT, check=True, timeout=880)
    archive = DEST/'warm-repeats.tar.gz'
    with archive.open('wb') as output:
        subprocess.run([SSH, 'cat ~/cuda-throughput-warm-repeats.tar.gz'], stdout=output, check=True, timeout=45)
    with tarfile.open(archive) as bundle:
        bundle.extractall(DEST, filter='data')
    root = DEST/'cuda-throughput-warm-repeats'
    count = 0
    for line in (root/'SHA256SUMS').read_text().splitlines():
        digest, name = line.split('  ', 1)
        path = root/name
        assert path.resolve().is_relative_to(root.resolve())
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, name
        count += 1
    log(f'Extra evidence retrieved and {count} checksums verified')
    DONE.write_text('success\n')
finally:
    if collector_command() == 'bash run-demographic-benchmark.sh':
        os.kill(PID, signal.SIGCONT)
        log('Collector resumed for primary transfer and automatic VM teardown')
    if not DONE.exists():
        DONE.write_text('failed; collector resumed\n')
