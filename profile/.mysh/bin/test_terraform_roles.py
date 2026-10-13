import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from urllib.parse import urlparse
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent / "lib"))
from terraform_roles import configured_role_overrides, read_configuration


class RoleRoutingTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.environment = patch.dict(os.environ, {}, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def write(self, relative, contents):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)
        return path.parent

    def generate(self):
        return configured_role_overrides(self.root, "prod", {})

    def test_only_prod_and_exact_types_are_routed(self):
        self.write("main.tf", '''
resource "aws_kms_key" "managed" {}
data "aws_kms_key" "lookup" { key_id = "alias/example" }
resource "aws_ssm_parameter" "managed" { name = "example" type = "String" value = "test" }
data "aws_ssm_parameter" "lookup" { name = "example" }
resource "aws_kms_alias" "untouched" {}
data "aws_ssm_parameters_by_path" "untouched" { path = "/example" }
''')
        self.assertIsNone(configured_role_overrides(self.root, "stg", {}))
        routing = self.generate()
        for kind in ("resource", "data"):
            self.assertEqual(set(routing[kind]), {"aws_kms_key", "aws_ssm_parameter"})
        self.assertEqual(routing["provider"]["aws"][0]["profile"], "DBAccess@tvlk-fpr-prod")
        self.assertEqual(routing, self.generate())

    def test_rule_environment_profile_and_types_are_configurable(self):
        self.write("main.tf", 'resource "aws_kms_alias" "alias" {}')
        with patch.multiple("terraform_roles", ENVIRONMENT="stg", PROFILE="Other@stg",
                            RESOURCE_TYPES={"aws_kms_alias"}, ALLOWED_ACCOUNT_IDS=["123456789012"]):
            self.assertIsNone(configured_role_overrides(self.root, "prod", {}))
            routing = configured_role_overrides(self.root, "stg", {})
        self.assertEqual(set(routing["resource"]), {"aws_kms_alias"})
        self.assertEqual(routing["provider"]["aws"][0]["profile"], "Other@stg")
        self.assertEqual(routing["provider"]["aws"][0]["allowed_account_ids"], ["123456789012"])

    def test_alias_region_endpoints_and_provider_options_are_preserved(self):
        self.write("main.tf", '''
provider "aws" {
  alias = "regional"
  region = var.region
  profile = "original"
  assume_role { role_arn = "arn:aws:iam::348767535692:role/original" }
  default_tags { tags = { Team = "test" } }
  endpoints { kms = "https://example.invalid" }
}
data "aws_kms_key" "lookup" { provider = aws.regional key_id = "alias/example" }
''')
        settings = next(provider for provider in self.generate()["provider"]["aws"]
                        if provider["alias"] == "traveloka_dbaccess_regional")
        self.assertEqual(settings["region"], "${var.region}")
        self.assertEqual(settings["alias"], "traveloka_dbaccess_regional")
        self.assertNotIn("assume_role", settings)
        self.assertEqual(settings["default_tags"], [{"tags": {"Team": "test"}}])
        self.assertEqual(settings["endpoints"], [{"kms": "https://example.invalid"}])

    def test_nested_modules_retain_provider_mappings_and_constraints(self):
        self.write("main.tf", '''
provider "aws" { alias = "regional" region = "ap-southeast-1" }
module "outer" { source = "./outer" providers = { aws = aws.regional random = random } }
''')
        self.write("outer/main.tf", '''
terraform { required_providers { aws = { source = "hashicorp/aws" version = ">= 5" } } }
module "inner" { source = "./inner" }
''')
        inner = self.write("outer/inner/main.tf", 'data "aws_kms_key" "lookup" { key_id = "alias/example" }')
        routing = self.generate()
        outer = Path(urlparse(routing["module"]["outer"]["source"]).path)
        self.assertNotEqual(outer, self.root / "outer")
        self.assertFalse((inner / "zzzz_traveloka_roles_override.tf.json").exists())
        mappings = routing["module"]["outer"]["providers"]
        self.assertEqual(mappings["aws"], "aws.regional")
        self.assertEqual(mappings["random"], "random")
        self.assertEqual(mappings["aws.traveloka_dbaccess"], "aws.traveloka_dbaccess_regional")
        # Inspect generated configuration directly; the loader excludes its own file.
        generated = json.loads((outer / "zzzz_traveloka_roles_override.tf.json").read_text())
        requirements = generated["terraform"]["required_providers"]["aws"]
        self.assertEqual(requirements["version"], ">= 5")
        self.assertEqual(requirements["configuration_aliases"], ["aws.traveloka_dbaccess"])
        self.assertEqual(routing, self.generate())

    def test_cached_package_ancestry_is_preserved(self):
        package = self.root / "cache/repository"
        package.mkdir(parents=True)
        (package / ".git").write_text("gitdir: ignored")
        self.write("cache/repository/shared.txt", "asset")
        self.write("cache/repository/nested/main.tf", 'data "aws_kms_key" "lookup" { key_id = "alias/example" }')
        self.write("main.tf", f'module "child" {{ source = "{package.as_uri()}//nested" }}')
        source = self.generate()["module"]["child"]["source"]
        self.assertTrue(source.endswith("//nested"))
        copied = Path(urlparse(source).path)
        self.assertEqual((copied.parent / "shared.txt").read_text(), "asset")
        self.assertFalse((package / "nested/zzzz_traveloka_roles_override.tf.json").exists())

    def test_module_updates_change_snapshot_and_deleted_files_do_not_linger(self):
        self.write("main.tf", 'module "child" { source = "./child" }')
        self.write("child/main.tf", 'data "aws_kms_key" "lookup" { key_id = "alias/example" }')
        self.write("child/old.txt", "old")
        before = Path(urlparse(self.generate()["module"]["child"]["source"]).path)
        (self.root / "child/old.txt").unlink()
        after = Path(urlparse(self.generate()["module"]["child"]["source"]).path)
        self.assertNotEqual(before, after)
        self.assertTrue((before / "old.txt").exists())
        self.assertFalse((after / "old.txt").exists())

    def test_json_configuration_and_custom_data_directory(self):
        self.write("main.tf.json", json.dumps({"module": {"child": {"source": "./child"}}}))
        self.write("child/main.tf.json", json.dumps({"data": {"aws_ssm_parameter": {"lookup": {"name": "example"}}}}))
        with patch.dict(os.environ, {"TF_DATA_DIR": "custom"}):
            destination = Path(urlparse(self.generate()["module"]["child"]["source"]).path)
        self.assertTrue(destination.is_relative_to(self.root / "custom/traveloka-role-modules"))

    def test_comments_and_example_strings_are_not_routed(self):
        self.write("main.tf", '''
# data "aws_kms_key" "example" {}
locals { example = "data \\"aws_ssm_parameter\\" \\"example\\" {}" }
''')
        routing = self.generate()
        self.assertNotIn("resource", routing)
        self.assertNotIn("data", routing)

    def test_malformed_configuration_reports_the_file(self):
        self.write("main.tf", 'resource "aws_kms_key" "broken" {')
        with self.assertRaisesRegex(ValueError, "Cannot parse .*main.tf"):
            self.generate()

    def test_reserved_alias_and_legacy_provider_fail_before_execution(self):
        self.write("main.tf", '''
provider "aws" { alias = "traveloka_dbaccess" }
data "aws_kms_key" "lookup" { key_id = "alias/example" }
''')
        with self.assertRaisesRegex(ValueError, "reserved"):
            self.generate()
        self.write("main.tf", 'module "child" { source = "./child" }')
        self.write("child/main.tf", '''
provider "aws" { region = "ap-southeast-1" }
data "aws_kms_key" "lookup" { key_id = "alias/example" }
''')
        with self.assertRaisesRegex(ValueError, "inherited providers"):
            self.generate()


if __name__ == "__main__":
    unittest.main()
