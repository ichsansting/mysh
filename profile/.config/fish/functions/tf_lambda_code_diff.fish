function tf_lambda_code_diff --description 'Compare Lambda archives from Terraform plan with AWS deployment'
    argparse 'h/help' 'f/full' -- $argv
    or return 2

    if set -q _flag_help
        printf '%s\n' \
            'Usage:' \
            '  terraform plan | tf_lambda_code_diff' \
            '  terraform plan | tf_lambda_code_diff --full' \
            '' \
            'By default, show only the file change summary.' \
            '--full shows file content differences through delta.'
        return 0
    end

    if test (count $argv) -ne 0
        echo 'Error: only the --full flag is supported.' >&2
        return 2
    end

    if isatty stdin
        echo 'Error: pipe Terraform plan output through stdin.' >&2
        echo 'Example: terraform plan | tf_lambda_code_diff' >&2
        return 2
    end

    set -l required_commands aws curl jq python
    if set -q _flag_full
        set --append required_commands git delta
    end

    for dependency in $required_commands
        if not command -q $dependency
            echo "Error: command '$dependency' was not found." >&2
            return 2
        end
    end

    set -l temp_dir (mktemp -d)
    if test $status -ne 0; or test -z "$temp_dir"
        echo 'Error: failed to create a temporary directory.' >&2
        return 2
    end

    set -l full_diff 0
    if set -q _flag_full
        set full_diff 1
    end

    __tf_lambda_code_diff_impl "$temp_dir" "$full_diff"
    set -l result $status

    command rm -rf -- "$temp_dir"
    return $result
end

function __tf_lambda_code_diff_impl --argument-names temp_dir full_diff
    set -l plan_text "$temp_dir/plan.txt"
    set -l parsed_plan "$temp_dir/plan.json"
    set -l identity_json "$temp_dir/identity.json"

    command cat >"$plan_text"
    if test $status -ne 0
        echo 'Error: failed to read Terraform plan output.' >&2
        return 2
    end

    if not test -s "$plan_text"
        echo 'Error: Terraform plan output is empty; check Terraform errors on stderr.' >&2
        return 2
    end

    if not command python -c '
import json
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", text)

plan_complete = re.search(
    r"(?m)^Plan:\s+\d+\s+to add,\s+\d+\s+to change,\s+\d+\s+to destroy\.\s*$",
    text,
)
no_changes = re.search(r"(?m)^No changes\.\s+Your infrastructure matches the configuration\.\s*$", text)

if not plan_complete and not no_changes:
    print("The output does not contain a recognized Terraform plan terminator.", file=sys.stderr)
    sys.exit(2)

if no_changes:
    json.dump({"resources": []}, sys.stdout)
    sys.exit(0)

header_re = re.compile(r"(?m)^\s*#\s+(.+?)\s+will be\s+.+$")
headers = list(header_re.finditer(text))
resources = []

for index, header in enumerate(headers):
    start = header.end()
    end = headers[index + 1].start() if index + 1 < len(headers) else len(text)
    block = text[start:end]

    if not re.search(r"(?m)^\s*[~+\-/]*\s*resource\s+\"aws_lambda_function\"\s+\"[^\"]+\"\s*\{", block):
        continue

    hash_change = re.search(
        r"(?m)^\s*~\s*source_code_hash\s*=\s*\"([^\"]+)\"\s*->\s*\"([^\"]+)\"\s*$",
        block,
    )
    if not hash_change:
        if re.search(r"(?m)^\s*~\s*source_code_hash\s*=.*->", block):
            print(f"The target hash for {header.group(1)} is unknown.", file=sys.stderr)
            sys.exit(2)
        continue

    function_id = re.search(r"(?m)^\s*id\s*=\s*\"([^\"]+)\"\s*$", block)
    function_name = re.search(r"(?m)^\s*function_name\s*=\s*\"([^\"]+)\"(?:\s*->.*)?$", block)
    name = function_id.group(1) if function_id else (function_name.group(1) if function_name else None)
    if not name:
        print(f"The function name for {header.group(1)} was not found.", file=sys.stderr)
        sys.exit(2)

    resources.append({
        "address": header.group(1),
        "function_name": name,
        "before_hash": hash_change.group(1),
        "after_hash": hash_change.group(2),
    })

json.dump({"resources": resources}, sys.stdout)
' "$plan_text" >"$parsed_plan"
        set -l parser_status $status
        if test $parser_status -ne 0
            echo 'Error: Terraform plan output could not be verified.' >&2
            return 2
        end
    end

    if not command jq -e '.resources | type == "array"' "$parsed_plan" >/dev/null
        echo 'Error: the parsed Terraform plan is invalid.' >&2
        return 2
    end

    set -l resource_count (command jq -r '.resources | length' "$parsed_plan")
    if test "$resource_count" -eq 0
        echo 'No Lambda source_code_hash changes were found in the plan.'
        return 0
    end

    set -l region (__tf_lambda_code_diff_region)
    if test $status -ne 0; or test -z "$region"
        echo 'Error: the AWS region could not be determined.' >&2
        echo 'Set AWS_REGION or AWS_DEFAULT_REGION, then retry.' >&2
        return 2
    end

    if not command aws sts get-caller-identity --region "$region" --output json >"$identity_json"
        echo 'Error: the AWS identity could not be verified.' >&2
        return 2
    end

    if not command jq -e '.Account and .Arn' "$identity_json" >/dev/null
        echo 'Error: the sts get-caller-identity response is invalid.' >&2
        return 2
    end

    set -l account_id (command jq -r '.Account' "$identity_json")
    set -l overall_result 0

    for index in (command seq 0 (math "$resource_count - 1"))
        set -l address (command jq -r ".resources[$index].address" "$parsed_plan")
        set -l function_name (command jq -r ".resources[$index].function_name" "$parsed_plan")
        set -l before_hash (command jq -r ".resources[$index].before_hash" "$parsed_plan")
        set -l after_hash (command jq -r ".resources[$index].after_hash" "$parsed_plan")

        set -l local_archives (__tf_lambda_code_diff_find_archives "$after_hash")
        if test $status -ne 0; or test (count $local_archives) -eq 0
            echo "Error: no local archive with the target hash was found for $address." >&2
            echo 'Run terraform plan again from the same directory.' >&2
            return 2
        end
        if test (count $local_archives) -gt 1
            echo "Error: more than one local archive matches $address:" >&2
            printf '  %s\n' $local_archives >&2
            echo 'Remove duplicate archives so the target is unambiguous.' >&2
            return 2
        end
        set -l local_archive "$local_archives[1]"

        set -l resource_dir "$temp_dir/resource-$index"
        command mkdir -p -- "$resource_dir/deployed" "$resource_dir/planned"
        or return 2

        set -l function_json "$resource_dir/function.json"
        if not command aws lambda get-function \
                --function-name "$function_name" \
                --region "$region" \
                --output json >"$function_json"
            echo "Error: the deployment package for $function_name could not be retrieved." >&2
            return 2
        end

        if not command jq -e '.Configuration.CodeSha256 and .Code.Location' "$function_json" >/dev/null
            echo "Error: the get-function response for $function_name is incomplete." >&2
            return 2
        end

        set -l package_type (command jq -r '.Configuration.PackageType // "Zip"' "$function_json")
        if test "$package_type" != Zip
            echo "Error: $function_name uses PackageType=$package_type instead of Zip." >&2
            return 2
        end

        set -l deployed_hash (command jq -r '.Configuration.CodeSha256' "$function_json")
        if test "$deployed_hash" != "$before_hash"
            echo "Error: the plan for $function_name no longer matches the AWS deployment." >&2
            printf '  Initial plan hash: %s\n' "$before_hash" >&2
            printf '  Current AWS hash : %s\n' "$deployed_hash" >&2
            return 2
        end

        set -l code_url (command jq -r '.Code.Location' "$function_json")
        set -l deployed_archive "$resource_dir/deployed.zip"
        if not command curl --fail --silent --show-error --location \
                --proto '=https' --tlsv1.2 \
                --output "$deployed_archive" "$code_url"
            echo "Error: failed to download the deployment package for $function_name." >&2
            return 2
        end
        set --erase code_url

        set -l downloaded_hash (__tf_lambda_code_diff_file_hash "$deployed_archive")
        if test $status -ne 0; or test "$downloaded_hash" != "$deployed_hash"
            echo "Error: the downloaded deployment package hash does not match for $function_name." >&2
            return 2
        end

        set -l report_json "$resource_dir/report.json"
        set -l extract "$full_diff"

        if not __tf_lambda_code_diff_compare_archives \
                "$deployed_archive" "$local_archive" "$report_json" \
                "$resource_dir/deployed" "$resource_dir/planned" "$extract"
            echo "Error: the archive contents for $function_name could not be compared safely." >&2
            return 2
        end

        set -l content_equal (command jq -r '.content_equal' "$report_json")
        set -l archive_equal (command jq -r '.archive_equal' "$report_json")
        set -l added_count (command jq -r '.added | length' "$report_json")
        set -l modified_count (command jq -r '.modified | length' "$report_json")
        set -l deleted_count (command jq -r '.deleted | length' "$report_json")

        if test $index -gt 0
            echo
        end
        printf 'Lambda: %s\n' "$function_name"
        printf 'Address: %s\n' "$address"
        printf 'AWS: %s / %s\n' "$account_id" "$region"
        printf 'Local archive: %s\n' "$local_archive"
        printf 'Deployed: %s\n' "$deployed_hash"
        printf 'Planned : %s\n' "$after_hash"

        if test "$archive_equal" = true
            echo 'Result: IDENTICAL_ARCHIVE'
        else if test "$content_equal" = true
            echo 'Result: METADATA_ONLY'
            echo 'Effective contents are identical; only ZIP metadata or encoding differs.'
        else
            echo 'Result: CONTENT_CHANGED'
            printf 'Summary: %s added, %s modified, %s deleted\n' \
                "$added_count" "$modified_count" "$deleted_count"
            command jq -r '.added[] | "A  \(.)"' "$report_json"
            command jq -r '.modified[] | "M  \(.)"' "$report_json"
            command jq -r '.deleted[] | "D  \(.)"' "$report_json"
            set overall_result 1

            if test "$full_diff" -eq 1
                echo
                echo 'Content diff:'
                command git -C "$resource_dir" -c core.pager=cat diff \
                    --no-index --no-ext-diff --no-color \
                    -- deployed planned \
                    | command delta --paging=never
                set -l diff_status $pipestatus
                if test "$diff_status[1]" -ne 1
                    echo 'Error: git failed to produce the content diff.' >&2
                    return 2
                end
                if test "$diff_status[2]" -ne 0
                    echo 'Error: delta failed to render the diff.' >&2
                    return 2
                end
            end
        end
    end

    return $overall_result
end

function __tf_lambda_code_diff_region
    if set -q AWS_REGION; and test -n "$AWS_REGION"
        echo "$AWS_REGION"
        return 0
    end

    if set -q AWS_DEFAULT_REGION; and test -n "$AWS_DEFAULT_REGION"
        echo "$AWS_DEFAULT_REGION"
        return 0
    end

    set -l configured_region (command aws configure get region 2>/dev/null | string trim)
    if test -n "$configured_region"
        echo "$configured_region"
        return 0
    end

    command python -c '
import re
from pathlib import Path

for path in sorted(Path.cwd().glob("*.tf")):
    text = path.read_text(encoding="utf-8", errors="replace")
    for block in re.finditer(r"backend\s+\"[^\"]+\"\s*\{(.*?)\n\s*\}", text, re.S):
        region = re.search(r"\bregion\s*=\s*\"([^\"]+)\"", block.group(1))
        if region:
            print(region.group(1))
            raise SystemExit(0)
raise SystemExit(1)
'
end

function __tf_lambda_code_diff_find_archives --argument-names expected_hash
    command python -c '
import base64
import hashlib
import os
import sys

expected = sys.argv[1]
matches = []
for root, directories, files in os.walk("."):
    directories[:] = sorted(
        directory for directory in directories
        if directory not in {".git", ".terragrunt-cache"}
    )
    for filename in sorted(files):
        if not filename.endswith(".zip"):
            continue
        path = os.path.join(root, filename)
        try:
            digest = hashlib.sha256()
            with open(path, "rb") as archive:
                for chunk in iter(lambda: archive.read(1024 * 1024), b""):
                    digest.update(chunk)
            encoded = base64.b64encode(digest.digest()).decode("ascii")
        except OSError:
            continue
        if encoded == expected:
            matches.append(path)

for path in sorted(matches, key=lambda value: (len(value), value)):
    print(path)

raise SystemExit(0 if matches else 1)
' "$expected_hash"
end

function __tf_lambda_code_diff_file_hash --argument-names path
    command python -c '
import base64
import hashlib
import sys

try:
    digest = hashlib.sha256()
    with open(sys.argv[1], "rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
except OSError as error:
    print(error, file=sys.stderr)
    raise SystemExit(1)

print(base64.b64encode(digest.digest()).decode("ascii"))
' "$path"
end

function __tf_lambda_code_diff_compare_archives \
        --argument-names deployed_archive planned_archive report_path deployed_dir planned_dir extract
    command python -c '
import hashlib
import json
import os
import shutil
import stat
import sys
import zipfile
from pathlib import Path, PurePosixPath

DEPLOYED_ARCHIVE = Path(sys.argv[1])
PLANNED_ARCHIVE = Path(sys.argv[2])
REPORT_PATH = Path(sys.argv[3])
DEPLOYED_DIR = Path(sys.argv[4])
PLANNED_DIR = Path(sys.argv[5])
EXTRACT = sys.argv[6] == "1"
MAX_TOTAL_SIZE = 1024 * 1024 * 1024
MAX_SYMLINK_SIZE = 16 * 1024


def normalize_name(raw_name):
    if chr(92) in raw_name:
        raise ValueError(f"Ambiguous entry name: {raw_name!r}")
    path = PurePosixPath(raw_name)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"Unsafe entry path: {raw_name!r}")
    normalized = str(path)
    if normalized in ("", "."):
        return None
    return normalized.rstrip("/")


def digest_stream(source):
    digest = hashlib.sha256()
    while True:
        chunk = source.read(1024 * 1024)
        if not chunk:
            break
        digest.update(chunk)
    return digest.hexdigest()


def inspect_archive(path):
    manifest = {}
    total_size = 0
    symlink_targets = {}

    with zipfile.ZipFile(path) as archive:
        bad_member = archive.testzip()
        if bad_member is not None:
            raise ValueError(f"Invalid ZIP CRC: {bad_member}")

        for info in archive.infolist():
            name = normalize_name(info.filename)
            if name is None:
                continue
            if name in manifest:
                raise ValueError(f"Duplicate ZIP entry: {name}")

            total_size += info.file_size
            if total_size > MAX_TOTAL_SIZE:
                raise ValueError("The extracted archive size exceeds the safety limit")

            mode = (info.external_attr >> 16) & 0xFFFF
            permissions = stat.S_IMODE(mode)
            file_type = stat.S_IFMT(mode)

            if info.is_dir() or info.filename.endswith("/"):
                entry_type = "directory"
                content_hash = None
                size = 0
            elif stat.S_ISLNK(mode):
                if info.file_size > MAX_SYMLINK_SIZE:
                    raise ValueError(f"Symlink target is too large: {name}")
                with archive.open(info) as source:
                    target_bytes = source.read(MAX_SYMLINK_SIZE + 1)
                try:
                    target = target_bytes.decode("utf-8")
                except UnicodeDecodeError as error:
                    raise ValueError(f"Symlink target is not UTF-8: {name}") from error
                if "\x00" in target:
                    raise ValueError(f"Symlink target contains NUL: {name}")
                entry_type = "symlink"
                content_hash = hashlib.sha256(target_bytes).hexdigest()
                size = len(target_bytes)
                symlink_targets[name] = target
            elif file_type in (0, stat.S_IFREG):
                entry_type = "file"
                with archive.open(info) as source:
                    content_hash = digest_stream(source)
                size = info.file_size
            else:
                raise ValueError(f"Unsupported ZIP entry type: {name}")

            manifest[name] = {
                "type": entry_type,
                "mode": permissions,
                "size": size,
                "sha256": content_hash,
            }

    return manifest, symlink_targets


def safe_symlink_target(name, target):
    target_path = PurePosixPath(target)
    if target_path.is_absolute():
        raise ValueError(f"Absolute symlink target: {name}")

    parts = []
    for part in PurePosixPath(name).parent.joinpath(target_path).parts:
        if part in ("", "."):
            continue
        if part == "..":
            if not parts:
                raise ValueError(f"Symlink target escapes the extraction root: {name}")
            parts.pop()
        else:
            parts.append(part)


def extract_archive(path, destination, manifest, symlink_targets):
    destination.mkdir(parents=True, exist_ok=True)
    symlink_names = {name for name, item in manifest.items() if item["type"] == "symlink"}

    for name in manifest:
        parent = PurePosixPath(name).parent
        while str(parent) not in ("", "."):
            if str(parent) in symlink_names:
                raise ValueError(f"Parent entry is a symlink: {name}")
            parent = parent.parent

    with zipfile.ZipFile(path) as archive:
        info_by_name = {}
        for info in archive.infolist():
            normalized = normalize_name(info.filename)
            if normalized is not None:
                info_by_name[normalized] = info

        for name, item in manifest.items():
            if item["type"] == "directory":
                (destination / name).mkdir(parents=True, exist_ok=True)

        for name, item in manifest.items():
            if item["type"] != "file":
                continue
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(info_by_name[name]) as source, target.open("xb") as output:
                shutil.copyfileobj(source, output, length=1024 * 1024)
            if item["mode"]:
                target.chmod(item["mode"])

        for name, item in manifest.items():
            if item["type"] != "symlink":
                continue
            target_value = symlink_targets[name]
            safe_symlink_target(name, target_value)
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            os.symlink(target_value, target)

        directories = [
            (name, item) for name, item in manifest.items()
            if item["type"] == "directory" and item["mode"]
        ]
        for name, item in sorted(directories, key=lambda pair: pair[0].count("/"), reverse=True):
            (destination / name).chmod(item["mode"])


def archive_digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


try:
    deployed_manifest, deployed_links = inspect_archive(DEPLOYED_ARCHIVE)
    planned_manifest, planned_links = inspect_archive(PLANNED_ARCHIVE)

    deployed_names = set(deployed_manifest)
    planned_names = set(planned_manifest)
    added = sorted(planned_names - deployed_names)
    deleted = sorted(deployed_names - planned_names)
    modified = sorted(
        name for name in deployed_names & planned_names
        if deployed_manifest[name] != planned_manifest[name]
    )

    content_equal = not added and not deleted and not modified
    archive_equal = archive_digest(DEPLOYED_ARCHIVE) == archive_digest(PLANNED_ARCHIVE)

    if EXTRACT and not content_equal:
        extract_archive(DEPLOYED_ARCHIVE, DEPLOYED_DIR, deployed_manifest, deployed_links)
        extract_archive(PLANNED_ARCHIVE, PLANNED_DIR, planned_manifest, planned_links)

    report = {
        "archive_equal": archive_equal,
        "content_equal": content_equal,
        "added": added,
        "modified": modified,
        "deleted": deleted,
    }
    REPORT_PATH.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
except (OSError, ValueError, zipfile.BadZipFile, RuntimeError) as error:
    print(error, file=sys.stderr)
    raise SystemExit(1)
' "$deployed_archive" "$planned_archive" "$report_path" \
        "$deployed_dir" "$planned_dir" "$extract"
end
