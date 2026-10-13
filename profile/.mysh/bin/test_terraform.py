import json
import os
import shlex
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch


sys.path.insert(0, str(Path(__file__).parent / "lib"))


def load_wrapper():
    runner = Path(__file__).parent / 'lib/terraform_runner.py'
    namespace = {'__name__': 'terraform_wrapper'}
    exec(compile(runner.read_text(), str(runner), 'exec'), namespace)
    namespace['role_overrides'] = Mock(return_value=None)
    return namespace


def git(*arguments):
    return subprocess.check_output(['git', *map(str, arguments)], text=True, stderr=subprocess.PIPE).strip()


class CommandRoutingTest(unittest.TestCase):
    def test_auto_init_precedes_plan_and_apply_with_inherited_credentials(self):
        wrapper = load_wrapper()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for environment in ('stg', 'prod'):
                backend = f'var-{environment}-backend.tfvars'
                (root / backend).write_text('bucket = "test-state"\n')
                for command in ('plan', 'apply'):
                    arguments = [f'-chdir={root}', command]
                    credentials = []

                    def record_credentials(*args, **kwargs):
                        self.assertNotIn('env', kwargs)
                        credentials.append(os.environ['AWS_PROFILE'])
                        return 0

                    with (
                        self.subTest(environment=environment, command=command),
                        patch.dict('os.environ', {'AWS_PROFILE': 'assumed-profile'}, clear=True),
                        patch.dict(wrapper, {'local_overrides': Mock(return_value={})}),
                        patch('sys.argv', ['terraform', '1.0', environment, *arguments]),
                        patch('subprocess.call', side_effect=record_credentials) as invoke,
                    ):
                        self.assertEqual(wrapper['main'](), 0)
                        terraform = ['mise', 'x', 'terraform@1.0', '--', 'terraform']
                        self.assertEqual(invoke.call_args_list[0].args[0], terraform + [
                            f'-chdir={root}', 'init', '-input=false', f'-backend-config={backend}',
                        ])
                        self.assertEqual(invoke.call_args_list[1].args[0], terraform + arguments + [
                            f'-var-file=var-{environment}.tfvars',
                        ])
                        self.assertEqual(credentials, ['assumed-profile', 'assumed-profile'])

    def test_role_routing_is_opt_in_and_flag_is_consumed(self):
        wrapper = load_wrapper()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for enabled in (False, True):
                arguments = [f'-chdir={root}', 'plan'] + (['--override'] if enabled else [])
                routing = Mock(return_value=None)
                with (
                    self.subTest(enabled=enabled),
                    patch.dict(wrapper, {
                        'local_overrides': Mock(return_value={}),
                        'backend_needs_init': Mock(return_value=False),
                        'role_overrides': routing,
                    }),
                    patch('sys.argv', ['terraform', '1.0', 'prod', *arguments]),
                    patch('subprocess.call', return_value=0) as invoke,
                ):
                    self.assertEqual(wrapper['main'](), 0)
                    self.assertNotIn('--override', invoke.call_args.args[0])
                    self.assertEqual(routing.call_count, int(enabled))
                    self.assertFalse(list(root.glob('traveloka_roles_*.tf.json')))

    def test_normal_run_restores_previously_overridden_local_module(self):
        wrapper = load_wrapper()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            child = root / 'child'
            child.mkdir()
            (root / 'main.tf').write_text('module "child" { source = "./child" }')
            (child / 'main.tf').write_text('data "aws_kms_key" "key" { key_id = "alias/example" }')
            manifest = root / '.terraform/modules/modules.json'
            manifest.parent.mkdir(parents=True)
            manifest.write_text(json.dumps({'Modules': [
                {'Key': '', 'Source': '', 'Dir': '.'},
                {'Key': 'child', 'Source': (root / '.terraform/traveloka-role-modules/example').as_uri(),
                 'Dir': str(child)},
            ]}))
            with (
                patch.dict(wrapper, {'backend_needs_init': Mock(return_value=False)}),
                patch('sys.argv', ['terraform', '1.0', 'prod', f'-chdir={root}', 'plan']),
                patch('subprocess.call', return_value=0) as invoke,
            ):
                self.assertEqual(wrapper['main'](), 0)
                wrapper['role_overrides'].assert_not_called()
                self.assertEqual(invoke.call_args_list[0].args[0][6], 'init')
                self.assertEqual(invoke.call_args_list[1].args[0][6], 'plan')
                self.assertFalse(list(root.glob('*roles*.tf.json')))

    def test_failed_auto_init_stops_requested_command(self):
        wrapper = load_wrapper()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'var-stg-backend.tfvars').write_text('bucket = "test-state"\n')
            with (
                patch.dict(wrapper, {'local_overrides': Mock(return_value={})}),
                patch('sys.argv', ['terraform', '1.0', 'stg', f'-chdir={root}', 'plan']),
                patch('subprocess.call', return_value=7) as invoke,
            ):
                self.assertEqual(wrapper['main'](), 7)
                invoke.assert_called_once()

    def test_environment_files_and_manual_overrides(self):
        wrapper = load_wrapper()
        scenarios = [
            ('stg', ['init'], ['init', '-backend-config=var-stg-backend.tfvars']),
            ('prod', ['init'], ['init', '-backend-config=var-prod-backend.tfvars']),
            ('prod', ['init', '-upgrade'], ['init', '-backend-config=var-prod-backend.tfvars', '-upgrade']),
            ('stg', ['-chdir=project', 'init'], ['-chdir=project', 'init', '-backend-config=var-stg-backend.tfvars']),
            ('prod', ['init', '-help'], ['init', '-help']),
            ('stg', ['init', '-backend-config=custom.tfvars'], ['init', '-backend-config=custom.tfvars']),
            ('prod', ['init', '-backend-config', 'custom.tfvars'], ['init', '-backend-config', 'custom.tfvars']),
            ('stg', ['plan', '-var-file=custom.tfvars'], ['plan', '-var-file=custom.tfvars']),
            ('prod', ['apply', '-var-file', 'custom.tfvars'], ['apply', '-var-file', 'custom.tfvars']),
            ('stg', ['plan', '-var=region=test'], ['plan', '-var-file=var-stg.tfvars', '-var=region=test']),
            ('prod', ['apply', 'saved.tfplan'], ['apply', 'saved.tfplan']),
            ('stg', ['apply', '-lock-timeout', '10s'], ['apply', '-var-file=var-stg.tfvars', '-lock-timeout', '10s']),
            ('prod', ['apply', '-parallelism', '2', 'saved.tfplan'], ['apply', '-parallelism', '2', 'saved.tfplan']),
        ]
        for environment, arguments, expected in scenarios:
            with (
                self.subTest(environment=environment, arguments=arguments),
                patch.dict(wrapper, {
                    'local_overrides': Mock(return_value={}),
                    'backend_needs_init': Mock(return_value=False),
                }),
                patch('sys.argv', ['terraform', '1.0', environment, *arguments]),
                patch('subprocess.call', return_value=0) as invoke,
            ):
                self.assertEqual(wrapper['main'](), 0)
                invoke.assert_called_once_with(['mise', 'x', 'terraform@1.0', '--', 'terraform', *expected])

    def test_missing_selected_backend_stops_plan(self):
        wrapper = load_wrapper()
        with tempfile.TemporaryDirectory() as temporary:
            with (
                patch('sys.argv', ['terraform', '1.0', 'prod', f'-chdir={temporary}', 'plan']),
                patch('subprocess.call') as invoke,
            ):
                with self.assertRaises(FileNotFoundError):
                    wrapper['main']()
                invoke.assert_not_called()

    def test_wrapper_environment_priority_and_credentials(self):
        wrapper = Path(__file__).with_name('terraform').resolve()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, script in {
                'assume': 'printf "%s\\n" "$@"\n',
                'terraform-version-name': 'echo 1.0\n',
                'uv': 'echo wrapper-uv >&2\nshift 5\nexec python3 "$@"\n',
                'python3': 'printf "%s\\n" "$@"\nexit 7\n',
            }.items():
                executable = root / name
                executable.write_text('#!/bin/sh\n' + script)
                executable.chmod(0o755)
            scenarios = [
                ('project', None, None, ['plan'], 'stg'),
                ('project', 'DeployerExt@tvlk-fpr-prod', None, ['plan', '--override'], 'prod'),
                ('project', 'DeployerExt@tvlk-fpr-stg', None, ['plan', '--override'], 'stg'),
                ('production/project', None, None, ['plan'], 'prod'),
                ('staging/project', None, None, ['plan'], 'stg'),
                ('prod', None, None, ['plan'], 'prod'),
                ('stg', None, None, ['plan'], 'stg'),
                ('staging/project', 'DeployerExt@tvlk-fpr-prod', None, ['plan'], 'prod'),
                ('production/project', 'DeployerExt@tvlk-fpr-stg', None, ['plan'], 'stg'),
                ('production/project', None, '1', ['plan'], 'prod'),
                ('staging/project', 'DeployerExt@tvlk-fpr-stg', None, ['plan', '-var-file=var-prod.tfvars'], 'prod'),
                ('production/project', 'DeployerExt@tvlk-fpr-prod', None, ['apply', '-var-file', 'var-stg.tfvars'], 'stg'),
                ('staging/project', None, None, ['init', '-backend-config=var-prod-backend.tfvars'], 'prod'),
                ('production/project', None, None, ['init', '-backend-config', 'var-stg-backend.tfvars'], 'stg'),
                ('production/project', None, None, ['plan', '-var-file=custom.tfvars'], 'prod'),
                ('staging/project', None, None, ['plan', '-var-file=var-stg.tfvars', '-var-file=var-prod.tfvars'], 'prod'),
                ('project', None, None, ['-chdir=/production/project', 'plan'], 'stg'),
                ('project', 'DeployerExt@tvlk-fpr-prod', None, ['apply', 'stg.tfplan'], 'prod'),
                ('project', 'unrecognized-profile', None, ['plan', '-var-file=var-prod.tfvars'], 'prod'),
                ('project', 'unrecognized-profile', None, ['plan'], 'stg'),
                ('production/project', 'unrecognized-profile', None, ['plan'], 'prod'),
                ('staging/project', 'Developer@team-prod', None, ['plan'], 'prod'),
                ('production/project', 'ReadOnly@team-stg', None, ['plan'], 'stg'),
                ('project', 'ReadOnly@team-stg', None, ['plan', '-var-file=var-prod.tfvars'], 'prod'),
                ('project', None, None, ['plan', '-var-file=directory with spaces/var-prod.tfvars'], 'prod'),
            ]
            for folder, profile, guard, arguments, target in scenarios:
                working_directory = root / folder
                working_directory.mkdir(parents=True, exist_ok=True)
                environment = dict(os.environ, PATH=f'{root}:{os.environ["PATH"]}')
                environment.pop('AWS_PROFILE', None)
                environment.pop('TF_ASSUME_WRAPPED', None)
                if profile:
                    environment['AWS_PROFILE'] = profile
                if guard:
                    environment['TF_ASSUME_WRAPPED'] = guard
                with self.subTest(folder=folder, profile=profile, guard=guard, arguments=arguments):
                    invocation = subprocess.run(['fish', '--no-config', str(wrapper), *arguments],
                                                cwd=working_directory, env=environment, capture_output=True, text=True)
                    if (profile == f'DeployerExt@tvlk-fpr-{target}' or guard
                            or profile in ('unrecognized-profile', 'Developer@team-prod', 'ReadOnly@team-stg')):
                        self.assertEqual(invocation.returncode, 7, invocation.stderr)
                        self.assertEqual('wrapper-uv' in invocation.stderr, '--override' in arguments)
                        self.assertEqual(invocation.stdout.splitlines(), [
                            '-B', str(wrapper.parent / 'lib/terraform_runner.py'), '1.0', target, *arguments,
                        ])
                    else:
                        self.assertEqual(invocation.returncode, 0, invocation.stderr)
                        assumed_arguments = invocation.stdout.splitlines()
                        self.assertEqual(assumed_arguments[:3], [f'DeployerExt@tvlk-fpr-{target}', '-x', '--exec'])
                        self.assertEqual(shlex.split(assumed_arguments[3]), ['terraform', *arguments])


class ModuleRefreshTest(unittest.TestCase):
    def test_cached_refs_and_explicit_refresh(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            origin = directory / 'origin'
            git('init', '-b', 'main', origin)
            git('-C', origin, 'config', 'user.email', 'test@example.com')
            git('-C', origin, 'config', 'user.name', 'Test')
            (origin / 'main.tf').write_text('# first commit\n')
            git('-C', origin, 'add', 'main.tf')
            git('-C', origin, 'commit', '-m', 'first')
            first_commit = git('-C', origin, 'rev-parse', 'HEAD')
            checkout = directory / 'module-cache/example/checkout'
            checkout.parent.mkdir(parents=True)
            git('clone', origin, checkout)
            root = directory / 'project'
            root.mkdir()
            (root / 'main.tf').write_text('module "example" { source = "git::https://github.com/traveloka/example.git?ref=main" }')
            def selected_commit(project, **options):
                # Reload to model separate wrapper invocations, including its in-process cache.
                invocation = load_wrapper()
                invocation['PLUGIN_CACHE'] = directory / 'plugin-cache'
                overrides = invocation['local_overrides'](project, **options)
                source = overrides[project]['module']['example']['source']
                return git('-C', source.removeprefix('file://'), 'rev-parse', 'HEAD')

            self.assertEqual(selected_commit(root), first_commit)
            (origin / 'main.tf').write_text('# second commit\n')
            git('-C', origin, 'commit', '-am', 'second')
            second_commit = git('-C', origin, 'rev-parse', 'HEAD')
            self.assertEqual(selected_commit(root), first_commit)
            self.assertEqual(selected_commit(root, upgrade=True), second_commit)
            self.assertEqual(selected_commit(root), second_commit)

            git('-C', checkout, 'remote', 'set-url', 'origin', directory / 'missing-origin')
            self.assertEqual(selected_commit(root), second_commit)
            with self.assertRaises(subprocess.CalledProcessError):
                selected_commit(root, upgrade=True)
            self.assertEqual(selected_commit(root), second_commit)
            git('-C', checkout, 'remote', 'set-url', 'origin', origin)

            other_root = directory / 'other-project'
            other_root.mkdir()
            (other_root / 'main.tf').write_text((root / 'main.tf').read_text())
            self.assertEqual(selected_commit(other_root), second_commit)
            (origin / 'main.tf').write_text('# third commit\n')
            git('-C', origin, 'commit', '-am', 'third')
            third_commit = git('-C', origin, 'rev-parse', 'HEAD')
            self.assertEqual(selected_commit(other_root, upgrade=True), third_commit)
            self.assertEqual(selected_commit(root), second_commit)

            # Sources without a ref follow HEAD and have the same refresh rules.
            (root / 'main.tf').write_text('module "example" { source = "git::https://github.com/traveloka/example.git" }')
            self.assertEqual(selected_commit(root), third_commit)
            (origin / 'main.tf').write_text('# fourth commit\n')
            git('-C', origin, 'commit', '-am', 'fourth')
            self.assertEqual(selected_commit(root), third_commit)
            self.assertEqual(selected_commit(root, upgrade=True), git('-C', origin, 'rev-parse', 'HEAD'))

            # A pinned SHA stays fixed, and changing the source installs that ref.
            (root / 'main.tf').write_text(f'module "example" {{ source = "git::https://github.com/traveloka/example.git?ref={first_commit}" }}')
            self.assertEqual(selected_commit(root), first_commit)
            self.assertEqual(selected_commit(root, upgrade=True), first_commit)

            # Removing Terraform's data directory discards its selected refs too.
            with patch.dict('os.environ', {'TF_DATA_DIR': 'custom-data'}):
                (root / 'main.tf').write_text('module "example" { source = "git::https://github.com/traveloka/example.git?ref=main" }')
                self.assertEqual(selected_commit(root), git('-C', origin, 'rev-parse', 'HEAD'))
                self.assertTrue((root / 'custom-data/traveloka-module-refs').is_dir())
                for cached_ref in (root / 'custom-data/traveloka-module-refs/example').iterdir():
                    cached_ref.unlink()
                (origin / 'main.tf').write_text('# fifth commit\n')
                git('-C', origin, 'commit', '-am', 'fifth')
                self.assertEqual(selected_commit(root), git('-C', origin, 'rev-parse', 'HEAD'))

    def test_refresh_flags(self):
        requests_upgrade = load_wrapper()['modules_upgrade_requested']
        with patch.dict('os.environ', {}, clear=True):
            self.assertFalse(requests_upgrade('plan', []))
            self.assertFalse(requests_upgrade('init', []))
            self.assertTrue(requests_upgrade('init', ['-upgrade']))
            self.assertTrue(requests_upgrade('init', ['-upgrade=true']))
            self.assertFalse(requests_upgrade('init', ['-upgrade=false']))
            self.assertFalse(requests_upgrade('init', ['-upgrade', '-upgrade=false']))
            self.assertFalse(requests_upgrade('get', []))
            self.assertTrue(requests_upgrade('get', ['-update']))
            self.assertFalse(requests_upgrade('get', ['-update=false']))
            self.assertFalse(requests_upgrade('plan', ['-upgrade']))
            with patch.dict('os.environ', {'TF_CLI_ARGS_init': '-upgrade'}):
                self.assertTrue(requests_upgrade('init', []))
                self.assertFalse(requests_upgrade('init', ['-upgrade=false']))
            with patch.dict('os.environ', {'TF_CLI_ARGS': '-update=true'}):
                self.assertTrue(requests_upgrade('get', []))

    def test_main_passes_refresh_flag(self):
        wrapper = load_wrapper()
        for arguments, expected in [(['plan'], False), (['init', '-upgrade'], True), (['get', '-update'], True)]:
            overrides = Mock(return_value={})
            with (
                self.subTest(arguments=arguments),
                patch.dict('os.environ', {}, clear=True),
                patch.dict(wrapper, {'local_overrides': overrides}),
                patch('sys.argv', ['terraform', '1.0', '', *arguments]),
                patch('subprocess.call', return_value=0),
            ):
                self.assertEqual(wrapper['main'](), 0)
                overrides.assert_called_once_with(Path.cwd(), expected)


if __name__ == '__main__':
    unittest.main()
