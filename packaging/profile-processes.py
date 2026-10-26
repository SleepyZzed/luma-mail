"""Linux process-tree measurements; synthetic UI only, no account or mail data."""
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import threading
import time

phase = ['startup', time.monotonic()]
errors = []
process = subprocess.Popen([sys.argv[1], '--performance-diagnostics'],
                           env={**os.environ, 'APPIMAGE_EXTRACT_AND_RUN': '1'},
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

def readout():
    for line in process.stdout:
        if line.startswith('PERF '):
            phase[:] = [line.strip()[5:], time.monotonic()]
            print(line.strip(), flush=True)

def readerr():
    for line in process.stderr:
        if 'CRITICAL' in line or 'ERROR' in line:
            errors.append(line.strip()[:300])

threading.Thread(target=readout, daemon=True).start()
threading.Thread(target=readerr, daemon=True).start()
previous = {}
samples = []
clock = os.sysconf('SC_CLK_TCK')
anchor = None
begin = time.monotonic()
while process.poll() is None:
    time.sleep(1)
    stats = {}
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            raw = (entry/'stat').read_text()
            fields = raw[raw.rfind(')')+2:].split()
            pid = int(entry.name)
            stats[pid] = (int(fields[1]), int(fields[11])+int(fields[12]), fields[19], entry)
            if anchor is None and Path(os.readlink(entry/'exe')).name == 'luma-mail':
                if b'--performance-diagnostics' in (entry/'cmdline').read_bytes():
                    anchor = pid
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    family = {anchor} if anchor else set()
    for _ in range(12):
        added = {pid for pid, (parent, *_) in stats.items() if parent in family}
        if added <= family:
            break
        family |= added
    pss = rss = private = cpu = count = 0
    current = {}
    now = time.monotonic()
    for pid in family:
        if pid not in stats:
            continue
        _, ticks, birth, entry = stats[pid]
        try:
            values = {k: int(v.split()[0]) for k, v in
                      (line.split(':', 1) for line in (entry/'smaps_rollup').read_text().splitlines()[1:])}
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        count += 1
        pss += values.get('Pss', 0)
        rss += values.get('Rss', 0)
        private += values.get('Private_Clean', 0)+values.get('Private_Dirty', 0)
        current[(pid, birth)] = (ticks, now)
        if (pid, birth) in previous:
            old_ticks, old_time = previous[(pid, birth)]
            cpu += (ticks-old_ticks)/clock/(now-old_time)*100
    previous = current
    if count:
        samples.append(dict(seconds=round(now-begin, 2), phase=phase[0],
            settled=now-phase[1]>=5, processes=count, rss_mib=round(rss/1024, 2),
            pss_mib=round(pss/1024, 2), private_mib=round(private/1024, 2), cpu_one_core_percent=round(cpu, 3)))
    if now-begin > 130:
        process.terminate()
        raise SystemExit('Performance fixture timeout')
code = process.wait(timeout=5)
if code or not samples or errors:
    raise SystemExit(f'Performance fixture failed: exit={code}, samples={len(samples)}, errors={errors}')
report = dict(description='Release AppImage plus all descendant renderer/helper processes. RSS double-counts shared mappings; PSS apportions them. CPU is percentage of one core, sampled once per second. Synthetic fixtures, no live sync/network or real GPU compositor; targets are goals, not enforced caps.',
    machine=dict(kernel=os.uname().release, cpu_count=os.cpu_count()), phases={}, samples=samples)
for name in dict.fromkeys(s['phase'] for s in samples):
    subset = [s for s in samples if s['phase']==name and s['settled']]
    if not subset:
        continue
    report['phases'][name] = {k: round(statistics.mean(s[k] for s in subset), 3)
        for k in ['rss_mib', 'pss_mib', 'private_mib', 'cpu_one_core_percent']}
    report['phases'][name]['peak_rss_mib'] = max(s['rss_mib'] for s in samples if s['phase']==name)
Path(sys.argv[2]).write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
print(json.dumps(report['phases'], indent=2))
required = {'baseline', 'newsletter', 'three_html_messages', 'released', 'reclaimed_background'}
assert required <= report['phases'].keys(), 'All resource lifecycle phases must be measured'
