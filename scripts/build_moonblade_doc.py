import subprocess

FUNCTIONS = subprocess.run(
    ["./target/debug/xan", "map", "--functions"],
    text=True,
    capture_output=True
).stdout

print(FUNCTIONS)