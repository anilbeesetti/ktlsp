# Administrative merge-guard reference

Data-only handoff reference, not shipped server, build, or required test implementation. Do not execute with stale leases. Read AGENTS and the handoff first; inspect this logic against the next exact integration identities. This version assumes the clean reviewed integration head already contains the leased main and has the exact tested merge tree.

```python
import json, re, subprocess, sys

lease = json.load(open(sys.argv[1]))
repo = 'anilbeesetti/ktlsp'
facts = {}

def run(*args):
    result = subprocess.run(args, cwd=lease['worktree'], capture_output=True, text=True, timeout=60)
    if result.returncode != 0:
        raise RuntimeError(f'{args[0:3]} exit {result.returncode}: {result.stderr[:400]}')
    return result.stdout.strip()

def api(path):
    return json.loads(run('gh', 'api', f'repos/{repo}/{path}'))

def need(ok, message):
    if not ok:
        raise RuntimeError(message)

try:
    pr = api(f'pulls/{lease["pr"]}')
    main = api('git/ref/heads/main')['object']['sha']
    head = api('git/ref/heads/' + pr['head']['ref'])['object']['sha']
    need(main == lease['main'], 'exclusive main lease changed')
    need(head == pr['head']['sha'] == lease['head'], 'reviewed head changed')
    need(pr['base']['sha'] == lease['base'] and pr['base']['ref'] == 'main', 'PR base identity changed')
    need(pr['state'] == 'open' and not pr['draft'] and pr['mergeable'] is True and pr['mergeable_state'] == 'clean', 'PR is not open non-draft clean mergeable')
    need(run('git', 'rev-parse', 'HEAD') == lease['head'], 'local reviewed head changed')
    need(run('git', 'rev-parse', 'HEAD^{tree}') == lease['tree'], 'local reviewed tree changed')
    need(run('git', 'status', '--porcelain') == '', 'local reviewed source not clean')
    need(run('git', 'diff', '--name-only', lease['main'], lease['head']).splitlines() == sorted(lease['scope']), 'source scope mismatch')
    run('git', 'merge-base', '--is-ancestor', lease['main'], lease['head'])
    workflow = lease.get('workflow', '.github/workflows/native.yml')
    workflow_blob = run('git', 'rev-parse', f'{lease["head"]}:{workflow}')
    need(workflow_blob == run('git', 'rev-parse', f'{lease["main"]}:{workflow}'), 'native CI configuration changed')
    branch = api('branches/main')
    need(branch['commit']['sha'] == main, 'main changed during guard')
    protection = branch.get('protection', {})
    required = protection.get('required_status_checks', {}).get('contexts', [])
    need(required == lease['required_contexts'] and branch['protected'] == lease['protected'], 'branch protection/config drift')
    merge = api('git/commits/' + pr['merge_commit_sha'])
    need([p['sha'] for p in merge['parents']] == [main, head], 'synthetic merge parents do not match lease')
    need(merge['tree']['sha'] == lease['tree'], 'synthetic integration tree differs from reviewed tree')
    checks = api('commits/' + head + '/check-runs')['check_runs']
    need(checks and all(c['status'] == 'completed' and c['conclusion'] == 'success' for c in checks), 'integration checks missing or non-success')
    need(set(lease['checks']).issubset({c['name'] for c in checks}), 'expected checks missing')
    statuses = api('commits/' + head + '/status')
    need(all(s['state'] == 'success' for s in statuses['statuses']), 'commit status non-success')
    runs = api('actions/runs?head_sha=' + head + '&event=pull_request&per_page=100')['workflow_runs']
    originals = [r for r in runs if r['path'] == workflow and r['run_attempt'] == 1 and r['status'] == 'completed' and r['conclusion'] == 'success' and any(p['number'] == lease['pr'] and p['head']['sha'] == head and p['base']['sha'] == lease['base'] for p in r['pull_requests'])]
    need(len(originals) == 1, 'one original exact-head/base native CI proof required')
    ci = originals[0]
    jobs = api(f'actions/runs/{ci["id"]}/jobs?per_page=100')['jobs']
    native = [j for j in jobs if j['name'] == 'native']
    need(len(native) == 1 and native[0]['status'] == 'completed' and native[0]['conclusion'] == 'success', 'native job missing or not successful')
    steps = {s['name']: s for s in native[0]['steps']}
    need(all(name in steps and steps[name]['status'] == 'completed' and steps[name]['conclusion'] == 'success' for name in ['Install pinned Rust toolchain', 'Format', 'Clippy', 'Tests']), 'mandatory native steps not all successful')
    need('ubuntu-latest' in native[0]['labels'], 'unexpected native executor')
    log = run('gh', 'run', 'view', str(ci['id']), '--log')
    need(re.search(r'(?m)^native\tRun actions/checkout@v4\t[0-9T:.Z+-]+ ' + re.escape(pr['merge_commit_sha']) + r'$', log) is not None, 'original native checkout does not match synthetic integration SHA')
    need(api('git/ref/heads/main')['object']['sha'] == main and api('git/ref/heads/' + pr['head']['ref'])['object']['sha'] == head, 'remote identity changed during guard')
    facts = {'exit_code': 0, 'pr': lease['pr'], 'remote_main': main, 'remote_head': head, 'pr_base': pr['base']['sha'], 'tree': lease['tree'], 'merge_test_sha': pr['merge_commit_sha'], 'merge_test_parents': [p['sha'] for p in merge['parents']], 'scope': lease['scope'], 'workflow_blob': workflow_blob, 'protected': branch['protected'], 'required_contexts': required, 'checks': [{'name': c['name'], 'status': c['status'], 'conclusion': c['conclusion']} for c in checks], 'original_native_run': ci['id'], 'native_job': native[0]['id'], 'local_reviewed_head_clean': True}
    print(json.dumps(facts, sort_keys=True))
except Exception as error:
    print(json.dumps({'exit_code': 1, 'error': str(error)}))
    sys.exit(1)

```
