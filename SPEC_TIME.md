# Aion Specification: Time & Duration

## 1. The `Duration` Type
Aion introduces a first-class `Duration` primitive type representing a span of time.

### Internal Representation
Under the hood, `Duration` is a 128-bit structure (to avoid Y2038 issues and allow nanosecond precision for thousands of years).
```rust
struct Duration {
    secs: u64,
    nanos: u32
}
```

### Syntax (Literals)
Aion supports suffixed literals for creating durations directly.
```aion
let t1 = 5s      // 5 seconds
let t2 = 500ms   // 500 milliseconds
let t3 = 10us    // 10 microseconds
let t4 = 2h      // 2 hours
let t5 = 30m     // 30 minutes
```

### Operations
| Left | Op | Right | Result | Description |
|---|---|---|---|---|
| `Duration` | `+` | `Duration` | `Duration` | Sum of spans |
| `Duration` | `-` | `Duration` | `Duration` | Difference (saturating at 0) |
| `Duration` | `*` | `int/float` | `Duration` | Scaling |
| `Duration` | `/` | `int/float` | `Duration` | Scaling |
| `Duration` | `/` | `Duration` | `float` | Ratio (how many X in Y) |
| `Duration` | `==` | `Duration` | `bool` | Equality |
| `Duration` | `>` | `Duration` | `bool` | Comparison |

## 2. The `Time` Type (Future)
Represents a point in time (Timestamp).
*   `Time + Duration -> Time`
*   `Time - Duration -> Time`
*   `Time - Time -> Duration`

## 3. Elo Compatibility
To support Elo's ISO8601 strings, the standard library will provide:
```aion
io.println(Duration("P1DT2H")) // Parses string at runtime
```

## 4. Implementation Status
- [x] **Lexer**: Recognize suffixes `s`, `ms`, `us`, `ns`, `m`, `h` and `DYYYY-MM-DD`.
- [x] **AST**: Add `Expression::Duration(u64, u32)` and `Expression::Date(i64)`.
- [x] **Type Checker**: Support for `Date + Duration` arithmetic.
- [x] **Codegen**:
    *   `Duration` stored as milliseconds in `i64`.
    *   `Date` stored as millisecond timestamp in `i64`.
    *   Arithmetic intrinsics implemented in `src/lib.rs`.
