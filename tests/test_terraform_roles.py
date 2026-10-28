from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "profile/.mysh/bin/lib"))
import terraform_roles


class ModuleProviderMappingsTest(unittest.TestCase):
    def test_only_routed_providers_are_inferred(self):
        mappings = self.generate_mappings({})
        self.assertEqual(mappings, {
            "aws": "aws",
            "aws.traveloka_override_aws_ssm_parameter": "aws.traveloka_override_aws_ssm_parameter",
        })

    def test_explicit_mappings_are_preserved(self):
        mappings = self.generate_mappings({"providers": {"aws": "${aws.prod}", "local": "local"}})
        self.assertEqual(mappings, {
            "aws": "aws.prod",
            "local": "local",
            "aws.traveloka_override_aws_ssm_parameter": "aws.traveloka_override_aws_ssm_parameter_prod",
        })

    def generate_mappings(self, settings):
        root = Path("/example")
        child = {
            "needed": {("aws", "aws_ssm_parameter")},
            "configuration": {
                "required_providers": {},
                "resource": {"aws_ssm_parameter": {}},
                "data": {"local_file": {}, "terraform_remote_state": {}},
            },
        }
        node = {
            "directory": root,
            "configuration": {"module": {"lambda": settings}},
            "children": {"lambda": child},
            "source_overrides": {},
            "needed": child["needed"],
            "rules": [],
        }
        with patch.object(terraform_roles, "add_resource_routing"), \
                patch.object(terraform_roles, "add_root_providers"), \
                patch.object(terraform_roles, "generate_module", return_value="./child"):
            override = terraform_roles.module_override(node, root, {}, root, root)
        return override["module"]["lambda"]["providers"]
