use rust_decimal::Decimal;

pub trait CapitalQuantum: Send + Sync {
    fn notional(&self) -> Decimal;
    fn token(&self) -> &str;
    fn venue(&self) -> &str;
}
