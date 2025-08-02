# Math Crate

A simple math utility crate that provides basic mathematical operations.

## Functions

### `add(a: i32, b: i32) -> i32`

Adds two integers together and returns the result.

#### Example

```rust
use math::add;

let result = add(2, 3);
assert_eq!(result, 5);
```

## Usage

Add this crate to your `Cargo.toml`:

```toml
[dependencies]
math = { path = "crates/math" }
```

Then use it in your code:

```rust
use math::add;

fn main() {
    let sum = add(10, 20);
    println!("10 + 20 = {}", sum);
}
```

## Running Tests

```bash
cargo test
```
