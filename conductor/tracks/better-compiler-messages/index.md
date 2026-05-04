# TRACK-015: Better Compiler Error Messages

## Status
⏳ **Pending**

## Overview
Improve compiler error messages to be more user-friendly, include source locations, and provide actionable suggestions.

## Current Issues
- Basic error messages without line/column info
- No "did you mean" suggestions for typos
- No code snippets in error output

## Goals
- Add line and column numbers to all errors
- Show code snippet around error location
- Add common suggestions (e.g., "did you mean X?")

## Plan
See [plan.md](plan.md)