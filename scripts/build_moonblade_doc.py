import re
import subprocess

FUNCTION_RE = re.compile(
    r"\s{4} - ([a-z0-9_]+)\(((?:[a-z0-9=?*_<>]+\s*,?\s*)*)\) -> ([a-z\[\]?,| ]+)",
    re.I
)

FUNCTIONS = subprocess.run(
    ["./target/debug/xan", "map", "--functions"],
    text=True,
    capture_output=True
).stdout

for match in FUNCTION_RE.findall(FUNCTIONS):
    print(match)