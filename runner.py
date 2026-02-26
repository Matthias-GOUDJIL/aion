#!/usr/bin/env python3
import subprocess
import sys
import os
import re
from pathlib import Path

# Config
# Script is now at project root, so parent is the root itself
PROJECT_ROOT = Path(__file__).parent.resolve()
WRAPPER_SCRIPT = PROJECT_ROOT / "aion"
FIXTURES = PROJECT_ROOT / "tests/fixtures"
EXPECTED = PROJECT_ROOT / "tests/expected"

EXPECTED_FAILURES = ["005_unsafe_check"]

def run():
    if not WRAPPER_SCRIPT.exists():
        print(f"❌ Wrapper script not found: {WRAPPER_SCRIPT}")
        sys.exit(1)
        
    tests = sorted(list(FIXTURES.glob("*.ai")))
    passed = 0
    
    # Make sure wrapper is executable
    os.chmod(WRAPPER_SCRIPT, 0o755)

    for t in tests:
        name = t.stem
        exp_file = EXPECTED / f"{name}.out"
        rel_test_path = t.relative_to(PROJECT_ROOT)
        
        # Run using the wrapper
        # ./aion run tests/fixtures/xxx.ai
        cmd = [str(WRAPPER_SCRIPT), "run", str(rel_test_path)]
        
        try:
            # Timeout increased to 60s for stability
            res = subprocess.run(cmd, cwd=PROJECT_ROOT, capture_output=True, text=True, timeout=60)
        except subprocess.TimeoutExpired:
            print(f"❌ {name}: Execution Timeout")
            continue
            
        if res.returncode != 0:
            if name in EXPECTED_FAILURES:
                print(f"✅ {name} (Expected Fail)")
                passed += 1
                continue
            print(f"❌ {name}: Run Fail\n{res.stderr}\n{res.stdout}")
            continue
            
        # Parse output to extract the actual program stdout
        full_output = res.stdout
        match = re.search(r'-{31}\n(.*?)\n-{31}', full_output, re.DOTALL)
        
        if match:
            actual = match.group(1).strip()
        else:
            # If we expected output but didn't find delimiters, it's a fail
            # (unless it's an intentional compiler error test)
            if not exp_file.exists() or os.path.getsize(exp_file) > 0:
                print(f"❌ {name}: Could not parse output format (no delimiters)")
                print(f"Full Output:\n{full_output}")
                continue
            actual = ""

        # Check
        if not exp_file.exists():
            with open(exp_file, "w") as f: f.write(actual)
            print(f"⚠️ {name}: Created baseline")
            passed += 1
        else:
            with open(exp_file, "r") as f: expected = f.read().strip()
            if actual == expected:
                print(f"✅ {name}")
                passed += 1
            else:
                print(f"❌ {name}: Mismatch\nExp: '{expected}'\nGot: '{actual}'")

    print(f"Result: {passed}/{len(tests)} passed")
    sys.exit(0 if passed == len(tests) else 1)

if __name__ == "__main__":
    run()
