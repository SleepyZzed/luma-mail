"""Linux process-tree measurements; reads counters only, never mail or tokens."""
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
diagnostics = []
attached = len(sys.argv) > 1 and sys.argv[1] == '--attach'
if attached:
    if len(sys.argv) != 5:
        raise SystemExit('Usage: profile-processes.py --attach MAIN_PID SECONDS output.json')
    anchor = int(sys.argv[2])
    duration = int(sys.argv[3])
    if not 10 <= duration <= 86400 or Path(os.readlink(f'/proc/{anchor}/exe')).name != 'luma-mail':
        raise SystemExit('Choose the Luma main process and a duration of 10–86400 seconds')
    output = sys.argv[4]
    phase = ['live_sync_and_use', time.monotonic()]
    process = None
else:
    anchor = None
    fluid = '--fluid' in sys.argv[3:]
    duration = 210 if fluid else 150
    output = sys.argv[2]
    arguments = ['--design-preview', '--resource-test'] if fluid else ['--performance-diagnostics']
    process = subprocess.Popen([sys.argv[1], *arguments],
                               env={**os.environ, 'APPIMAGE_EXTRACT_AND_RUN': '1', **({'LUMA_PREVIEW_HTML':'1'} if fluid else {})},
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

def readout():
    for line in process.stdout:
        diagnostics.append(line.strip()[:300])
        del diagnostics[:-30]
        if line.startswith('PERF bound_rows '):
            print(line.strip(), flush=True)
        elif line.startswith('PERF '):
            phase[:] = [line.strip()[5:], time.monotonic()]
            print(line.strip(), flush=True)

def readerr():
    for line in process.stderr:
        diagnostics.append(line.strip()[:300])
        del diagnostics[:-30]
        if 'CRITICAL' in line or 'ERROR' in line:
            errors.append(line.strip()[:300])

if process:
    threading.Thread(target=readout, daemon=True).start()
    threading.Thread(target=readerr, daemon=True).start()
previous = {}
samples = []
clock = os.sysconf('SC_CLK_TCK')
begin = time.monotonic()
while (process.poll() is None if process else Path(f'/proc/{anchor}').exists()):
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
            if anchor is None and Path(os.readlink(entry/'exe')).name in {'luma-mail','luma-mail-fluid'}:
                if (b'--resource-test' if fluid else b'--performance-diagnostics') in (entry/'cmdline').read_bytes():
                    anchor = pid
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            pass
    family = {anchor} if anchor else set()
    for _ in range(12):
        added = {pid for pid, (parent, *_) in stats.items() if parent in family}
        if added <= family:
            break
        family |= added
    pss = rss = private = cpu = count = unreadable_pss = 0
    current = {}
    now = time.monotonic()
    for pid in family:
        if pid not in stats:
            continue
        _, ticks, birth, entry = stats[pid]
        try:
            values = {k: int(v.split()[0]) for k, v in
                      (line.split(':', 1) for line in (entry/'smaps_rollup').read_text().splitlines()[1:])}
        except FileNotFoundError:
            continue
        except (PermissionError, ProcessLookupError):
            # Sandboxed helpers may deny smaps. Keep their RSS and CPU in the
            # totals and explicitly mark the PSS figure incomplete.
            try:
                values = {'Rss': int((entry/'statm').read_text().split()[1])*os.sysconf('SC_PAGE_SIZE')//1024}
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                continue
            unreadable_pss += 1
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
            settled=now-phase[1]>=5, processes=count, unreadable_pss=unreadable_pss, rss_mib=round(rss/1024, 2),
            pss_mib=round(pss/1024, 2), private_mib=round(private/1024, 2), cpu_one_core_percent=round(cpu, 3)))
    if now-begin > duration:
        if attached:
            break
        process.terminate()
        raise SystemExit('Performance fixture timeout')
code = process.wait(timeout=5) if process else 0
if code or not samples or errors:
    raise SystemExit(f'Performance fixture failed: exit={code}, samples={len(samples)}, errors={errors}, recent output={diagnostics}')
report = dict(description='Release AppImage plus all descendant renderer/helper processes. RSS double-counts shared mappings; PSS apportions them. CPU is percentage of one core, sampled once per second. Synthetic fixtures, no live sync/network or real GPU compositor; targets are goals, not enforced caps.',
    machine=dict(kernel=os.uname().release, cpu_count=os.cpu_count()), phases={}, samples=samples)
for name in dict.fromkeys(s['phase'] for s in samples):
    subset = [s for s in samples if s['phase']==name and s['settled']]
    if not subset:
        continue
    report['phases'][name] = {k: round(statistics.mean(s[k] for s in subset), 3)
        for k in ['rss_mib', 'pss_mib', 'private_mib', 'cpu_one_core_percent']}
    report['phases'][name]['peak_rss_mib'] = max(s['rss_mib'] for s in samples if s['phase']==name)
    report['phases'][name]['peak_pss_mib'] = max(s['pss_mib'] for s in samples if s['phase']==name)
    report['phases'][name]['peak_cpu_one_core_percent'] = max(s['cpu_one_core_percent'] for s in samples if s['phase']==name)
    report['phases'][name]['all_samples_cpu_one_core_percent'] = round(statistics.mean(s['cpu_one_core_percent'] for s in samples if s['phase']==name),3)
    report['phases'][name]['pss_complete'] = all(s['unreadable_pss']==0 for s in subset)
if attached:
    report['description'] = 'Read-only attachment to the live Luma main process and descendants. No mail, tokens, titles or arguments recorded. Luma is not stopped or reconfigured. CPU is percentage of one core; RSS includes shared mappings and PSS apportions them.'
Path(output).write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
print(json.dumps(report['phases'], indent=2))
required = {'baseline', 'inbox_1000', 'scrolled_1000', 'newsletter', 'released', 'reclaimed_background'}
if not attached and not fluid:
    required.add('three_html_messages')
assert attached or required <= report['phases'].keys(), 'All resource lifecycle phases must be measured'
