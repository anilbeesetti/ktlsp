"""PROPOSAL DATA. Future optional preflight/compile-only route; never tests/runs candidates."""
from pathlib import Path, PurePosixPath
import datetime, hashlib, json, os, platform, re, struct, subprocess, sys, tarfile, tempfile, resource, shutil, tomllib

BASE = '941c72efd9896b0847ab342ec0dd22c10a000f3b'
PREFIX = 'preparation/t020-g2-route/v1'
WORKFLOW = '.github/workflows/t020-g2-compile-only.yml'
TOOLCHAIN = '1.99.0-x86_64-unknown-linux-gnu'
TARGET = 'x86_64-unknown-linux-gnu'


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as source:
        for chunk in iter(lambda: source.read(65536), b''):
            digest.update(chunk)
    return digest.hexdigest()


def load(path, limit=262144):
    with Path(path).open('rb') as source:
        raw = source.read(limit + 1)
    if len(raw) > limit:
        raise ValueError('JSON size limit')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique)


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def clean_env(toolchain=None):
    result = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'HOME': os.environ['HOME']}
    if toolchain:
        result['PATH'] = str(toolchain / 'bin') + ':' + result['PATH']
    return result


def text(command, limit=65536):
    # Metadata only, fixed argv; candidate paths are never passed here.
    timed_out = False
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        try:
            result = subprocess.run(command, env=clean_env(), stdout=output,
                                    stderr=errors, timeout=30, check=False)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code, timed_out = None, True
        output.seek(0)
        errors.seek(0)
        raw, error = output.read(limit + 1), errors.read(limit + 1)
    if timed_out or code or len(raw) > limit or len(error) > limit:
        failure = ValueError('metadata command failed/oversized: ' + command[0])
        failure.metadata_receipt = {'argv': command, 'exit': code, 'timed_out': timed_out,
            'stdout': raw[:4096].decode('utf-8', errors='replace'),
            'stderr': error[:4096].decode('utf-8', errors='replace'),
            'stdout_truncated': len(raw) > 4096, 'stderr_truncated': len(error) > 4096}
        raise failure
    return raw.decode('utf-8').strip()


def file_record(path):
    path = Path(path).resolve(strict=True)
    if not path.is_file():
        raise ValueError('required regular tool/library missing')
    return {'sha256': sha(path), 'bytes': path.stat().st_size}


def toolchain_inventory(root):
    if root.is_symlink() or not root.is_dir():
        raise ValueError('installed toolchain root must be a real directory')
    resolved = root.resolve(strict=True)
    files, total = {}, 0
    for path in sorted(root.rglob('*')):
        relative = str(path.relative_to(root))
        if not safe_name(relative):
            raise ValueError('unsafe installed toolchain path')
        if path.is_symlink():
            target = path.resolve(strict=True)
            target.relative_to(resolved)
            if not target.is_file():
                raise ValueError('toolchain symlink is not an internal file')
            record = {'symlink': os.readlink(path), **file_record(target)}
        elif path.is_dir():
            continue
        elif path.is_file():
            record = file_record(path)
        else:
            raise ValueError('special installed toolchain file')
        total += record['bytes']
        if len(files) >= 100000 or total > 8589934592:
            raise ValueError('toolchain inventory size limit')
        files[relative] = record
    component_path = root / 'lib/rustlib/components'
    if component_path.stat().st_size > 65536:
        raise ValueError('component list size limit')
    components = component_path.read_text().splitlines()
    required = {name + '-' + TARGET for name in ('rustc', 'cargo', 'rust-std', 'rustfmt-preview', 'clippy-preview')}
    if len(components) != len(set(components)) or not required.issubset(components):
        raise ValueError('required installed components missing/duplicated')
    manifests = {}
    for component in components:
        if not re.fullmatch('[a-zA-Z0-9_-]+', component):
            raise ValueError('unsafe component name')
        key = 'lib/rustlib/manifest-' + component
        manifests[component] = files[key]
    result = {'schema': 1, 'toolchain': TOOLCHAIN, 'files': files, 'component_manifests': manifests}
    if len((json.dumps(result, indent=2, sort_keys=True) + '\n').encode()) > 33554432:
        raise ValueError('toolchain JSON inventory size limit')
    return result


def provision_command(argv, environment, cwd, timeout):
    code, timed_out, spawn_error = None, False, None
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        try:
            result = subprocess.run(argv, cwd=cwd, env=environment, stdout=output,
                                    stderr=errors, timeout=timeout, check=False)
            code = result.returncode
        except subprocess.TimeoutExpired:
            timed_out = True
        except OSError as error:
            spawn_error = str(error)[:2048]
        output.seek(0)
        errors.seek(0)
        raw, error = output.read(4097), errors.read(4097)
    return {'argv': argv, 'exit': code, 'timed_out': timed_out, 'spawn_error': spawn_error,
            'stdout': raw[:4096].decode('utf-8', errors='replace'),
            'stderr': error[:4096].decode('utf-8', errors='replace'),
            'stdout_truncated': len(raw) > 4096, 'stderr_truncated': len(error) > 4096}


def rustup_settings(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError('unsafe rustup settings file')
    with path.open('rb') as source:
        raw = source.read(65537)
    if len(raw) > 65536:
        raise ValueError('rustup settings size limit')
    settings = tomllib.loads(raw.decode('utf-8'))
    if settings.get('overrides') or settings.get('default_host', TARGET) != TARGET:
        raise ValueError('rustup directory/host overrides present')
    return settings


def provision():
    results = Path(sys.argv[2])
    if results.is_symlink() or not results.is_dir() or any(results.iterdir()):
        raise ValueError('require fresh empty workflow-owned results directory')
    receipt = {'schema': 1, 'status': 'HOLD', 'phase': 'toolchain-provisioning',
               'toolchain': TOOLCHAIN, 'commands': []}
    try:
        if platform.system() != 'Linux' or platform.machine() != 'x86_64':
            raise ValueError('provision only on ephemeral Linux x86_64 runner')
        keys = ('GITHUB_REPOSITORY', 'GITHUB_SHA', 'GITHUB_REF', 'GITHUB_WORKFLOW_REF', 'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT')
        receipt['provenance'] = {key: os.environ[key] for key in keys}
        receipt['mode'] = os.environ['G2_MODE']
        receipt['payload_commit'] = os.environ['G2_PAYLOAD_COMMIT']
        if receipt['provenance']['GITHUB_REPOSITORY'] != 'anilbeesetti/ktlsp' or receipt['provenance']['GITHUB_REF'] != 'refs/heads/main' or receipt['mode'] not in ('preflight', 'compile') or not re.fullmatch('[0-9a-f]{40}', receipt['payload_commit']):
            raise ValueError('provision dispatch identity mismatch')
        home = Path(os.environ['HOME'])
        rustup = home / '.cargo/bin/rustup'
        install = [str(rustup), 'toolchain', 'install', TOOLCHAIN, '--profile', 'minimal',
                   '--component', 'rustfmt', '--component', 'clippy', '--no-self-update']
        receipt['install_argv'] = install
        roots = {'RUSTUP_HOME': str(home / '.rustup'), 'CARGO_HOME': str(home / '.cargo')}
        official = {'RUSTUP_DIST_SERVER': 'https://static.rust-lang.org',
                    'RUSTUP_UPDATE_ROOT': 'https://static.rust-lang.org/rustup'}
        permitted = {**roots, **official, 'RUSTUP_AUTO_INSTALL': '0'}
        forbidden = ('RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC',
                     'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS',
                     'CARGO_ENCODED_RUSTFLAGS', 'RUSTDOC', 'RUSTDOCFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_BUILD_RUSTFLAGS')
        rejected = sorted(key for key, value in os.environ.items() if value and (
            (key in permitted and value != permitted[key]) or
            (key.startswith('RUSTUP_') and key not in permitted) or key in forbidden or
            (key.startswith('CARGO_TARGET_') and key.endswith(('_LINKER', '_RUSTFLAGS', '_RUNNER'))) or
            key.startswith('CARGO_PROFILE_')))
        if rejected:
            receipt['rejected_environment_names'] = [key[:128] for key in rejected[:32]]
            receipt['rejected_environment_count'] = len(rejected)
            raise ValueError('inherited mirror/root/toolchain/compiler overrides rejected')
        settings = home / '.rustup/settings.toml'
        before = rustup_settings(settings)
        if not before.get('default_toolchain'):
            raise ValueError('rustup default missing')
        fallback = Path('/etc/rustup/settings.toml')
        if fallback.exists() or fallback.is_symlink():
            rustup_settings(fallback)
        receipt.update({'installer_before': file_record(rustup), 'official_endpoints': official,
                        'default_before': before['default_toolchain'],
                        'route_helper_sha256': sha(Path(__file__)),
                        'contract_sha256': sha(Path(__file__).parent / 'route-contract.json')})
        environment = {**clean_env(), **permitted}
        for argv, timeout in (([str(rustup), '--version'], 30), (install, 900)):
            result = provision_command(argv, environment, results.parent, timeout)
            receipt['commands'].append(result)
            if result['exit'] != 0 or result['timed_out'] or result['spawn_error']:
                raise ValueError('first provisioning command failure; no fallback/retry')
            if argv[-1] == '--version' and not result['stdout'].startswith('rustup '):
                raise ValueError('installer version output mismatch')
            after = rustup_settings(settings)
            receipt.update({'installer_after': file_record(rustup), 'default_after': after.get('default_toolchain')})
            if receipt['installer_after'] != receipt['installer_before'] or receipt['default_after'] != receipt['default_before']:
                raise ValueError('installer/default identity changed: HOLD')
        inventory = toolchain_inventory(home / '.rustup/toolchains' / TOOLCHAIN)
        write(results / 'toolchain-files.json', inventory)
        receipt.update({'status': 'PROVISIONED_ONLY_NO_COMPILE_GRANT',
                        'toolchain_files_sha256': sha(results / 'toolchain-files.json')})
        write(results / 'status.json', {'status': 'PROVISIONED_ONLY_PREFLIGHT_PENDING'})
    except Exception as error:
        receipt['reason'] = str(error)[:2048]
        write(results / 'status.json', {'status': 'HOLD', 'phase': 'toolchain-provisioning', 'reason': str(error)[:2048]})
        raise
    finally:
        write(results / 'provisioning.json', receipt)
        finalize_results(results)


def collect(control):
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise ValueError('only Linux x86_64 admitted selector')
    toolchain = Path(os.environ['HOME']) / '.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu'
    rustc, cargo = toolchain / 'bin/rustc', toolchain / 'bin/cargo'
    for path in (rustc, cargo):
        if not path.is_file():
            raise ValueError('explicitly provisioned pinned Rust1.99.0 missing: HOLD')
    rust_version = text([str(rustc), '-vV'])
    cargo_version = text([str(cargo), '-V'])
    if not rust_version.startswith('rustc 1.99.0 ') or not cargo_version.startswith('cargo 1.99.0 '):
        raise ValueError('pinned tool versions mismatch')
    cfg = text([str(rustc), '--print', 'cfg', '--target', 'x86_64-unknown-linux-gnu'])
    if 'target_env="gnu"' not in cfg or 'target_pointer_width="64"' not in cfg:
        raise ValueError('target metadata mismatch')
    image_os, image_version = os.environ.get('ImageOS'), os.environ.get('ImageVersion')
    if not image_os or not image_version:
        raise ValueError('runner image release identity missing')
    pages = os.sysconf('SC_PAGE_SIZE')
    aslr = Path('/proc/sys/kernel/randomize_va_space').read_text().strip()
    if pages != 4096 or aslr != '2' or struct.calcsize('P') != 8:
        raise ValueError('page/ASLR policy mismatch')
    preload = Path('/etc/ld.so.preload')
    if preload.exists() and preload.read_bytes():
        raise ValueError('nonempty system loader preload: HOLD')
    cpu_keys = ('vendor_id', 'cpu family', 'model', 'model name', 'stepping',
                'microcode', 'flags', 'siblings', 'cpu cores', 'physical id', 'core id')
    blocks = Path('/proc/cpuinfo').read_text().strip().split('\n\n')
    cpus = []
    for block in blocks:
        fields = dict((k.strip(), v.strip()) for k, v in
                      (line.split(':', 1) for line in block.splitlines() if ':' in line))
        if any(key not in fields for key in cpu_keys):
            raise ValueError('CPU identity field missing')
        cpus.append({key: fields[key] for key in cpu_keys})
    libraries = {name: file_record('/lib/x86_64-linux-gnu/' + name) for name in
                 ('ld-linux-x86-64.so.2', 'libc.so.6', 'libpthread.so.0', 'libm.so.6', 'libgcc_s.so.1')}
    tools = {'rustc': file_record(rustc), 'cargo': file_record(cargo),
             'rustc_version': rust_version, 'cargo_version': cargo_version,
             'target_cfg': cfg, 'python': {'version': sys.version, **file_record(sys.executable)}}
    for name in ('cc', 'ld', 'readelf', 'objdump', 'getconf', 'git'):
        path = Path('/usr/bin') / name
        tools[name] = file_record(path)
    tools['cc_version'] = text(['/usr/bin/cc', '--version']).splitlines()[0]
    tools['ld_version'] = text(['/usr/bin/ld', '--version']).splitlines()[0]
    tools['glibc_version'] = text(['/usr/bin/getconf', 'GNU_LIBC_VERSION'])
    if 'GNU ld' not in tools['ld_version'] or not tools['glibc_version'].startswith('glibc '):
        raise ValueError('GNU linker/glibc missing')
    library_dir = toolchain / 'lib/rustlib/x86_64-unknown-linux-gnu/lib'
    stdlib = {p.name: file_record(p) for p in sorted(library_dir.iterdir()) if p.is_file()}
    compiler_libs = {p.name: file_record(p) for p in sorted((toolchain / 'lib').glob('*.so*')) if p.is_file()}
    if not stdlib or not compiler_libs:
        raise ValueError('compiler/standard-library inventory missing')
    link_inputs = {}
    for name in ('crt1.o', 'Scrt1.o', 'crti.o', 'crtn.o', 'crtbeginS.o', 'crtendS.o',
                 'libgcc.a', 'libgcc_s.so', 'libc_nonshared.a'):
        located = text(['/usr/bin/cc', '-print-file-name=' + name])
        if not Path(located).is_absolute():
            raise ValueError('linker input unavailable: ' + name)
        link_inputs[name] = file_record(located)
    for name in ('collect2', 'ld'):
        located = text(['/usr/bin/cc', '-print-prog-name=' + name])
        if not Path(located).is_absolute():
            located = '/usr/bin/' + located
        link_inputs[name] = file_record(located)
    uname = platform.uname()
    eligibility = {'schema': 1, 'platform': {'system': uname.system, 'machine': uname.machine,
                   'pointer_bits': struct.calcsize('P') * 8, 'page_bytes': pages, 'aslr': aslr},
        'image': {'ImageOS': image_os, 'ImageVersion': image_version,
                  'os_release_sha256': sha('/etc/os-release')},
        'kernel': {'release': uname.release, 'version': uname.version,
                   'proc_version': Path('/proc/version').read_text().strip()},
        'cpu': {'logical_count': len(cpus), 'processors': cpus,
                'possible': Path('/sys/devices/system/cpu/possible').read_text().strip(),
                'online': Path('/sys/devices/system/cpu/online').read_text().strip()},
        'host_envelope': {'cpu_affinity': sorted(os.sched_getaffinity(0)),
              'memory_total': next(line for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemTotal:')),
              'rlimits': {name: list(resource.getrlimit(getattr(resource, name))) for name in
                          ('RLIMIT_AS', 'RLIMIT_STACK', 'RLIMIT_NOFILE', 'RLIMIT_NPROC')},
              'vm_policy': {name: Path('/proc/sys/vm/' + name).read_text().strip() for name in
                            ('overcommit_memory', 'overcommit_ratio')}},
        'tools': tools, 'stdlib': stdlib, 'compiler_libraries': compiler_libs,
        'linker_inputs_inventory': link_inputs,
        'installed_system_libraries': libraries,
        'system_preload': 'empty' if preload.exists() else 'absent',
        'workflow_file_sha256': sha(control / WORKFLOW)}
    return eligibility, toolchain


def safe_name(name):
    path = PurePosixPath(name)
    return bool(name) and not path.is_absolute() and '\\' not in name and all(
        part not in ('', '.', '..') for part in name.split('/'))


def unpack(archive, destination, expected, prefix=''):
    if destination.exists():
        raise ValueError('refuse existing extraction destination')
    with tarfile.open(archive, 'r:gz') as source:
        members = source.getmembers()
        files = {}
        for member in members:
            name = member.name
            if member.isdir():
                if prefix and (name.rstrip('/') == prefix.rstrip('/') or name.startswith(prefix)) and safe_name(name.rstrip('/')):
                    continue
                raise ValueError('unexpected archive directory')
            if not member.isfile() or not safe_name(name) or (prefix and not name.startswith(prefix)):
                raise ValueError('unsafe archive member/type')
            relative = name[len(prefix):] if prefix else name
            if relative in files or relative not in expected or member.size != expected[relative]['bytes']:
                raise ValueError('archive member/size not allowlisted')
            files[relative] = member
        if set(files) != set(expected):
            raise ValueError('archive allowlist not complete')
        destination.mkdir(parents=True)
        for relative, member in files.items():
            raw = source.extractfile(member).read(member.size + 1)
            if len(raw) != member.size or hashlib.sha256(raw).hexdigest() != expected[relative]['sha256']:
                raise ValueError('archive file hash mismatch')
            path = destination / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            with path.open('xb') as output:
                output.write(raw)


def vendor(stage, inventory):
    records = load(stage / 'evidence/REGISTRY-RECORDS.json')
    if len(records) != 11 or len(inventory) != 11:
        raise ValueError('registry graph count')
    root = stage / 'vendor'
    for record in records:
        name = record['name'] + '-' + record['version']
        archive = stage / 'dependencies' / (name + '.crate')
        entry = inventory[name]
        if sha(archive) != record['checksum'] or entry['package_sha256'] != record['checksum']:
            raise ValueError('registry checksum mismatch')
        destination = root / name
        unpack(archive, destination, entry['files'], name + '/')
        checksum = json.dumps({'files': {key: value['sha256'] for key, value in sorted(entry['files'].items())},
                               'package': record['checksum']}, sort_keys=True, separators=(',', ':')) + '\n'
        if hashlib.sha256(checksum.encode()).hexdigest() != entry['cargo_checksum_sha256']:
            raise ValueError('directory-source checksum mismatch')
        (destination / '.cargo-checksum.json').write_text(checksum)
    overlays = {}
    for directory, relative in ((stage, 'vendor'), (stage / 'diagnostic-copy', '../vendor')):
        config = directory / '.cargo/config.toml'
        before = sha(config)
        config.write_text(config.read_text() + '\n[source.crates-io]\nreplace-with = "g2-preserved"\n'
                          '[source.g2-preserved]\ndirectory = "' + relative + '"\n')
        overlays[str(config.relative_to(stage))] = {'before_sha256': before, 'after_sha256': sha(config)}
    return overlays


def command(argv, cwd, environment, log):
    # Compile/ELF inspection only. First failure stops; outputs stay on disk.
    with log.open('xb') as output:
        result = subprocess.run(argv, cwd=cwd, env=environment, stdout=output,
                                stderr=subprocess.STDOUT, check=False)
    write(log.with_suffix('.status.json'), {'argv': argv, 'exit': result.returncode})
    if result.returncode:
        raise ValueError('compile/inspection failed: ' + argv[0])


def allowed_result(relative):
    fixed = {'provenance.json', 'eligibility.json', 'status.json', 'postcompile-facts.json',
             'vendor-overlays.json', 'output-files.json', 'metadata-failure.json', 'provisioning.json', 'toolchain-files.json'}
    return relative in fixed or bool(re.fullmatch(r'(accepted|diagnostic)-(D|R)-build\.(txt|status\.json)', relative)) or bool(
        re.fullmatch(r'artifacts/(accepted|diagnostic)/(D|R)/(capture\.json|g2-(child|supervise)(\.(readelf|objdump)\.(txt|status\.json))?)', relative))


def finalize_results(results):
    files, violations = {}, []
    for path in sorted(results.rglob('*')):
        relative = str(path.relative_to(results))
        if not path.exists() and not path.is_symlink():
            continue
        if path.is_symlink() or (not path.is_dir() and not path.is_file()):
            # Remove only unsafe entries in this fresh owned results directory, not targets.
            path.unlink()
            violations.append(relative)
            continue
        if path.is_dir():
            if relative not in {'artifacts', 'artifacts/accepted', 'artifacts/diagnostic',
                'artifacts/accepted/D', 'artifacts/accepted/R', 'artifacts/diagnostic/D', 'artifacts/diagnostic/R'}:
                violations.append(relative)
                shutil.rmtree(path)
            continue
        if not allowed_result(relative):
            violations.append(relative)
            continue
        if relative != 'output-files.json':
            files[relative] = {'bytes': path.stat().st_size, 'sha256': sha(path)}
    if violations:
        write(results / 'status.json', {'status': 'HOLD', 'phase': 'receipt-allowlist',
              'reason': 'unexpected result path/type', 'paths_first_32': violations[:32]})
        files['status.json'] = file_record(results / 'status.json')
    write(results / 'output-files.json', files)
    if violations:
        raise ValueError('unexpected result path/type')


def run():
    mode, payload_root, control_root, work_root, results_root = sys.argv[1:]
    if mode not in ('preflight', 'compile'):
        raise ValueError('only preflight/compile allowed')
    payload, control, work, results = map(Path, (payload_root, control_root, work_root, results_root))
    if results.is_symlink() or not results.is_dir():
        raise ValueError('require workflow-owned results directory')
    phase = 'provisioning-verification'
    try:
        if {p.name for p in results.iterdir()} != {'provisioning.json', 'toolchain-files.json', 'status.json', 'output-files.json'} or any(not p.is_file() or p.is_symlink() for p in results.iterdir()):
            raise ValueError('require only regular provisioning receipts in workflow-owned results')
        contract = load(payload / 'route-contract.json')
        provisioned = load(results / 'provisioning.json')
        keys = ('GITHUB_REPOSITORY', 'GITHUB_SHA', 'GITHUB_REF', 'GITHUB_WORKFLOW_REF', 'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT')
        if provisioned.get('schema') != 1 or provisioned.get('status') != 'PROVISIONED_ONLY_NO_COMPILE_GRANT' or provisioned.get('toolchain') != TOOLCHAIN or provisioned.get('mode') != mode or provisioned.get('provenance') != {key: os.environ[key] for key in keys} or provisioned.get('payload_commit') != os.environ['G2_PAYLOAD_COMMIT']:
            raise ValueError('successful same-run provisioning receipt required')
        if provisioned.get('route_helper_sha256') != sha(Path(__file__)) or provisioned.get('contract_sha256') != sha(payload / 'route-contract.json'):
            raise ValueError('provisioned route identity mismatch')
        rustup = Path(os.environ['HOME']) / '.cargo/bin/rustup'
        install = [str(rustup), 'toolchain', 'install', TOOLCHAIN, '--profile', 'minimal', '--component', 'rustfmt', '--component', 'clippy', '--no-self-update']
        commands = provisioned.get('commands', [])
        if len(commands) != 2 or [r['argv'] for r in commands] != [[str(rustup), '--version'], install] or any(r['exit'] != 0 or r['timed_out'] or r['spawn_error'] for r in commands):
            raise ValueError('provisioning command evidence mismatch')
        if provisioned['installer_before'] != provisioned['installer_after'] or provisioned['installer_after'] != file_record(rustup) or provisioned['default_before'] != provisioned['default_after']:
            raise ValueError('provisioning installer/default mismatch')
        current = rustup_settings(Path(os.environ['HOME']) / '.rustup/settings.toml')
        if not current.get('default_toolchain') or current['default_toolchain'] != provisioned['default_after']:
            raise ValueError('fresh rustup default identity mismatch')
        fallback = Path('/etc/rustup/settings.toml')
        if fallback.exists() or fallback.is_symlink():
            rustup_settings(fallback)
        inventory = toolchain_inventory(Path(os.environ['HOME']) / '.rustup/toolchains' / TOOLCHAIN)
        if sha(results / 'toolchain-files.json') != provisioned.get('toolchain_files_sha256') or load(results / 'toolchain-files.json', 33554432) != inventory:
            raise ValueError('installed toolchain/component inventory mismatch')
        phase = 'publication-integrity'
        commit = os.environ['G2_PAYLOAD_COMMIT']
        if not re.fullmatch('[0-9a-f]{40}', commit) or text(['/usr/bin/git', '-C', str(payload.parents[2]), 'rev-parse', 'HEAD']) != commit:
            raise ValueError('immutable auxiliary commit mismatch')
        repo = payload.parents[2]
        changed = text(['/usr/bin/git', '-C', str(repo), 'diff', '--name-only', BASE, 'HEAD']).splitlines()
        text(['/usr/bin/git', '-C', str(repo), 'merge-base', '--is-ancestor', BASE, 'HEAD'])
        if set(changed) != set(contract['publication_allowlist']):
            raise ValueError('auxiliary publication diff outside full allowlist')
        pins = {'stage.tar.gz': contract['archive_sha256'], 'stage-members.json': contract['stage_members_sha256'],
                'vendor-members.json': contract['vendor_members_sha256']}
        for path, digest in pins.items():
            if sha(payload / path) != digest:
                raise ValueError('payload pin mismatch')
        provenance_keys = ('GITHUB_REPOSITORY', 'GITHUB_SHA', 'GITHUB_REF', 'GITHUB_WORKFLOW_REF',
                           'GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT')
        provenance = {key: os.environ[key] for key in provenance_keys}
        if provenance['GITHUB_REPOSITORY'] != 'anilbeesetti/ktlsp' or provenance['GITHUB_REF'] != 'refs/heads/main':
            raise ValueError('dispatch must use reviewed default-branch anchor')
        provenance.update({'payload_commit': commit, 'mode': mode, 'route_helper_sha256': sha(Path(__file__)),
                           'contract_sha256': sha(payload / 'route-contract.json'),
                           'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()})
        write(results / 'provenance.json', provenance)
        phase = 'preflight'
        eligibility, toolchain = collect(control)
        eligibility['provisioned_toolchain'] = {'toolchain_files_sha256': provisioned['toolchain_files_sha256'],
            'component_manifests': inventory['component_manifests'], 'installer': provisioned['installer_after'],
            'installer_version': commands[0]['stdout'], 'official_endpoints': provisioned['official_endpoints']}
        write(results / 'eligibility.json', eligibility)
        write(results / 'postcompile-facts.json', {'candidate_elf': 'NOT_COMPILED',
              'actual_linked_helpers': 'UNVERIFIED', 'runtime_ifunc_selections': 'UNVERIFIED_NOT_EXECUTED'})
        if mode == 'preflight':
            write(results / 'status.json', {'status': 'PREFLIGHT_ONLY_NO_COMPILE_GRANT'})
            return
        phase = 'compile-admission-comparison'
        admission_path = payload / 'compile-admission.json'
        if sha(admission_path) != os.environ['G2_ADMISSION_SHA256']:
            raise ValueError('root-reviewed compile admission hash mismatch')
        admission = load(admission_path)
        expected_keys = {'schema', 'status', 'mode', 'expected_eligibility', 'preflight_provenance',
                         'stage_sha256', 'source_sha256', 'accepted_lock_sha256', 'diagnostic_lock_sha256',
                         'route_helper_sha256', 'contract_sha256'}
        if set(admission) != expected_keys or admission['schema'] != 1 or admission['status'] != 'ROOT_ADMITTED' or admission['mode'] != 'compile_only':
            raise ValueError('compile admission missing/HOLD/unknown fields')
        for field, actual in [('stage_sha256', contract['archive_sha256']), ('source_sha256', contract['source_manifest_sha256']),
                              ('accepted_lock_sha256', contract['accepted_lock_sha256']), ('diagnostic_lock_sha256', contract['diagnostic_lock_sha256']),
                              ('route_helper_sha256', provenance['route_helper_sha256']), ('contract_sha256', provenance['contract_sha256'])]:
            if admission[field] != actual:
                raise ValueError('compile admission source/graph/route mismatch')
        if admission['expected_eligibility'] != eligibility:
            raise ValueError('fresh CPU/kernel/image/tool/library eligibility mismatch: stop before Cargo')
        prior = admission['preflight_provenance']
        if not isinstance(prior, dict) or set(prior) != {'run_id', 'attempt', 'eligibility_sha256', 'receipt_sha256'} or not all(isinstance(v, str) and v for v in prior.values()):
            raise ValueError('root-reviewed preflight provenance missing')
        canonical = (json.dumps(eligibility, indent=2, sort_keys=True) + '\n').encode()
        if hashlib.sha256(canonical).hexdigest() != prior['eligibility_sha256']:
            raise ValueError('admitted eligibility receipt hash mismatch')
        phase = 'offline-source-provisioning'
        if work.exists():
            raise ValueError('refuse existing work/cache directory')
        for ancestor in (work.parent, *work.parent.parents):
            if any((ancestor / '.cargo' / name).exists() for name in ('config', 'config.toml')):
                raise ValueError('unreviewed ancestor Cargo configuration')
        (work / 'cargo-home').mkdir(parents=True)
        unpack(payload / 'stage.tar.gz', work / 'stage', load(payload / 'stage-members.json'))
        stage = work / 'stage'
        for path, digest in [('SOURCE-HASHES.json', contract['source_manifest_sha256']), ('Cargo.lock', contract['accepted_lock_sha256']),
                             ('diagnostic-copy/Cargo.lock', contract['diagnostic_lock_sha256'])]:
            if sha(stage / path) != digest:
                raise ValueError('extracted stage source/lock mismatch')
        write(results / 'vendor-overlays.json', vendor(stage, load(payload / 'vendor-members.json')))
        phase = 'compile-only'
        for role, directory in [('accepted', stage), ('diagnostic', stage / 'diagnostic-copy')]:
            for profile, label in [('g2-d', 'D'), ('g2-r', 'R')]:
                environment = clean_env(toolchain)
                environment.update({'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER': '/usr/bin/cc',
                                    'CARGO_HOME': str(work / 'cargo-home'), 'CARGO_NET_OFFLINE': 'true',
                                    'CARGO_TARGET_DIR': str(work / 'targets' / role), 'CARGO_INCREMENTAL': '0',
                                    'RUSTC': str(toolchain / 'bin/rustc')})
                log = results / (role + '-' + label + '-build.txt')
                destination = results / 'artifacts' / role / label
                destination.mkdir(parents=True)
                try:
                    command([str(toolchain / 'bin/cargo'), 'build', '--locked', '--offline', '--frozen',
                             '--profile', profile, '--workspace'], directory, environment, log)
                finally:
                    # Preserve only named outputs already emitted; never execute or retry them.
                    captured = {}
                    for name in ('g2-child', 'g2-supervise'):
                        binary = work / 'targets' / role / 'x86_64-unknown-linux-gnu' / profile / name
                        if binary.is_symlink():
                            raise ValueError('unexpected binary symlink')
                        if binary.is_file():
                            shutil.copy2(binary, destination / name)
                            captured[name] = file_record(destination / name)
                        else:
                            captured[name] = 'NOT_EMITTED'
                    write(destination / 'capture.json', captured)
                # Both named outputs are retained before the first static-inspection command.
                for name in ('g2-child', 'g2-supervise'):
                    binary = destination / name
                    command(['/usr/bin/readelf', '--wide', '--file-header', '--program-headers', '--dynamic',
                             '--symbols', '--debug-dump=frames', str(binary)], directory, environment, destination / (name + '.readelf.txt'))
                    command(['/usr/bin/objdump', '--wide', '--disassemble', '--demangle', str(binary)], directory,
                            environment, destination / (name + '.objdump.txt'))
        write(results / 'postcompile-facts.json', {'candidate_elf': 'STATIC_ARTIFACTS_RETAINED',
              'actual_linked_helpers': 'UNVERIFIED_STATIC_DECLARATIONS_ONLY',
              'runtime_ifunc_selections': 'UNVERIFIED_NOT_EXECUTED', 'compiled_stack_acceptance': 'PENDING_COMPLETE_REVIEW',
              'calibration': 'NOT_RUN', 'tests': 'NOT_RUN', 'candidate_execution': 'NOT_RUN'})
        write(results / 'status.json', {'status': 'COMPILE_ONLY_COMPLETE_NO_RUNTIME_GRANT'})
    except Exception as error:
        if hasattr(error, 'metadata_receipt'):
            write(results / 'metadata-failure.json', {'phase': phase, **error.metadata_receipt})
        write(results / 'status.json', {'status': 'HOLD', 'phase': phase, 'reason': str(error)[:2048]})
        raise
    finally:
        finalize_results(results)


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == 'provision':
        provision()
    else:
        run()
