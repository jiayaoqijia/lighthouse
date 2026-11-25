#!/usr/bin/env python3
"""Reproduce and strictly consume the complete pinned full-node Gloas inventory.

Keep generated inputs, immutable executables and receipts outside the checkout.
Four light-client *client-role* cases are inventoried, never counted as passes.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
BUNDLE = HERE / 'source-pinned/current-d2021d27'
REFERENCE = 'd2021d2719235f0df4726a1d330502cb15aa8ea3'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def manifest(name):
    data = json.loads(gzip.decompress((BUNDLE / (name + '.manifest.json.gz')).read_bytes()))
    if data['reference'] != REFERENCE:
        raise ValueError('Unexpected reference source')
    return data


def extract(archive, destination, expected=None):
    """Only regular files/directories, no duplicate names or escaping members."""
    seen = set()
    with tarfile.open(archive, 'r:*') as source:
        for member in source:
            name = PurePosixPath(member.name)
            if name.is_absolute() or '..' in name.parts or not name.parts:
                raise ValueError('Invalid archive path')
            if member.name in seen or not (member.isfile() or member.isdir()):
                raise ValueError('Duplicate or unsupported archive member')
            seen.add(member.name)
            path = destination / name
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
                continue
            if expected is not None and member.name not in expected:
                raise ValueError('Unexpected input file')
            path.parent.mkdir(parents=True, exist_ok=True)
            with source.extractfile(member) as incoming, path.open('wb') as output:
                shutil.copyfileobj(incoming, output)
            if expected is not None and digest(path) != expected[member.name]:
                raise ValueError('Input content differs from independent reference')
    if expected is not None and seen != expected.keys():
        raise ValueError('Incomplete input archive')


def verify_files(root, expected):
    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    if actual != expected.keys():
        raise ValueError('Missing or unexpected generated inputs')
    for name, sha in expected.items():
        if digest(root / name) != sha:
            raise ValueError(f'Generated input differs: {name}')


def prepare(output):
    if output.exists():
        raise ValueError('Use a new output directory; existing evidence is preserved')
    runtime = manifest('runtime')
    archive = BUNDLE / runtime['vector_archive']['name']
    if archive.name != runtime['vector_archive']['name'] or digest(archive) != runtime['vector_archive']['sha256']:
        raise ValueError('Runtime archive digest mismatch')
    if sum(runtime['case_counts'].values()) != 2525:
        raise ValueError('Unexpected dynamic case inventory')
    s = importlib.util.spec_from_file_location('source_fcr', HERE / 'install_source_pinned_fcr.py')
    fcr = importlib.util.module_from_spec(s)
    s.loader.exec_module(fcr)
    # Validate every FCR/source hash before publishing any prepared directory.
    fcr_files = fcr.verified_files(fcr.BUNDLE)
    fcr_manifest = json.loads((fcr.BUNDLE / 'manifest.json').read_text())
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.current-stage-', dir=output.parent) as temporary:
        stage = Path(temporary) / 'inputs'
        stage.mkdir()
        extract(archive, stage / 'runtime', runtime['files'])
        for name, data in fcr_files.items():
            path = stage / 'fcr' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        extract(fcr.BUNDLE / fcr_manifest['source_archive']['name'], stage / 'reference')
        stage.rename(output)
    print(f'Prepared {REFERENCE}: dynamic2525, FCR330; SSZ9344 must be independently regenerated.')


def regenerate_ssz(inputs):
    ref = inputs / 'reference'
    output = inputs / 'ssz'
    if output.exists():
        raise ValueError('Preserve previous generation; use fresh inputs')
    output.mkdir()
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', PYTHONPATH=f'{ref}/tests/core/pyspec:{ref}')
    with (inputs / 'ssz-generation.log').open('w') as log:
        subprocess.run([sys.executable, '-m', 'pytest',
            'tests/core/pyspec/eth_consensus_specs/test/phase0/ssz_static/test_ssz_static.py',
            '--preset=minimal', '--preset=mainnet', '--fork=gloas', '--reftests',
            '--reftests-output=' + str(output), '-p', 'no:cacheprovider', '-n', '2', '-q',
            '--junitxml=' + str(inputs / 'ssz-generation.xml')],
            cwd=ref, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    expected = manifest('ssz')
    verify_files(output, expected['files'])
    assert sum(expected['case_counts'].values()) == 9344
    (inputs / 'ssz-generation-receipt.json').write_text(json.dumps({
        'reference': REFERENCE, 'cases': 9344, 'files': len(expected['files']),
        'byte_identity': True}, indent=2) + '\n')


def git(*args):
    return subprocess.check_output(['git', *args], cwd=REPO, text=True).strip()


def build(output):
    revision = git('rev-parse', 'HEAD')
    if git('status', '--porcelain'):
        raise ValueError('Build only a clean, committed source tree')
    output.mkdir(parents=True, exist_ok=False)
    target = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target'))
    if not target.is_absolute():
        target = REPO / target
    for mode, features in [('real', 'ef_tests'), ('fixture_fake', 'ef_tests,fake_crypto_real_pubkeys')]:
        env = dict(os.environ, CARGO_INCREMENTAL='0', ETH2030_SOURCE_REVISION=revision, ETH2030_SOURCE_DIRTY='false')
        with (output / (mode + '-build.log')).open('w') as log:
            subprocess.run(['cargo', 'build', '--locked', '--release', '-p', 'ef_tests',
                '--features', features, '--bin', 'current_spec_tests'], cwd=REPO, env=env,
                stdout=log, stderr=subprocess.STDOUT, check=True)
        if git('rev-parse', 'HEAD') != revision or git('status', '--porcelain'):
            raise ValueError('Source changed during qualification build')
        binary = output / ('current-spec-tests-' + mode)
        shutil.copyfile(target / 'release/current_spec_tests', binary)
        binary.chmod(0o755)
        (output / (mode + '.json')).write_text(json.dumps({
            'revision': revision, 'tree': git('rev-parse', 'HEAD^{tree}'), 'dirty': False,
            'features': features, 'mode': mode, 'sha256': digest(binary)}, indent=2) + '\n')


def validate_results(records, expected, revision, mode, fork):
    paths = [Path(case['path']).resolve() for case in records]
    if len(paths) != len(set(paths)) or set(paths) != expected:
        raise ValueError('Missing, duplicate or unexpected case receipts')
    for case in records:
        if (case['source_revision'] != revision or case['source_dirty'] is not False
                or case['crypto'] != mode or case['fork'] != fork
                or case['passed'] is not True or case['error'] is not None):
            raise ValueError('Case failed, skipped or used an unqualified source/crypto profile')


def run(inputs, binaries, output, workers):
    from ruamel.yaml import YAML
    yaml = YAML(typ='safe')
    output.mkdir(parents=True, exist_ok=False)
    stamps = {mode: json.loads((binaries / (mode + '.json')).read_text()) for mode in ('real', 'fixture_fake')}
    for mode, stamp in stamps.items():
        if stamp['dirty'] or stamp['mode'] != mode or stamp['sha256'] != digest(binaries / ('current-spec-tests-' + mode)):
            raise ValueError('Immutable executable receipt mismatch')
    if stamps['real']['revision'] != stamps['fixture_fake']['revision']:
        raise ValueError('Crypto lanes must use the same committed client')
    revision = stamps['real']['revision']
    # Verify installed inputs again before executing them.
    verify_files(inputs / 'runtime', manifest('runtime')['files'])
    verify_files(inputs / 'ssz', manifest('ssz')['files'])
    fcr_manifest = json.loads((HERE / 'source-pinned/fcr-d2021d27/manifest.json').read_text())
    verify_files(inputs / 'fcr', fcr_manifest['files'])
    jobs = {}
    omitted = []
    synthetic_operations = []
    for lane in ('runtime', 'ssz', 'fcr'):
        for marker in sorted((inputs / lane).rglob('manifest.yaml')):
            case = marker.parent
            rel = case.relative_to(inputs / lane)
            parts = rel.parts[1:] if lane != 'ssz' else rel.parts
            preset, fork, runner, handler = parts[:4]
            if runner == 'light_client' and handler == 'sync':
                omitted.append({'path': str(rel), 'status': 'unrun_client_role',
                    'reason': 'This client is a full-node LC server; wallet/store sync is a separate role.'})
                continue
            mode = 'fixture_fake' if lane in ('ssz', 'fcr') else 'real'
            if runner == 'operations' and (case / 'meta.yaml').exists():
                if yaml.load((case / 'meta.yaml').read_text()).get('bls_setting') == 2:
                    mode = 'fixture_fake'
                    synthetic_operations.append(str(rel))
            # Isolate each selected set without editing any original vector bytes.
            key = (preset, fork, runner, handler, mode)
            jobs.setdefault(key, []).append(case)
    totals = {'ssz': sum(len(v) for k,v in jobs.items() if k[2]=='ssz_static'),
        'fcr': sum(len(v) for k,v in jobs.items() if k[2]=='fast_confirmation'),
        'all': sum(map(len, jobs.values())), 'client_role': len(omitted),
        'synthetic_operations': len(synthetic_operations)}
    if totals != {'ssz': 9344, 'fcr': 330, 'all': 12195, 'client_role': 4, 'synthetic_operations': 6}:
        raise ValueError(f'Inventory differs: {totals}')
    (output / 'inventory.json').write_text(json.dumps({'reference': REFERENCE, 'totals': totals,
        'client_role_unrun': omitted, 'explicit_ignored_bls_operations': synthetic_operations}, indent=2) + '\n')

    def execute(item):
        (preset, fork, runner, handler, mode), cases = item
        name = '-'.join((preset, fork, runner, handler, mode))
        subset = output / 'selected' / name / handler
        for case in cases:
            dest = subset / case.parent.name / case.name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(case, dest, copy_function=os.link)
        expected = {p.parent.resolve() for p in subset.rglob('manifest.yaml')}
        command = [str(binaries / ('current-spec-tests-' + mode)), preset, runner, str(subset)]
        if runner == 'fast_confirmation':
            command.append(fork)
        log = output / (name + '.log')
        with log.open('w') as stream:
            result = subprocess.run(command, cwd=REPO, env=dict(os.environ, RUST_LOG='warn', RAYON_NUM_THREADS='2'),
                stdout=stream, stderr=subprocess.STDOUT)
        records = []
        error = None
        try:
            for line in log.open():
                if line.startswith('{'):
                    records.append(json.loads(line))
            validate_results(records, expected, revision, mode, fork)
            if result.returncode != 0:
                raise ValueError(f'Native consumer exited {result.returncode}')
        except (ValueError, KeyError) as exc:
            error = str(exc)
        receipt = {'name': name, 'expected': len(cases), 'reported': len(records),
            'passed': sum(x.get('passed') is True for x in records), 'exit_code': result.returncode,
            'error': error, 'log': str(log)}
        print(json.dumps(receipt), flush=True)
        return receipt

    with ThreadPoolExecutor(max_workers=workers) as pool:
        results = list(pool.map(execute, sorted(jobs.items())))
    summary = {'reference': REFERENCE, 'client_revision': revision, 'inventory': totals,
        'passed': sum(x['passed'] for x in results), 'failed_jobs': sum(x['error'] is not None for x in results),
        'jobs': results, 'client_role_unrun': omitted}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    if summary['failed_jobs'] or summary['passed'] != 12195:
        raise SystemExit('Current-reference qualification failed; inspect all retained job receipts')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for name in ('prepare', 'regenerate-ssz', 'build'):
        command = commands.add_parser(name)
        command.add_argument('directory', type=Path)
    command = commands.add_parser('run')
    command.add_argument('inputs', type=Path)
    command.add_argument('binaries', type=Path)
    command.add_argument('output', type=Path)
    command.add_argument('--workers', type=int, choices=range(1, 9), default=2)
    args = parser.parse_args()
    if args.command == 'prepare': prepare(args.directory.resolve())
    elif args.command == 'regenerate-ssz': regenerate_ssz(args.directory.resolve())
    elif args.command == 'build': build(args.directory.resolve())
    else: run(args.inputs.resolve(), args.binaries.resolve(), args.output.resolve(), args.workers)
