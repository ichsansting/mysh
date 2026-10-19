import contextlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

TF = os.environ.get('TEST_TERRAFORM_BINARY', str(Path.home() / '.mysh/mise/installs/terraform/1.7.5/terraform'))
spec = importlib.util.spec_from_file_location('terraform_runner', Path(__file__).with_name('terraform_runner.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


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


if __name__ == '__main__':
    unittest.main()
