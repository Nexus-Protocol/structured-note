use cosmwasm_std::{Decimal, Uint128};

const DECIMAL_FRACTIONAL: Uint128 = Uint128::new(1_000_000_000u128);

pub fn decimal_multiplication(arg_1: Decimal, arg_2: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL * arg_1 * arg_2, DECIMAL_FRACTIONAL)
}

pub fn decimal_division(num: Decimal, denom: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL * num, DECIMAL_FRACTIONAL * denom)
}

pub fn reverse_decimal(decimal: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL, decimal * DECIMAL_FRACTIONAL)
}
