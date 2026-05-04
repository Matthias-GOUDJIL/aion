# TRACK-012 Plan: Fix Broken Tests

## Investigation
- [x] 1. Run test 021 manually to see actual output
- [x] 2. Check `src/runtime.c` for argument handling (`aion_argc`, `aion_argv`)
- [x] 3. Verify `std.env` module correctly reads arguments
- [x] 4. Compare with working test 020_env_var to understand difference

## Root Cause
During the file renaming, the expected output files were incorrectly assigned:
- `021_env_args.out` had the content from test 020 (env vars)
- `022_vector_repro.out` had the content from test 021 (args count)

## Fixes Applied
- [x] Corrected `021_env_args.out` to "Args count: 0"
- [x] Corrected `022_vector_repro.out` to "10"

## Verification
- [x] Run `python3 runner.py` - all 46 tests pass ✅