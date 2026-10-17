# std.json — JSON Parser & Serializer

## Purpose

Provide JSON parsing and serialization for Aion, enabling interoperability with web APIs, configuration files, and data exchange formats.

## API

### Types

```aion
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(vector.Vector<Value>),
    Object(ordered_map.OrderedMap<Value>),
}
```

### Functions

```aion
pub fn parse(json: String) -> option.Option<Value>
pub fn stringify(value: Value) -> String
pub fn array_len(value: Value) -> i64
pub fn array_get(value: Value, index: i64) -> option.Option<Value>
pub fn object_len(value: Value) -> i64
pub fn object_get(value: Value, key: String) -> option.Option<Value>
```

## Implementation Status

- [x] Basic module structure with `Value` enum
- [x] `stringify` for all types (`Null`, `Bool`, `Number`, `String`, `Array`, `Object`)
- [x] Recursive descent parser for primitives (`null`, `true`, `false`, numbers, strings)
- [x] Whitespace handling and strict trailing-token rejection
- [x] Recursive array parsing (`Value::Array(Vector<Value>)`)
- [x] Recursive object parsing (`Value::Object(OrderedMap<Value>)`, insertion-order preserving, duplicate keys overwrite)
- [x] Escape sequence support (`\"`, `\\`, `\/`, `\n`, `\r`, `\t`, `\b`, `\f`) in both `parse` and `stringify`
- [x] Safe accessors (`array_len`, `array_get`, `object_len`, `object_get`)
- [ ] Unicode escape sequences (`\uXXXX`)
- [ ] Detailed error reporting (line/column position, error kind)

## Language Requirements

The following Aion features are required for a robust implementation:

### Required (Blocking)

1. ~~**Struct return by pointer**: Parser functions need to return `(Value, updated_position)`.~~ **VERIFIED**: `ParseStep { ok, value, next_pos }` works correctly when returned from functions.

2. ~~**Recursive function calls**: The parser is naturally recursive (objects contain values, arrays contain values).~~ **VERIFIED**: Recursive descent works across nested `Value::Array` and `Value::Object` structures.

3. **String comparison**: `string.substr()` + `string.equals()` works for keyword matching (`null`, `true`, `false`).

4. **Character code comparison**: `string.at()` returns integer char codes. This works for delimiter detection (`"`, `{`, `}`, `[`, `]`, `:`, `,`).

### Nice-to-Have (Non-Blocking)

1. ~~**Pattern matching on strings**: Would simplify keyword parsing (e.g., `match chunk { "null" => ..., "true" => ... }`).~~ **VERIFIED**: String pattern matching works. Tested in `tests/fixtures/language/string_match.ai`.

2. ~~**Character literals**: `'{'` instead of `123` for readability.~~ **VERIFIED**: Char literals work. Tested in `tests/fixtures/language/char_literal.ai`.

3. **Float parsing from string**: `string.to_float()` exists and works for number parsing.

## Design Decisions

### Object Representation

Using `ordered_map.OrderedMap<Value>` (`std.collections.ordered_map`) for `Value::Object`:
- Preserves key insertion order for deterministic `stringify` output across runs
- Duplicate keys overwrite the value in place while preserving the first-seen key order
- Single-payload enum variant `Value::Object(OrderedMap<Value>)` integrates cleanly with Aion's enum representation

### Parser Architecture

Using a recursive descent parser with explicit position tracking (`ParseStep { ok: bool, value: Value, next_pos: i64 }` and `StringStep { ok: bool, text: String, next_pos: i64 }`):
- Returning `ok` + `next_pos` avoids global mutable state and cleanly propagates syntax errors (`Option::None` at top level)
- Top-level `parse` verifies that only whitespace follows the root value (`end_p == string.len(json)`), rejecting trailing garbage or truncated inputs

### Number Parsing

`parse_number_step` scans optional leading `-`, integer digits, and optional fractional `.digits`, rejecting lone `-` or trailing `.` without digits, then converts the slice via `string.to_float(num_str)` (`aion_str_to_float` in `src/runtime.c`) and wraps it in `Value::Number(num_val)`.

## Known Limitations

1. **No `\uXXXX` unicode escape**: Standard single-character escapes (`\"`, `\\`, `\/`, `\n`, `\r`, `\t`, `\b`, `\f`) are supported; 4-hex-digit `\uXXXX` escapes are not decoded yet.
2. **No error positions**: `parse` returns `Option::None` on malformed input without line/column diagnostics.
3. **No streaming**: Parser loads entire input into memory.

## Test Cases

See `tests/fixtures/stdlib/json_parse_basic.ai` and `tests/fixtures/stdlib/json_complex.ai`.
