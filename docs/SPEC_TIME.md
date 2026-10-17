# Aion Specification: Time & Duration

## 1. The `Duration` and `Date` Types
Aion provides first-class `Duration` (a span of time) and `Date` (a calendar timestamp) primitive types.

### Internal Representation (Current Implementation)
Both `Duration` and `Date` are represented at the AST (`Expression::Duration(i64, Span)`, `Expression::Date(i64, Span)`) and LLVM IR levels as signed 64-bit integers (`i64`) storing **milliseconds**:
- `Duration`: signed `i64` milliseconds (`1s` = `1000`, `2m` = `120000`, `1h` = `3600000`, `1d` = `86400000`). Sub-millisecond suffixes (`us`, `ns`) are recognized by the lexer and currently truncate to `0` at millisecond resolution.
- `Date`: signed `i64` epoch milliseconds at midnight UTC (`D2024-01-01`).

*(Note: A future 128-bit `{ secs: u64, nanos: u32 }` representation is planned if nanosecond precision is required.)*

### Syntax (Literals)
Aion supports suffixed literals for creating durations and `DYYYY-MM-DD` literals for dates directly:
```aion
let t1 = 5s          // 5 seconds (5000 ms)
let t2 = 500ms       // 500 milliseconds
let t3 = 10us        // 10 microseconds (truncates to 0 ms in v0.6)
let t4 = 2h          // 2 hours
let t5 = 30m         // 30 minutes
let t6 = 1d          // 1 day
let d1 = D2024-01-01 // UTC date literal
```

### Operations
| Left | Op | Right | Result | Description |
|---|---|---|---|---|
| `Duration` | `+` | `Duration` | `Duration` | Sum of spans |
| `Duration` | `-` | `Duration` | `Duration` | Difference |
| `Duration` | `/` | `Duration` | `f64` | Ratio (`sitofp` + `fdiv`) |
| `Duration` | `==`, `!=`, `<`, `>`, `<=`, `>=` | `Duration` | `bool` | Comparison |
| `Date` | `+` | `Duration` | `Date` | Offset date forward |
| `Date` | `-` | `Duration` | `Date` | Offset date backward |

## 2. Implementation Status
- [x] **Lexer**: Recognize suffixes `s`, `ms`, `us`, `ns`, `m`, `h`, `d` and `DYYYY-MM-DD` (in both Rust `src/lexer/mod.rs` and self-hosted `compiler/lexer.ai`).
- [x] **AST**: `Expression::Duration(i64, Span)` and `Expression::Date(i64, Span)`.
- [x] **Type Checker**: Support for `Date` and `Duration` arithmetic and comparisons.
- [x] **Codegen**:
    *   `Duration` stored as milliseconds in `i64`.
    *   `Date` stored as millisecond timestamp in `i64`.
    *   SQL transpiler lowers `Duration` to `INTERVAL` and `Date` to `TIMESTAMP`.
- [x] **Runtime & Stdlib (`std.time`, `std.date`)**:
    *   `aion_time_now()`, `aion_time_sleep(ms)`, and `aion_date_{year,month,day,hour,min,sec}(ts_ms)` implemented in `src/runtime.c` and registered in `src/builtins.rs` (`#199`).

