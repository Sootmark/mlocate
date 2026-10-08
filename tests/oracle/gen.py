"""Writes plaso.tsv: what plaso's locate_database parser reads from
tests/fixtures/plaso/mlocate.db (psort JSON lines), one line per directory:
its path, modification time (microseconds since 1970) and entries joined
with "|".

Run: python3 -I tests/oracle/gen.py plaso.jsonl > tests/oracle/plaso.tsv
"""

import json
import sys

lines = []
for line in open(sys.argv[1], encoding="utf-8"):
    event = json.loads(line)
    if event["data_type"] == "linux:locate_database:entry":
        lines.append("\t".join([event["path"], str(event["timestamp"]), "|".join(event.get("entries") or [])]))
for line in sorted(lines):
    print(line)
