# TRACK-012: Fix Broken Tests

## Status
✅ **Done**

## Overview
Fix the 2 failing tests (021_env_args and 022_vector_repro) in the test suite.

## Symptoms
- `021_env_args`: Mismatch - expected "Args count: 0", got wrong value
- `022_vector_repro`: Mismatch - expected "Args count: 0", got "10"

## Root Cause
The runtime is not receiving command-line arguments correctly.

## Plan
See [plan.md](plan.md)