"""Independent guard: bound the collector pause even if coordination dies."""
import json
import os
from pathlib import Path
import signal
import subprocess
import time

pause = Path('/tmp/cuda-throughput-collector-paused.json')
done = Path('/tmp/cuda-throughput-repeat-coordinator.done')
deadline = time.time() + 7500
print('Resume guard ready', flush=True)
while time.time() < deadline:
    if done.exists():
        print(done.read_text().strip(), flush=True)
        break
    if pause.exists():
        record = json.loads(pause.read_text())
        if time.time() >= record['paused_epoch'] + 900:
            result = subprocess.run(['ps', '-p', str(record['pid']), '-o', 'command='], capture_output=True, text=True)
            if result.returncode == 0 and result.stdout.strip() == record['command']:
                os.kill(record['pid'], signal.SIGCONT)
                print('15-minute bound reached: collector resumed', flush=True)
            break
    time.sleep(10)
