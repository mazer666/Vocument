#!/usr/bin/env bash
# Names the tests of Gradle that failed, with their class and their reason.
#
# The summary of Gradle says only *that* there were failures ("There were
# failing tests. See the report at …"). The report itself is written as XML
# and names every test, its class and the reason of the failure — and that
# is what a reader of a failed run needs.
#
# The report is printed to the standard output, one failure per line. The
# caller of the script decides what to do with it (the workflow of the
# continuous integration writes the lines as annotations of the run ✗).
#
# Usage: scripts/report-gradle-test-failures.sh [directory of the XML reports]
#
# Exit code 0: the report was read (whether it names failures or not).
# Exit code 1: the directory of the reports does not exist.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

REPORT_DIRECTORY="${1:-apps/android/app/build/test-results/testDebugUnitTest}"

if [[ ! -d "$REPORT_DIRECTORY" ]]; then
  echo "The directory of the reports does not exist: $REPORT_DIRECTORY"
  exit 1
fi

python3 - "$REPORT_DIRECTORY" <<'PYTHON'
import pathlib
import sys
import xml.etree.ElementTree as ET

directory = pathlib.Path(sys.argv[1])
reports = sorted(directory.glob("*.xml"))
if not reports:
    print("There is no report of the tests in the directory.")
    raise SystemExit(0)

failures = 0
for report in reports:
    try:
        root = ET.parse(report).getroot()
    except ET.ParseError as error:
        print(f"{report.name}: the report could not be read ({error})")
        continue
    for case in root.iter("testcase"):
        for problem in list(case.iter("failure")) + list(case.iter("error")):
            failures += 1
            message = (problem.get("message") or "").strip().replace("\n", " ")[:220]
            print(f"{case.get('classname')}::{case.get('name')} — {message}")

if failures == 0:
    print("No test failed — the failure is somewhere else.")
PYTHON
