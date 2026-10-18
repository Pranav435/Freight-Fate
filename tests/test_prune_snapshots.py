"""tools/prune_snapshots.py: keep the newest tester snapshots, touch nothing else."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


def load_module():
    path = ROOT / "tools" / "prune_snapshots.py"
    spec = importlib.util.spec_from_file_location("prune_snapshots", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_keeps_the_newest_tester_snapshots_and_nothing_else_is_touched():
    prune = load_module()
    tags = [
        "v1.9.3",
        "nightly-20261001",
        "1.9-tester-20261001",
        "1.9-tester-20261018",
        "1.9-tester-20261005",
        "1.9-tester-20261017",
        "v1.8.8.1",
        "1.9-tester-20261010",
        "1.9-tester-20261010-old",
    ]
    assert prune.releases_to_prune(tags, 2) == [
        "1.9-tester-20261010",
        "1.9-tester-20261005",
        "1.9-tester-20261001",
    ]


def test_nothing_to_prune_when_there_are_few():
    prune = load_module()
    assert prune.releases_to_prune(["1.9-tester-20261018", "v1.9.3"], 7) == []


def test_keep_must_leave_at_least_one():
    prune = load_module()
    with pytest.raises(ValueError):
        prune.releases_to_prune(["1.9-tester-20261018"], 0)


def test_the_workflow_prunes_after_publishing_and_keeps_tags():
    workflow = (ROOT / ".github" / "workflows" / "build-career-1.9.yml").read_text("utf-8")
    assert "python3 tools/prune_snapshots.py --keep 7" in workflow
    assert workflow.index("Create prerelease") < workflow.index("tools/prune_snapshots.py")
    assert "--cleanup-tag" not in (ROOT / "tools" / "prune_snapshots.py").read_text("utf-8")
