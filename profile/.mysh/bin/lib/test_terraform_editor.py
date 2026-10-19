import asyncio
import contextlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

TF = os.environ.get('TEST_TERRAFORM_BINARY', str(Path.home() / '.mysh/mise/installs/terraform/1.7.5/terraform'))
LS = os.environ.get('TEST_TERRAFORM_LS_BINARY', str(Path.home() / '.mysh/mise/installs/terraform-ls/0.39.0/terraform-ls'))
spec = importlib.util.spec_from_file_location('terraform_runner', Path(__file__).with_name('terraform_runner.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


async def read_lsp_message(reader):
    headers = {}
    while True:
        line = await reader.readline()
        if line == b'\r\n':
            break
        if not line:
            raise EOFError('Terraform LS closed its output')
        name, content = line.decode().split(':', 1)
        headers[name.lower()] = content.strip()
    return json.loads(await reader.readexactly(int(headers['content-length'])))


async def lsp_request(process, method, parameters, identifier=None):
    message = {'jsonrpc': '2.0', 'method': method, 'params': parameters}
    if identifier is not None:
        message['id'] = identifier
    encoded = json.dumps(message).encode()
    process.stdin.write(f'Content-Length: {len(encoded)}\r\n\r\n'.encode() + encoded)
    await process.stdin.drain()
    if identifier is None:
        return None
    while True:
        response = await read_lsp_message(process.stdout)
        if response.get('id') != identifier:
            continue
        if 'error' in response:
            raise RuntimeError(response['error'])
        return response.get('result')


async def module_completions(directory, configuration):
    process = await asyncio.create_subprocess_exec(LS, 'serve', cwd=directory,
        stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
    try:
        await asyncio.wait_for(lsp_request(process, 'initialize', {
            'processId': os.getpid(), 'rootUri': directory.as_uri(), 'capabilities': {},
            'workspaceFolders': [{'uri': directory.as_uri(), 'name': 'fixture'}],
            'initializationOptions': {'terraform': {'path': TF}}}, 1), 10)
        await lsp_request(process, 'initialized', {})
        document = {'uri': (directory / 'main.tf').as_uri()}
        await lsp_request(process, 'textDocument/didOpen', {'textDocument': {
            **document, 'languageId': 'terraform', 'version': 1, 'text': configuration}})
        deadline = time.monotonic() + 10
        identifier = 1
        labels = []
        while time.monotonic() < deadline:
            identifier += 1
            completion = await asyncio.wait_for(lsp_request(process, 'textDocument/completion', {
                'textDocument': document, 'position': {'line': 2, 'character': 5}}, identifier), 10)
            entries = completion.get('items', []) if isinstance(completion, dict) else completion or []
            labels = [entry['label'] for entry in entries]
            if 'child_input' in labels:
                return labels
            await asyncio.sleep(0.1)
        return labels
    finally:
        process.terminate()
        await asyncio.wait_for(process.wait(), 5)


class EditorModulesTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / 'project'
        self.root.mkdir()
        self.cache = Path(self.temporary.name) / 'cache'
        self.cache.mkdir()
        self.child = Path(self.temporary.name) / 'child'
        self.child.mkdir()
        self.source = 'git::https://github.com/example/parent.git?ref=v1'
        self.child_source = 'git::https://github.com/example/child.git?ref=v1'
        (self.root / 'main.tf').write_text('module "example" {\n source = ' + json.dumps(self.source) + '\n}\n')
        (self.cache / 'main.tf').write_text('module "child" {\n source = ' + json.dumps(self.child_source) + '\n}\n')
        (self.cache / 'variables.tf').write_text('variable "cache_test_input" {\n type = string\n default = "test"\n}\n')
        (self.child / 'outputs.tf').write_text('output "test" { value = "cached" }\n')
        self.overrides = {
            self.root: {'module': {'example': {'source': self.cache.as_uri()}}},
            self.cache: {'module': {'child': {'source': self.child.as_uri()}}},
        }
        self.calls = []
        self.failure = None
        self.data_directory = ''

    def run_terraform(self, command, **options):
        arguments = command[5:]
        self.calls.append(arguments)
        if self.failure == arguments[0]:
            return 17
        execution = subprocess.run([TF, *arguments], cwd=self.root, text=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        if execution.returncode:
            self.fail(execution.stdout)
        return execution.returncode

    def invoke(self, *arguments):
        with (contextlib.chdir(self.root), patch.dict(os.environ, {'TF_DATA_DIR': self.data_directory}),
              patch.object(sys, 'argv', ['terraform_runner.py', '1.7.5', '', *arguments]),
              patch.object(runner, 'local_overrides', return_value=self.overrides),
              patch.object(runner.subprocess, 'call', side_effect=self.run_terraform)):
            return runner.main()

    def manifest(self):
        manifest = json.loads((self.root / (self.data_directory or '.terraform') / 'modules/modules.json').read_text())
        return {module['Key']: module for module in manifest['Modules']}

    def test_completion_metadata_and_repeated_commands_reuse_cache(self):
        self.assertEqual(self.invoke('init', '-backend=false'), 0)
        installed = self.manifest()
        self.assertEqual(installed['example']['Source'], self.source)
        self.assertEqual(installed['example.child']['Source'], self.child_source)
        self.assertTrue((self.root / installed['example']['Dir'] / 'variables.tf').is_file())
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 1)
        self.assertEqual(installed, self.manifest())

    def test_changed_cached_revision_triggers_init(self):
        self.invoke('init', '-backend=false')
        new_cache = Path(self.temporary.name) / 'cache-v2'
        new_cache.mkdir()
        (new_cache / 'variables.tf').write_text('variable "new_input" { default = "new" }\n')
        self.overrides = {self.root: {'module': {'example': {'source': new_cache.as_uri()}}}}
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 2)
        self.assertEqual(self.manifest()['example']['Source'], self.source)
        self.assertNotIn('example.child', self.manifest())

    def test_failed_command_still_restores_editor_sources(self):
        self.invoke('init', '-backend=false')
        self.failure = 'validate'
        self.assertEqual(self.invoke('validate'), 17)
        self.assertEqual(self.manifest()['example']['Source'], self.source)
        self.assertEqual(self.manifest()['example.child']['Source'], self.child_source)
        self.assertFalse(list(self.root.glob('traveloka_local_*_override.tf.json')))

    def test_failed_init_leaves_no_overrides_or_manifest(self):
        self.failure = 'init'
        self.assertEqual(self.invoke('init', '-backend=false'), 17)
        self.assertFalse(list(self.root.glob('traveloka_local_*_override.tf.json')))
        self.assertFalse((self.root / '.terraform/modules/modules.json').exists())

    def test_external_manifest_changes_are_not_reverted(self):
        self.invoke('init', '-backend=false')
        manifest_path = self.root / '.terraform/modules/modules.json'
        manifest = json.loads(manifest_path.read_text())
        manifest['Modules'][1]['Source'] = 'git::https://github.com/example/different.git'
        manifest_path.write_text(json.dumps(manifest))
        runner.restore_execution_sources(self.root)
        self.assertEqual(self.manifest()['example']['Source'], 'git::https://github.com/example/different.git')

    def test_custom_data_directory_preserves_cache(self):
        self.data_directory = 'terraform-data'
        self.assertEqual(self.invoke('init', '-backend=false'), 0)
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 1)
        self.assertEqual(self.manifest()['example']['Source'], self.source)
        self.assertFalse((self.root / '.terraform/modules/modules.json').exists())

    def test_exception_still_restores_editor_sources(self):
        self.invoke('init', '-backend=false')
        with patch.object(self, 'run_terraform', side_effect=OSError('execution failed')):
            with self.assertRaisesRegex(OSError, 'execution failed'):
                self.invoke('validate')
        self.assertEqual(self.manifest()['example']['Source'], self.source)
        self.assertFalse(list(self.root.glob('traveloka_local_*_override.tf.json')))

    def test_ssh_sources_use_the_address_expected_by_the_language_server(self):
        self.source = 'git@github.com:traveloka/parent.git?ref=v1'
        (self.root / 'main.tf').write_text('module "example" { source = ' + json.dumps(self.source) + ' }\n')
        self.invoke('init', '-backend=false')
        self.assertEqual(self.manifest()['example']['Source'], 'git::ssh://git@github.com/traveloka/parent.git?ref=v1')
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 1)

    def test_installed_nested_module_has_its_own_editor_manifest(self):
        role_module = self.root / '.terraform/role-module'
        shutil.copytree(self.cache, role_module)
        self.overrides = {
            self.root: {'module': {'example': {'source': role_module.as_uri()}}},
            role_module: {'module': {'child': {'source': self.child.as_uri()}}},
        }
        self.invoke('init', '-backend=false')
        parent = self.root / self.manifest()['example']['Dir']
        projection = json.loads((parent / '.terraform/modules/modules.json').read_text())
        projected = {module['Key']: module for module in projection['Modules']}
        self.assertEqual(projected['']['Dir'], '.')
        self.assertEqual(projected['child']['Source'], self.child_source)
        child = Path(os.path.abspath(parent / projected['child']['Dir'])).resolve()
        self.assertEqual(child, (self.root / self.manifest()['example.child']['Dir']).resolve())
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 1)

    def test_shared_cache_is_not_modified_by_editor_projections(self):
        self.invoke('init', '-backend=false')
        self.assertFalse((self.cache / '.terraform/modules/modules.json').exists())
        self.assertFalse((self.child / '.terraform/modules/modules.json').exists())

    def test_installed_modules_are_local_copies_and_survive_repeated_commands(self):
        self.invoke('init', '-backend=false')
        parent = self.root / self.manifest()['example']['Dir']
        self.assertFalse(parent.is_symlink())
        self.assertTrue((parent / 'traveloka_local_installed_override.tf.json.disabled').is_file())
        original = (self.cache / 'variables.tf').read_text()
        (parent / 'variables.tf').write_text(original + '\n# local editor change\n')
        self.assertEqual(self.invoke('validate'), 0)
        self.assertIn('# local editor change', (parent / 'variables.tf').read_text())
        self.assertEqual((self.cache / 'variables.tf').read_text(), original)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 1)

    def test_nested_ref_upgrade_reuses_cached_files(self):
        self.invoke('init', '-backend=false')
        new_child = Path(self.temporary.name) / 'child-v2'
        new_child.mkdir()
        (new_child / 'outputs.tf').write_text('output "test" { value = "upgraded" }\n')
        self.overrides[self.cache]['module']['child']['source'] = new_child.as_uri()
        self.assertEqual(self.invoke('init', '-backend=false', '-upgrade'), 0)
        snapshot = json.loads((self.root / '.terraform/traveloka-module-sources.json').read_text())
        self.assertEqual(snapshot['example.child']['execution'], new_child.as_uri())
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 2)

    def test_relative_children_resolve_inside_the_local_package_copy(self):
        (self.cache / 'main.tf').unlink()
        service = self.cache / 'modules/service'
        service.mkdir(parents=True)
        (service / 'main.tf').write_text('module "this" { source = "../.." }\n')
        self.overrides = {self.root: {'module': {'example': {'source': self.cache.as_uri() + '//modules/service'}}}}
        self.assertEqual(self.invoke('init', '-backend=false'), 0)
        child = self.root / self.manifest()['example.this']['Dir']
        self.assertTrue(child.is_relative_to(self.root))
        self.assertNotEqual(child.resolve(), self.cache)
        self.assertEqual(self.invoke('validate'), 0)
        self.assertEqual(sum(arguments[0] == 'init' for arguments in self.calls), 1)

    def test_failed_materialization_restores_the_original_symlink(self):
        installed = self.root / 'installed'
        installed.symlink_to(self.cache, target_is_directory=True)
        with patch.object(runner.os, 'replace', side_effect=OSError('rename failed')):
            with self.assertRaisesRegex(OSError, 'rename failed'):
                runner.materialize_installed_module(installed)
        self.assertTrue(installed.is_symlink())
        self.assertEqual(installed.resolve(), self.cache)

    def test_github_shorthand_matches_native_terraform_addresses(self):
        self.assertEqual(runner.editor_module_source('github.com/traveloka/example'),
                         'git::https://github.com/traveloka/example.git')
        self.assertEqual(runner.editor_module_source('github.com/traveloka/example//module?ref=v1'),
                         'git::https://github.com/traveloka/example.git//module?ref=v1')

    @unittest.skipUnless(Path(LS).is_file(), 'Set TEST_TERRAFORM_LS_BINARY to run the LSP check')
    def test_language_server_completes_inputs_in_an_installed_module_copy(self):
        self.child_source = 'git@github.com:traveloka/child.git?ref=v1'
        (self.cache / 'main.tf').write_text('module "child" { source = ' + json.dumps(self.child_source) + ' }\n')
        (self.child / 'variables.tf').write_text('variable "child_input" { default = "cached" }\n')
        self.invoke('init', '-backend=false')
        parent = self.root / self.manifest()['example']['Dir']
        configuration = 'module "child" {\n source = ' + json.dumps(self.child_source) + '\n  chi\n}\n'
        self.assertIn('child_input', asyncio.run(module_completions(parent, configuration)))


if __name__ == '__main__':
    unittest.main()
