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
    Array(Vector<Value>),
    Object {
        keys: Vector<String>,
        values: Vector<Value>,
    },
}
```

### Functions

```aion
pub fn parse(json: String) -> Option<Value>
pub fn stringify(value: Value) -> String
```

## Implementation Status

- [x] Basic module structure with `Value` enum
- [x] `stringify` for all types (Null, Bool, Number, String)
- [x] Recursive descent parser for primitives (null, true, false, numbers, strings)
- [x] Whitespace handling
- [ ] Array parsing (blocked: type checker cannot resolve Vector methods on non-generic `vector.Vector`)
- [ ] Object parsing (blocked: same as Array)
- [ ] Escape sequence support (`\"`, `\\`, `\n`, `\t`, `\uXXXX`)
- [ ] Error reporting (position, type)

## Language Requirements

The following Aion features are required for a robust implementation:

### Required (Blocking)

1. ~~**Struct return by pointer**: Parser functions need to return `(Value, updated_position)`.~~ **VERIFIED**: `ParseResult { value, pos }` works correctly when returned from functions. Tested in `tests/fixtures/048_struct_return.ai` and `049_parse_result.ai`.

2. ~~**Recursive function calls**: The parser is naturally recursive (objects contain values, arrays contain values).~~ **VERIFIED**: Recursive descent works. The HashMap resize test (`030_collections_extra`) exercises recursion through method calls.

3. **String comparison**: `string.substr()` + `==` works for keyword matching (`null`, `true`, `false`).

4. **Character code comparison**: `string.at()` returns integer char codes. This works for delimiter detection (`"`, `{`, `}`, `[`, `]`, `:`).

### Nice-to-Have (Non-Blocking)

1. ~~**Pattern matching on strings**: Would simplify keyword parsing (e.g., `match chunk { "null" => ..., "true" => ... }`).~~ **VERIFIED**: String pattern matching works. Tested in `tests/fixtures/language/string_match.ai`.

2. ~~**Character literals**: `'{'` instead of `123` for readability.~~ **VERIFIED**: Char literals work. Tested in `tests/fixtures/language/char_literal.ai`.

3. **Float parsing from string**: `string.to_float()` exists and works for number parsing.

## Design Decisions

### Object Representation

Using parallel `Vector<String>` for keys and `Vector<Value>` for values instead of `HashMap` because:
- HashMap had recent bugs (resize issue)
- JSON objects preserve insertion order (spec requirement)
- Simpler implementation for a first pass

Alternative: Implement a dedicated `JsonObject` struct with ordered key-value storage.

### Parser Architecture

Using a recursive descent parser with explicit position tracking (returning `ParseResult { value, pos }` from each function) because:
- Aion doesn't support `&mut` references
- Returning position avoids global state
- Natural fit for JSON's recursive grammar

### Number Parsing

Currently returns `Value::Number(0.0)` as placeholder because:
- Aion's `string.to_float()` may not exist or may be inaccurate
- Need to implement `atof`-style parsing manually, or
- Add `string.to_float()` to the stdlib

## Known Limitations

1. **No float parsing**: `parse_number` returns `0.0` for all numbers. Need `string.from_float` reverse function.
2. **No unicode escape**: `\uXXXX` parsing returns code point as integer, not proper UTF-8 character.
3. **No error positions**: Parser returns `Option::None` without indicating where the error occurred.
4. **No streaming**: Parser loads entire input into memory. Fine for most JSON, but not for huge files.

## Test Cases

```json
// Primitives
"null"     → Value::Null
"true"     → Value::Bool(true)
"false"    → Value::Bool(false)
"42"       → Value::Number(42.0)
"3.14"     → Value::Number(3.14)
"-1"       → Value::Number(-1.0)
"\"hello\"" → Value::String("hello")

// Arrays
"[]"       → Value::Array([])
"[1,2,3]"  → Value::Array([1, 2, 3])
"[\"a\",1]" → Value::Array(["a", 1])

// Objects
"{}"       → Value::Object {}
"{\"k\":\"v\"}" → Value::Object { keys: ["k"], values: ["v"] }

// Nested
"{\"a\":[1,{\"b\":true}]}" → nested structure

// Escape sequences
"\"line1\\nline2\"" → String with newline
"\"tab\\there\""    → String with tab
"\"quote\\\"inside\\\"\"" → String with escaped quotes
```
