"""Generate configured role routing without editing shared module caches."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import tempfile
from urllib.parse import unquote, urlparse

import hcl2
from lark.exceptions import UnexpectedInput

from terraform_override_rules import ALLOWED_ACCOUNT_IDS, ENVIRONMENT, PROFILE, RESOURCE_TYPES
ALIAS = "traveloka_dbaccess"
GENERATED = "zzzz_traveloka_roles_override.tf.json"
OPTIONS = hcl2.utils.SerializationOptions(
    with_comments=False, explicit_blocks=False, preserve_heredocs=False, strip_string_quotes=True,
)
AUTH_SETTINGS = {
    "alias", "profile", "access_key", "secret_key", "token", "assume_role",
    "assume_role_with_web_identity", "shared_config_files", "shared_credentials_files",
}


class ModulesNotInstalled(ValueError):
    """The original module graph must be initialized before it can be copied."""


def reference(expression):
    return expression[2:-1] if expression.startswith("${") and expression.endswith("}") else expression


def routed_reference(original):
    if original == "aws":
        return "aws." + ALIAS
    if original.startswith("aws."):
        return "aws." + ALIAS + "_" + original[4:]
    raise ValueError(f"Cannot route non-AWS provider {original}")


def blocks(document, kind):
    declarations = document.get(kind, [])
    return declarations if isinstance(declarations, list) else [declarations]


def read_configuration(directory):
    configuration = {"module": {}, "resource": {}, "data": {}, "providers": {}, "required_providers": {}}
    filenames = sorted([*directory.glob("*.tf"), *directory.glob("*.tf.json")])
    filenames.sort(key=lambda path: path.name in ("override.tf", "override.tf.json")
                   or path.name.endswith(("_override.tf", "_override.tf.json")))
    for filename in filenames:
        if filename.name == GENERATED or filename.name.startswith("traveloka_local_"):
            continue
        contents = filename.read_text()
        try:
            document = json.loads(contents) if filename.name.endswith(".json") else hcl2.loads(contents, serialization_options=OPTIONS)
        except UnexpectedInput as error:
            raise ValueError(f"Cannot parse {filename}:{error.line}:{error.column}") from None
        merge_configuration(configuration, document)
    return configuration


def merge_configuration(configuration, document):
    for declaration in blocks(document, "module"):
        for name, settings in declaration.items():
            configuration["module"].setdefault(name, {}).update(settings)
    for kind in ("resource", "data"):
        for declaration in blocks(document, kind):
            merge_resources(configuration[kind], declaration)
    for declaration in blocks(document, "provider"):
        merge_providers(configuration["providers"], declaration)
    for declaration in blocks(document, "terraform"):
        merge_requirements(configuration["required_providers"], declaration)


def merge_resources(existing, declaration):
    for resource_type, resources in declaration.items():
        for name, settings in resources.items():
            existing.setdefault(resource_type, {}).setdefault(name, {}).update(settings)


def merge_providers(existing, declaration):
    for name, provider_settings in declaration.items():
        settings_list = provider_settings if isinstance(provider_settings, list) else [provider_settings]
        for settings in settings_list:
            provider = name + ("." + settings["alias"] if "alias" in settings else "")
            existing.setdefault(provider, {}).update(settings)


def merge_requirements(existing, declaration):
    for requirements in blocks(declaration, "required_providers"):
        for name, settings in requirements.items():
            existing.setdefault(name, {}).update(settings)


def local_source(directory, source):
    if source.startswith("file://"):
        return Path(unquote(urlparse(source).path.replace("//", "/"))).resolve()
    if source.startswith(("./", "../", "/")):
        return (directory / source).resolve()
    return None


def resolve_child(root, directory, source, entry, key):
    child = local_source(directory, source)
    if child is not None:
        return child
    if entry is None or entry["Source"] != source:
        raise ModulesNotInstalled(f"Module {key} must be installed before role routing")
    return (root / entry["Dir"]).resolve()


def discover_modules(root, directory, source_overrides, installed, key="", active=()):
    directory = directory.resolve()
    if directory in active:
        raise ValueError(f"Recursive module source while generating role routing: {directory}")
    if not directory.is_dir():
        raise ValueError(f"Missing module directory: {directory}")
    configuration = read_configuration(directory)
    children = {}
    for name, settings in configuration["module"].items():
        child_key = f"{key}.{name}" if key else name
        override = source_overrides.get(directory, {}).get("module", {}).get(name, {})
        source = override.get("source", settings["source"])
        child = resolve_child(root, directory, source, installed.get(child_key), child_key)
        children[name] = discover_modules(root, child, source_overrides, installed, child_key, (*active, directory))
    needed = target_providers(configuration)
    for name, child in children.items():
        mappings = configuration["module"][name].get("providers", {})
        needed.update(reference(mappings.get(provider, provider)) for provider in child["needed"])
    if key and needed and any(provider.startswith("aws") for provider in configuration["providers"]):
        raise ValueError(f"Module {key} configures its own AWS provider; role routing requires inherited providers")
    return {"directory": directory, "configuration": configuration, "children": children, "needed": needed,
            "source_overrides": source_overrides.get(directory, {}).get("module", {})}


def target_providers(configuration):
    needed = set()
    for kind in ("resource", "data"):
        for resource_type in RESOURCE_TYPES:
            resources = configuration[kind].get(resource_type, {})
            needed.update(reference(settings.get("provider", "aws")) for settings in resources.values())
    return needed


def ignored(directory, names):
    return [name for name in names if name in (".git", ".terraform", GENERATED, ".terraform.lock.hcl")
            or name.startswith("traveloka_local_")]


def package_root(directory, data_directory):
    # Copy the whole downloaded package to preserve ../../ modules and file paths.
    for parent in (directory, *directory.parents):
        if (parent / ".git").is_file():
            return parent
    installed = data_directory / "modules"
    if directory.is_relative_to(installed):
        return installed / directory.relative_to(installed).parts[0]
    return directory


def snapshot(package, cache_directory):
    digest = hashlib.sha256(str(package).encode())
    for directory, folders, filenames in os.walk(package):
        folders[:] = sorted(set(folders) - set(ignored(directory, folders)))
        for filename in sorted(set(filenames) - set(ignored(directory, filenames))):
            path = Path(directory) / filename
            digest.update(str(path.relative_to(package)).encode() + b"\0")
            digest.update(path.read_bytes())
    destination = cache_directory / digest.hexdigest()
    if destination.is_dir():
        return destination
    cache_directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=cache_directory) as temporary:
        staged = Path(temporary) / "module"
        shutil.copytree(package, staged, ignore=ignored)
        try:
            staged.rename(destination)
        except FileExistsError:
            pass
    return destination


def inherited_providers(configuration):
    names = set(configuration["required_providers"])
    for kind in ("resource", "data"):
        names.update(resource_type.split("_", 1)[0] for resource_type in configuration[kind])
    return {name: name for name in names}


def module_override(node, root, copies, data_directory, cache_directory):
    configuration = node["configuration"]
    override = {}
    add_resource_routing(override, configuration)
    child_overrides = dict(node["source_overrides"])
    for name, child in node["children"].items():
        if not child["needed"]:
            continue
        child_source = generate_module(child, root, copies, data_directory, cache_directory)
        settings = configuration["module"][name]
        mappings = {name: reference(provider) for name, provider in
                    settings.get("providers", inherited_providers(child["configuration"])).items()}
        for provider in child["needed"]:
            source = reference(mappings.get(provider, provider))
            mappings[routed_reference(provider)] = routed_reference(source)
        child_overrides[name] = {"source": child_source, "providers": mappings}
    if child_overrides:
        override["module"] = child_overrides
    if node["directory"] == root:
        add_root_providers(override, configuration, node["needed"])
    else:
        requirements = dict(configuration["required_providers"].get("aws", {"source": "hashicorp/aws"}))
        aliases = [reference(alias) for alias in requirements.get("configuration_aliases", [])]
        requirements["configuration_aliases"] = sorted(set(aliases) | {routed_reference(provider) for provider in node["needed"]})
        override["terraform"] = {"required_providers": {"aws": requirements}}
    return override


def add_resource_routing(override, configuration):
    for kind in ("resource", "data"):
        declarations = resource_overrides(configuration[kind])
        if declarations:
            override[kind] = declarations


def resource_overrides(configuration):
    declarations = {}
    for resource_type in RESOURCE_TYPES:
        resources = configuration.get(resource_type, {})
        if resources:
            declarations[resource_type] = {name: {"provider": routed_reference(reference(settings.get("provider", "aws")))}
                                           for name, settings in resources.items()}
    return declarations


def add_root_providers(override, configuration, needed):
    providers = []
    for original in sorted(needed):
        alias = routed_reference(original)[4:]
        if "aws." + alias in configuration["providers"]:
            raise ValueError(f"Provider alias {alias} is reserved for generated DBAccess routing")
        if original != "aws" and original not in configuration["providers"]:
            raise ValueError(f"Missing original AWS provider configuration: {original}")
        settings = configuration["providers"].get(original, {})
        provider = {name: setting for name, setting in settings.items() if name not in AUTH_SETTINGS}
        providers.append(dict(provider, alias=alias, profile=PROFILE, allowed_account_ids=ALLOWED_ACCOUNT_IDS))
    override["provider"] = {"aws": providers}


def generate_module(node, root, copies, data_directory, cache_directory):
    directory = node["directory"]
    package = package_root(directory, data_directory)
    if package not in copies:
        copies[package] = snapshot(package, cache_directory)
    destination = copies[package] / directory.relative_to(package)
    override = module_override(node, root, copies, data_directory, cache_directory)
    descriptor, filename = tempfile.mkstemp(dir=destination, suffix=".json")
    try:
        with os.fdopen(descriptor, "w") as generated:
            json.dump(override, generated)
        os.replace(filename, destination / GENERATED)
    finally:
        Path(filename).unlink(missing_ok=True)
    source = copies[package].as_uri()
    subdirectory = directory.relative_to(package)
    return source if subdirectory == Path(".") else source + "//" + subdirectory.as_posix()


def configured_role_overrides(root, environment, source_overrides):
    if environment != ENVIRONMENT:
        return None
    root = root.resolve()
    data_directory = (root / (os.environ.get("TF_DATA_DIR") or ".terraform")).resolve()
    try:
        manifest = json.loads((data_directory / "modules/modules.json").read_text())
        installed = {entry["Key"]: entry for entry in manifest["Modules"]}
    except FileNotFoundError:
        installed = {}
    tree = discover_modules(root, root, source_overrides, installed)
    # Keep aliases available for destroying resources removed from configuration.
    tree["needed"].add("aws")
    tree["needed"].update(name for name in tree["configuration"]["providers"] if name.startswith("aws."))
    return module_override(tree, root, {}, data_directory, data_directory / "traveloka-role-modules")
