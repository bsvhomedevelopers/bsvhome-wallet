#!/usr/bin/env python3
import json
from pathlib import Path
import shlex
import subprocess
import sys

if len(sys.argv) != 3 or sys.argv[1] not in {"test", "build-for-testing", "test-without-building"}:
    raise SystemExit("Usage: test-unit.py ACTION TARGET (run from ios/)")
action, target = sys.argv[1:]
families = {}
for scheme, plan in [("BsvHomePackages", "unit_frameworks"), ("BsvHome", "unit_hosted")]:
    rows = json.loads(Path(f"BsvHomeTests/{plan}.xctestplan").read_text())["testTargets"]
    families[scheme] = {row["target"]["name"] for row in rows}
if families["BsvHome"] & families["BsvHomePackages"]:
    raise SystemExit("A unit target is registered in both test families")
try:
    tokens = shlex.split(target)
except ValueError as error:
    raise SystemExit(f"Invalid test selector quoting: {error}") from None
selectors = []
while tokens:
    selector = tokens.pop(0)
    if selector == "-only-testing":
        if not tokens:
            raise SystemExit("Missing selector after -only-testing")
        selector = tokens.pop(0)
    elif selector.startswith("-only-testing:"):
        selector = selector.removeprefix("-only-testing:")
    elif selectors or selector.startswith("-") or "=" in selector:
        raise SystemExit("Use TARGET plus -only-testing selectors; custom flags/settings require _test")
    if not selector or any(not part for part in selector.split("/")) or "=" in selector:
        raise SystemExit("Invalid test selector; custom flags/settings require _test")
    if selector.split("/", 1)[0] not in set().union(*families.values()):
        raise SystemExit(f"Unknown registered unit-test target: {selector.split('/', 1)[0]}")
    if selector not in selectors:
        selectors.append(selector)
for scheme, targets in families.items():
    chosen = [s for s in selectors if s.split("/", 1)[0] in targets]
    if selectors and not chosen:
        continue
    argument = " -only-testing:".join(shlex.quote(s) for s in chosen)
    result = subprocess.run(["just", "_test", action, argument, "YES", scheme])
    if result.returncode:
        raise SystemExit(result.returncode)
