from concurrent.futures import ThreadPoolExecutor
from functools import cache
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import shlex
import subprocess
import sys
import tempfile
from urllib.parse import parse_qs

PLUGIN_CACHE = Path(os.environ.get("TF_PLUGIN_CACHE_DIR", str(Path.home() / ".terraform.d/plugin-cache"))).expanduser().resolve()

# Skip comments and heredocs so example text cannot become a module override.
TOKENS = re.compile(
    r'(?P<comment>\#[^\n]*|//[^\n]*|/\*.*?\*/)'
    r'|(?P<heredoc><<-?(?P<marker>[\w-]+)[^\n]*\n.*?^\s*(?P=marker)\s*$)'
    r'|(?P<string>"(?:\\.|[^"\\])*")'
    r'|(?P<token>[\w-]+|[^\s])', re.MULTILINE | re.DOTALL,
)
TRAVELOKA = re.compile(
    r'^(?:git::)?(?:git@github\.com:|ssh://git@github\.com/|https://github\.com/|github\.com/)'
    r'traveloka/([^/?]+)(?://([^?]*))?(?:\?(.*))?$'
)


# shortcut: literal sources only; use an HCL parser if computed sources are adopted.
def native_module_sources(configuration):
    modules = {}
    tokens = [match.group() for match in TOKENS.finditer(configuration)
              if match.lastgroup in ("string", "token")]
    depth = 0
    module = None
    for index, token in enumerate(tokens):
        if (depth == 0 and token == "module" and index + 2 < len(tokens)
                and tokens[index + 1].startswith('"') and tokens[index + 2] == "{"):
            module = json.loads(tokens[index + 1])
        if (depth == 1 and module and token == "source" and index + 2 < len(tokens)
                and tokens[index + 1] == "=" and tokens[index + 2].startswith('"')):
            modules[module] = json.loads(tokens[index + 2])
        depth += (token == "{") - (token == "}")
        if depth == 0 and token == "}":
            module = None
    return modules


def module_sources(directory):
    modules = {}
    filenames = sorted([*directory.glob("*.tf"), *directory.glob("*.tf.json")])
    filenames.sort(key=lambda path: path.name in ("override.tf", "override.tf.json")
                   or path.name.endswith(("_override.tf", "_override.tf.json")))
    for filename in filenames:
        configuration = filename.read_text()
        if filename.name.endswith(".json"):
            module_blocks = json.loads(configuration).get("module", {})
            modules.update({name: settings["source"] for name, settings in module_blocks.items()
                            if "source" in settings})
            continue
        modules.update(native_module_sources(configuration))
    return modules


def expand_sparse_module(directory):
    existing_parent = directory
    while not existing_parent.is_dir() and existing_parent != existing_parent.parent:
        existing_parent = existing_parent.parent
    git_root = subprocess.run(["git", "-C", str(existing_parent), "rev-parse", "--show-toplevel"],
                              capture_output=True, text=True)
    if git_root.returncode:
        return
    checkout = Path(git_root.stdout.strip()).resolve()
    relative_path = directory.relative_to(checkout)
    if relative_path == Path("."):
        return
    sparse = subprocess.run(["git", "-C", str(checkout), "config", "--bool", "core.sparseCheckout"],
                            capture_output=True, text=True)
    if sparse.stdout.strip() == "true":
        subprocess.run(["git", "-C", str(checkout), "sparse-checkout", "add", "--", str(relative_path)], check=True)


@cache
def ref_worktree(checkout, reference, cache_directory, upgrade):
    reference_cache = cache_directory / checkout.parent.name / hashlib.sha256(reference.encode()).hexdigest()
    if reference_cache.is_file() and not upgrade:
        commit = reference_cache.read_text().strip()
    else:
        subprocess.run(["git", "-C", str(checkout), "fetch", "--no-tags", "--", "origin", reference], check=True)
        commit = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "--verify", "FETCH_HEAD^{commit}"], text=True).strip()
    module_cache = checkout.parent / "refs"
    module_cache.mkdir(parents=True, exist_ok=True)
    cached_worktree = module_cache / commit
    if cached_worktree.is_dir():
        worktree = cached_worktree
    else:
        branch = "tf-module-" + commit
        existing_branch = subprocess.run(["git", "-C", str(checkout), "show-ref", "--verify", "--quiet", "refs/heads/" + branch])
        if existing_branch.returncode not in (0, 1):
            existing_branch.check_returncode()
        worktree_setting = "worktree-path=" + json.dumps(str(cached_worktree), ensure_ascii=False)
        command = ["wt", "-C", str(checkout), "--config-set", worktree_setting,
                   "switch", branch, "--no-hooks", "--no-cd", "--format=json"]
        if existing_branch.returncode == 1:
            command.extend(["--create", "--base", commit])
        worktree = Path(json.loads(subprocess.check_output(command, text=True))["path"]).resolve()
    actual_commit = subprocess.check_output(["git", "-C", str(worktree), "rev-parse", "HEAD"], text=True).strip()
    if actual_commit != commit:
        raise ValueError(f"Worktree {worktree} has moved away from requested ref {reference}; expected {commit}")
    if worktree != cached_worktree:
        if worktree == checkout.resolve():
            raise ValueError(f"Cannot move the main checkout into the module cache: {worktree}")
        subprocess.run(["git", "-C", str(checkout), "worktree", "move", str(worktree), str(cached_worktree)], check=True)
        worktree = cached_worktree
    reference_cache.parent.mkdir(parents=True, exist_ok=True)
    descriptor, filename = tempfile.mkstemp(dir=reference_cache.parent)
    try:
        with os.fdopen(descriptor, "w") as cached_reference:
            cached_reference.write(commit)
        os.replace(filename, reference_cache)
    finally:
        Path(filename).unlink(missing_ok=True)
    return worktree


def resolve_module_path(checkout, subdirectory, source):
    module_path = (checkout / (subdirectory or "")).resolve()
    if not module_path.is_relative_to(checkout.resolve()):
        raise ValueError(f"Module path escapes Traveloka checkout: {source}")
    return module_path


def module_override(directory, source, cache_directory, upgrade):
    match = TRAVELOKA.fullmatch(source)
    if not match:
        if source.startswith(("./", "../", "/")):
            return None, directory / source
        return None, None
    repository = match[1].removesuffix(".git")
    if repository in ("", ".", ".."):
        raise ValueError(f"Invalid Traveloka repository: {source}")
    checkout = PLUGIN_CACHE.parent / "module-cache" / repository / "checkout"
    module_path = resolve_module_path(checkout, match[2], source)
    if not checkout.exists():
        checkout.parent.mkdir(parents=True, exist_ok=True)
        print(f"terraform local: cloning traveloka/{repository} -> {checkout}", file=sys.stderr)
        subprocess.run(["gh", "repo", "clone", f"traveloka/{repository}", str(checkout),
                        "--", "--depth", "1", "--filter=blob:none", "--sparse"], check=True)
    reference = parse_qs(match[3] or "").get("ref", ["HEAD"])[0]
    checkout = ref_worktree(checkout, reference, cache_directory, upgrade)
    module_path = resolve_module_path(checkout, match[2], source)
    local_source = checkout.resolve().as_uri()
    subdirectory = module_path.relative_to(checkout.resolve())
    # Preserve repository ancestry for nested modules that reference ../../.
    if subdirectory != Path("."):
        local_source += "//" + subdirectory.as_posix()
    return {"source": local_source}, module_path


def repository_overrides(directory, sources, cache_directory, upgrade):
    # One queue owns each repository because fetches share FETCH_HEAD and worktree metadata.
    return {name: module_override(directory, source, cache_directory, upgrade) for name, source in sources.items()}


def directory_overrides(directory, cache_directory, upgrade):
    replacements = {}
    children = []
    sources = module_sources(directory)
    if not sources:
        return replacements, children
    queues = {}
    for name, source in sources.items():
        match = TRAVELOKA.fullmatch(source)
        repository = match[1].removesuffix(".git") if match else None
        queues.setdefault(repository, {})[name] = source
    resolved_sources = {}
    with ThreadPoolExecutor(max_workers=len(queues)) as executor:
        futures = [executor.submit(repository_overrides, directory, queue, cache_directory, upgrade) for queue in queues.values()]
        for future in futures:
            resolved_sources.update(future.result())
    for name in sources:
        settings, module_path = resolved_sources[name]
        if settings is not None:
            replacements[name] = settings
        if module_path is not None:
            children.append(module_path)
    return replacements, children


def local_overrides(root, upgrade=False):
    # Keep each working directory on its selected commits until explicitly upgraded.
    cache_directory = (root / (os.environ.get("TF_DATA_DIR") or ".terraform") / "traveloka-module-refs").resolve()
    pending = [root]
    visited = set()
    overrides = {}
    while pending:
        directory = pending.pop().resolve()
        if directory in visited:
            continue
        visited.add(directory)
        expand_sparse_module(directory)
        if not directory.is_dir():
            raise ValueError(f"Missing local module directory: {directory}")
        replacements, children = directory_overrides(directory, cache_directory, upgrade)
        pending.extend(children)
        if replacements:
            overrides[directory] = {"module": replacements}
    return overrides


def module_sources_match(configured, installed):
    if configured.startswith(("./", "../", "/")):
        return os.path.normpath(configured) == os.path.normpath(installed)
    return configured == installed


def modules_need_init(root):
    data_directory = root / (os.environ.get("TF_DATA_DIR") or ".terraform")
    try:
        manifest = json.loads((data_directory / "modules/modules.json").read_text())
        installed_modules = {module["Key"]: module for module in manifest["Modules"]}
    except (FileNotFoundError, json.JSONDecodeError, KeyError, TypeError):
        return True
    pending = [("", root)]
    visited = set()
    while pending:
        parent_key, directory = pending.pop()
        if not directory.is_dir():
            return True
        visited.add(parent_key)
        sources = module_sources(directory)
        children = {f"{parent_key}.{name}" if parent_key else name: source
                    for name, source in sources.items()}
        if any(key not in installed_modules or not module_sources_match(source, installed_modules[key]["Source"])
               for key, source in children.items()):
            return True
        pending.extend((key, (root / installed_modules[key]["Dir"]).resolve()) for key in children)
    return visited != installed_modules.keys()


def role_modules_installed(root):
    data_directory = (root / (os.environ.get("TF_DATA_DIR") or ".terraform")).resolve()
    try:
        manifest = json.loads((data_directory / "modules/modules.json").read_text())
        source_prefix = (data_directory / "traveloka-role-modules").as_uri() + "/"
        return any(module["Source"].startswith(source_prefix) for module in manifest["Modules"])
    except (FileNotFoundError, json.JSONDecodeError, KeyError, TypeError):
        return False


def backend_needs_init(root, filename):
    # shortcut: scalar backend settings only; use an HCL parser for nested settings.
    tokens = [match.group() for match in TOKENS.finditer((root / filename).read_text())
              if match.lastgroup in ("string", "token")]
    if len(tokens) % 3 or any(tokens[index + 1] != "=" for index in range(0, len(tokens), 3)):
        raise ValueError(f"Cannot compare non-scalar backend settings in {filename}; run explicit init")
    settings = {tokens[index]: json.loads(tokens[index + 2]) for index in range(0, len(tokens), 3)}
    data_directory = root / (os.environ.get("TF_DATA_DIR") or ".terraform")
    try:
        cached_backend = json.loads((data_directory / "terraform.tfstate").read_text())["backend"]["config"]
    except (FileNotFoundError, json.JSONDecodeError, KeyError, TypeError):
        return True
    return any(cached_backend.get(key) != setting for key, setting in settings.items())


def modules_upgrade_requested(command, arguments):
    flag = {"init": "-upgrade", "get": "-update"}.get(command)
    if flag is None:
        return False
    options = shlex.split(os.environ.get("TF_CLI_ARGS", ""))
    options += shlex.split(os.environ.get("TF_CLI_ARGS_" + command, "")) + arguments
    settings = [option for option in options if option.split("=", 1)[0] == flag]
    return bool(settings) and settings[-1].partition("=")[2] in ("", "true", "1", "t", "T", "TRUE", "True")


def apply_uses_saved_plan(arguments):
    options_with_values = {"-backup", "-invoke", "-lock-timeout", "-parallelism", "-replace",
                           "-state", "-state-out", "-target", "-var", "-var-file"}
    remaining = iter(arguments)
    for argument in remaining:
        if argument in options_with_values:
            next(remaining, None)
            continue
        if not argument.startswith("-"):
            return True
    return False


def initialize(terraform, arguments, backend_file):
    chdir = arguments[:1] if arguments[0].startswith("-chdir=") else []
    init_arguments = ["init", "-input=false"]
    if backend_file is not None:
        init_arguments.append("-backend-config=" + backend_file)
    return subprocess.call(terraform + chdir + init_arguments, stdout=sys.stderr)


def role_overrides(root, environment, overrides):
    from terraform_roles import configured_role_overrides
    return configured_role_overrides(root, environment, overrides)


def main():
    version, environment, *arguments = sys.argv[1:]
    use_role_overrides = "--override" in arguments
    arguments = [argument for argument in arguments if argument != "--override"]
    root = Path.cwd()
    command_arguments = arguments[:]
    if command_arguments and command_arguments[0].startswith("-chdir="):
        root = (root / command_arguments.pop(0).split("=", 1)[1]).resolve()
    command = command_arguments[0] if command_arguments else ""
    terraform = ["mise", "x", f"terraform@{version}", "--", "terraform"]
    configuration_commands = {"init", "get", "plan", "apply", "destroy", "validate",
                              "console", "import", "refresh", "graph", "test", "providers", "show"}
    uses_configuration = command in configuration_commands or command_arguments[:2] == ["state", "show"]
    if not uses_configuration or any(arg in ("-help", "--help") for arg in arguments):
        return subprocess.call(terraform + arguments)
    if environment:
        explicit_options = {argument.split("=", 1)[0] for argument in command_arguments[1:]}
        default_option = None
        if command == "init" and "-backend-config" not in explicit_options:
            default_option = f"-backend-config=var-{environment}-backend.tfvars"
        elif command in ("plan", "apply") and "-var-file" not in explicit_options:
            default_option = f"-var-file=var-{environment}.tfvars"
        # Saved plans already contain their variables.
        if command == "apply" and apply_uses_saved_plan(command_arguments[1:]):
            default_option = None
        if default_option:
            arguments.insert(2 if arguments[0].startswith("-chdir=") else 1, default_option)
    backend_file = f"var-{environment}-backend.tfvars" if environment else None
    if (backend_file is not None and command not in ("init", "plan", "apply")
            and not (root / backend_file).is_file()):
        backend_file = None
    backend_changed = (command not in ("init", "get") and backend_file is not None
                       and backend_needs_init(root, backend_file))
    overrides = local_overrides(root, modules_upgrade_requested(command, command_arguments[1:]))
    temporary_files = []
    routing = None
    try:
        for directory, configuration in overrides.items():
            descriptor, filename = tempfile.mkstemp(prefix="traveloka_local_", suffix="_override.tf.json", dir=directory)
            temporary_files.append(filename)
            with os.fdopen(descriptor, "w") as override:
                json.dump(configuration, override)
            for name, settings in configuration["module"].items():
                print(f"terraform local: {directory}: {name} -> {settings['source']}", file=sys.stderr)
        if use_role_overrides:
            from terraform_roles import ModulesNotInstalled
            try:
                routing = role_overrides(root, environment, overrides)
            except ModulesNotInstalled:
                status = initialize(terraform, arguments, backend_file)
                if status:
                    return status
                routing = role_overrides(root, environment, overrides)
            if routing:
                descriptor, filename = tempfile.mkstemp(prefix="traveloka_roles_", suffix=".tf.json", dir=root)
                temporary_files.append(filename)
                with os.fdopen(descriptor, "w") as provider_configuration:
                    json.dump({"provider": routing.pop("provider")}, provider_configuration)
                descriptor, filename = tempfile.mkstemp(prefix="zzzz_traveloka_roles_", suffix="_override.tf.json", dir=root)
                temporary_files.append(filename)
                with os.fdopen(descriptor, "w") as override:
                    json.dump(routing, override)
                print(f"terraform roles: YAML overrides enabled for {environment}", file=sys.stderr)
        if command not in ("init", "get") and (backend_changed or
                ((overrides or routing or role_modules_installed(root)) and modules_need_init(root))):
            status = initialize(terraform, arguments, backend_file)
            if status:
                return status
        return subprocess.call(terraform + arguments)
    finally:
        for filename in temporary_files:
            Path(filename).unlink(missing_ok=True)


def terminate(signum, frame):
    raise SystemExit(128 + signum)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, terminate)
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        print(f"terraform local: command failed: {error}", file=sys.stderr)
        sys.exit(error.returncode if error.returncode > 0 else 128 - error.returncode)
    except (OSError, ValueError) as error:
        print(f"terraform local: {error}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
