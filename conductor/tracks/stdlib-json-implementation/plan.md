# TRACK-014 Plan: Stdlib JSON Implementation

## Phase 1: String Parsing Utilities
- [x] 1. Implement `skip_whitespace()` helper
- [x] 2. Implement `read_string()` with escape handling
- [x] 3. Implement `read_number()` for integers and floats

## Phase 2: Core Parser Implementation
- [x] 4. Implement `parse_value()` - dispatches based on first character
- [x] 5. Implement `parse_null()`, `parse_bool()`
- [x] 6. Implement `parse_number()`
- [x] 7. Implement `parse_string()`
- [x] 8. Implement `parse_array()` (stub)
- [x] 9. Implement `parse_object()` (stub)

## Phase 3: Integration
- [x] 10. Connect `parse()` function to the parser
- [x] 11. Test with various JSON inputs
- [x] 12. Create test fixture for JSON parsing

## Phase 4: Known Limitations
- Pattern matching on `Option<Value>` with custom enum inner type not fully supported
- Workaround: use `result.is_some()` instead of `match`
- String escaping not yet implemented

## Result
Basic JSON parsing works for: null, true, false, numbers, strings
Arrays and objects return Null (stubs)