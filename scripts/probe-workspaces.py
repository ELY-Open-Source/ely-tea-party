#!/usr/bin/env python3
"""Read project identities, or exercise worktree isolation in a disposable repo."""

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def git(folder, *args):
    result = subprocess.run(
        ["git", "-C", str(folder), *args], capture_output=True, text=True, timeout=15
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or f"git {args} failed")
    return result.stdout.strip()


def inspect(folder):
    folder = Path(folder).resolve(strict=True)
    root = Path(git(folder, "rev-parse", "--show-toplevel")).resolve()
    common = Path(git(folder, "rev-parse", "--path-format=absolute", "--git-common-dir")).resolve()
    branch = git(folder, "rev-parse", "--abbrev-ref", "HEAD")
    return {"project": str(common), "checkout": str(root), "branch": branch}


def group(folders):
    projects = {}
    for folder in folders:
        entry = inspect(folder)
        checkouts = projects.setdefault(entry.pop("project"), [])
        if entry not in checkouts:
            checkouts.append(entry)
    return [{"project": project, "checkouts": checkouts} for project, checkouts in projects.items()]


def runtimes():
    result = {}
    for name in ("hc", "herdr"):
        candidates = [shutil.which(name), str(Path.home() / ".local/bin" / name)]
        executable = next((p for p in candidates if p and Path(p).is_file()), None)
        if not executable:
            result[name] = {"installed": False}
            continue
        probe = subprocess.run([executable, "--version"], capture_output=True, text=True, timeout=5)
        result[name] = {
            "installed": True, "version": probe.stdout.strip(),
            "exit_code": probe.returncode, "persistence_verified": False,
        }
    return result


def self_test():
    with tempfile.TemporaryDirectory(prefix="ely-workspace-probe-") as temporary:
        base = Path(temporary)
        repo, isolated, other = base / "project", base / "worker", base / "other"
        repo.mkdir()
        git(repo, "init", "-q", "-b", "main")
        (repo / "sample.txt").write_text("original\n")
        (repo / "nested").mkdir()
        git(repo, "add", "sample.txt")
        git(repo, "-c", "user.name=ElyProbe", "-c", "user.email=probe@localhost",
            "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
        git(repo, "worktree", "add", "-qb", "task", str(isolated))
        (isolated / "sample.txt").write_text("worker edit\n")
        linked = base / "linked"
        linked.symlink_to(repo, target_is_directory=True)
        grouped = group([repo, repo / "nested", linked, isolated])
        assert len(grouped) == 1 and len(grouped[0]["checkouts"]) == 2
        assert (repo / "sample.txt").read_text() == "original\n"
        assert git(repo, "status", "--porcelain") == ""
        assert "sample.txt" in git(isolated, "status", "--porcelain")
        git(base, "clone", "-q", str(repo), str(other))
        assert len(group([repo, isolated, other])) == 2
        git(isolated, "checkout", "--detach", "-q")
        assert inspect(isolated)["branch"] == "HEAD"
    return {
        "passed": ["subdirectory grouping", "symlink grouping", "shared project across worktrees",
                   "separate checkout identity", "isolated edits", "independent clone", "detached HEAD"],
        "application_ui_changed": False,
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folders", nargs="*")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    output = {"runtimes": runtimes()}
    if args.self_test:
        output["worktree_probe"] = self_test()
    if args.folders:
        output["projects"] = group(args.folders)
    print(json.dumps(output, ensure_ascii=False, indent=2))
