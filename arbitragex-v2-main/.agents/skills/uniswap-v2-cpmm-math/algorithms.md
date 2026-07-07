## V2 Amount Out Calculator
### Pseudocódigo (Rust style)
```rust
fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256, fee: u32) -> Result<U256, MathError> {
    let amount_in_with_fee = amount_in * (10000 - fee);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = (reserve_in * 10000) + amount_in_with_fee;
    Ok(numerator / denominator)
}
```
