#!/usr/bin/env python3
import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Optional, Set, Tuple


ROOT = Path(__file__).resolve().parents[1]
VERSION_RE = re.compile(r"^v?(?P<version>[0-9]+\.[0-9]+\.[0-9]+)$")


def write_output(name: str, value) -> None:
    text = str(value).lower() if isinstance(value, bool) else str(value)
    output_path = os.environ.get("GITHUB_OUTPUT")

    if output_path:
        with open(output_path, "a", encoding="utf-8") as f:
            f.write(f"{name}={text}\n")
    else:
        print(f"{name}={text}")


def read_workspace_version() -> str:
    in_workspace_package = False

    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines():
        if line.startswith("[") and line.endswith("]"):
            in_workspace_package = line == "[workspace.package]"
            continue

        if in_workspace_package:
            match = re.match(r'version = "([^"]+)"', line)
            if match:
                return match.group(1)

    raise RuntimeError("Could not find [workspace.package] version in Cargo.toml")


def read_cargo_lock_version() -> str:
    package = {}

    for line in (ROOT / "Cargo.lock").read_text(encoding="utf-8").splitlines():
        if line == "[[package]]":
            if package.get("name") == "hyfetch":
                return package["version"]
            package = {}
            continue

        match = re.match(r'(name|version) = "([^"]+)"', line)
        if match:
            package[match.group(1)] = match.group(2)

    if package.get("name") == "hyfetch":
        return package["version"]

    raise RuntimeError("Could not find hyfetch package version in Cargo.lock")


def read_python_version() -> str:
    content = (ROOT / "hyfetch" / "__version__.py").read_text(encoding="utf-8")
    match = re.search(r"^VERSION = ['\"]([^'\"]+)['\"]$", content, re.MULTILINE)
    if not match:
        raise RuntimeError("Could not find VERSION in hyfetch/__version__.py")

    return match.group(1)


def read_package_json() -> dict:
    return json.loads((ROOT / "package.json").read_text(encoding="utf-8"))


def read_pyproject_name() -> str:
    content = (ROOT / "pyproject.toml").read_text(encoding="utf-8")
    match = re.search(r'^name = "([^"]+)"$', content, re.MULTILINE)
    if not match:
        raise RuntimeError("Could not find project name in pyproject.toml")

    return match.group(1)


def read_changelog(version: str) -> str:
    content = (ROOT / "README.md").read_text(encoding="utf-8")
    heading = f"### {version}"
    match = re.search(
        rf"^{re.escape(heading)}\s*\n(?P<body>.*?)(?=^### \S|\Z)",
        content,
        flags=re.MULTILINE | re.DOTALL,
    )

    if not match:
        raise RuntimeError(f"Could not find changelog section {heading!r} in README.md")

    body = match.group("body").strip()
    if not body:
        raise RuntimeError(f"Changelog section {heading!r} is empty")

    return body + "\n"


def normalize_tag(tag: str) -> Tuple[str, str]:
    match = VERSION_RE.match(tag)
    if not match:
        raise RuntimeError(
            f"Release tag {tag!r} is not supported. Use a plain version tag like 2.1.1 or v2.1.1."
        )

    return tag, match.group("version")


def expected_python_assets(version: str) -> Set[str]:
    package = "hyfetch"
    platforms = [
        "any",
        "win_amd64",
        "manylinux_2_31_x86_64",
        "manylinux_2_31_aarch64",
        "manylinux_2_31_armv7l",
        "musllinux_1_1_x86_64",
        "macosx_11_0_x86_64",
        "macosx_11_0_arm64",
    ]

    assets = {f"{package}-{version}.tar.gz"}
    assets.update(f"{package}-{version}-py3-none-{platform}.whl" for platform in platforms)
    return assets


def http_json_status(url: str, token: Optional[str] = None) -> Tuple[int, Optional[dict]]:
    headers = {
        "Accept": "application/json",
        "User-Agent": "hyfetch-release-workflow",
    }

    if token:
        headers["Authorization"] = f"Bearer {token}"

    request = urllib.request.Request(url, headers=headers)

    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = response.read().decode("utf-8")
            return response.status, json.loads(payload) if payload else None
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return 404, None
        raise


def pypi_version_exists(package: str, version: str) -> bool:
    package_path = urllib.parse.quote(package)
    version_path = urllib.parse.quote(version)
    status, _ = http_json_status(f"https://pypi.org/pypi/{package_path}/{version_path}/json")
    return status == 200


def npm_version_exists(package: str, version: str) -> bool:
    package_path = urllib.parse.quote(package, safe="@")
    version_path = urllib.parse.quote(version)
    status, _ = http_json_status(f"https://registry.npmjs.org/{package_path}/{version_path}")
    return status == 200


def crates_version_exists(package: str, version: str) -> bool:
    package_path = urllib.parse.quote(package)
    version_path = urllib.parse.quote(version)
    status, _ = http_json_status(f"https://crates.io/api/v1/crates/{package_path}/{version_path}")
    return status == 200


def github_release_state(repo: str, tag: str, version: str) -> Tuple[bool, bool]:
    repo_path = urllib.parse.quote(repo, safe="/")
    tag_path = urllib.parse.quote(tag, safe="")
    token = os.environ.get("GITHUB_TOKEN")
    status, payload = http_json_status(
        f"https://api.github.com/repos/{repo_path}/releases/tags/{tag_path}",
        token=token,
    )

    if status == 404:
        return False, False

    if not payload:
        raise RuntimeError(f"GitHub release lookup for {repo}@{tag} returned no data")

    asset_names = {asset["name"] for asset in payload.get("assets", [])}
    return True, expected_python_assets(version).issubset(asset_names)


def command_metadata(args: argparse.Namespace) -> None:
    tag, version = normalize_tag(args.tag)
    package_json = read_package_json()

    versions = {
        "Cargo.toml": read_workspace_version(),
        "Cargo.lock": read_cargo_lock_version(),
        "hyfetch/__version__.py": read_python_version(),
        "package.json": package_json["version"],
    }
    mismatches = {path: found for path, found in versions.items() if found != version}

    if mismatches:
        details = ", ".join(f"{path} has {found}" for path, found in mismatches.items())
        raise RuntimeError(f"Release tag version {version} does not match checked-in versions: {details}")

    write_output("tag", tag)
    write_output("version", version)
    write_output("python_package", read_pyproject_name())
    write_output("npm_package", package_json["name"])
    write_output("cargo_package", "hyfetch")


def command_state(args: argparse.Namespace) -> None:
    pypi_exists = pypi_version_exists(args.python_package, args.version)
    npm_exists = npm_version_exists(args.npm_package, args.version)
    crates_exists = crates_version_exists(args.cargo_package, args.version)
    github_release_exists, github_release_complete = github_release_state(args.repo, args.tag, args.version)

    write_output("pypi_exists", pypi_exists)
    write_output("npm_exists", npm_exists)
    write_output("crates_exists", crates_exists)
    write_output("github_release_exists", github_release_exists)
    write_output("github_release_complete", github_release_complete)


def command_changelog(args: argparse.Namespace) -> None:
    _, version = normalize_tag(args.version)
    changelog = read_changelog(version)

    if args.output:
        Path(args.output).write_text(changelog, encoding="utf-8")
    else:
        print(changelog, end="")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Release workflow helpers")
    subparsers = parser.add_subparsers(dest="command", required=True)

    metadata = subparsers.add_parser("metadata", help="Validate release tag and checked-in versions")
    metadata.add_argument("--tag", required=True)
    metadata.set_defaults(func=command_metadata)

    state = subparsers.add_parser("state", help="Check whether release stages already completed")
    state.add_argument("--tag", required=True)
    state.add_argument("--version", required=True)
    state.add_argument("--python-package", required=True)
    state.add_argument("--npm-package", required=True)
    state.add_argument("--cargo-package", default="hyfetch")
    state.add_argument("--repo", required=True)
    state.set_defaults(func=command_state)

    changelog = subparsers.add_parser("changelog", help="Extract release notes from README.md")
    changelog.add_argument("--version", required=True)
    changelog.add_argument("--output")
    changelog.set_defaults(func=command_changelog)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    try:
        args.func(args)
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
