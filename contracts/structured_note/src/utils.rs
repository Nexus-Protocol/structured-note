use cosmwasm_std::{Decimal, Event, Fraction, StdError, StdResult, Uint128};

// Math
const DECIMAL_FRACTIONAL: Uint128 = Uint128::new(1_000_000_000u128);

pub fn decimal_multiplication(arg_1: &Decimal, arg_2: &Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL * (*arg_1) * (*arg_2), DECIMAL_FRACTIONAL)
}

pub fn decimal_division(num: Decimal, denom: Decimal) -> StdResult<Decimal> {
    if denom * DECIMAL_FRACTIONAL <= Uint128::zero() {
        return Err(StdError::generic_err("Division by zero"));
    }

    Ok(Decimal::from_ratio(DECIMAL_FRACTIONAL * num, DECIMAL_FRACTIONAL * denom))
}

pub fn reverse_decimal(decimal: Decimal) -> Decimal {
    Decimal::from_ratio(DECIMAL_FRACTIONAL, decimal * DECIMAL_FRACTIONAL)
}

//Response parsing
pub fn get_amount_from_response_raw_attr(events: Vec<Event>, raw_attr_name: String) -> StdResult<String> {
    events
        .into_iter()
        .map(|event| event.attributes)
        .flatten()
        .find(|attr| attr.key == raw_attr_name.clone())
        .map(|attr| attr.value)
        .ok_or_else(|| {
            StdError::generic_err(format!("Attr '{}' not found", &raw_attr_name))
        })
}

pub fn get_amount_from_response_asset_as_string_attr(events: Vec<Event>, attr_name: String) -> StdResult<String> {
    let attr_value = events
        .into_iter()
        .map(|event| event.attributes)
        .flatten()
        .find(|attr| attr.key == attr_name.clone())
        .map(|attr| attr.value)
        .ok_or_else(|| {
            StdError::generic_err(format!("Attr '{}' not found", &attr_name))
        })?;

    let result = get_amount_from_asset_as_string(&attr_value);
    return match result {
        None => {
            Err(StdError::generic_err(format!("Fail to parse attr. Attr value: '{}'", attr_value)))
        }
        Some(a) => {
            Ok(a)
        }
    };
}

// asset as string format is 0123terra1..... or 0123uusd(amount + token_addr or denom without spaces)
// split mint_amount by the first met 't' or 'u'
pub fn get_amount_from_asset_as_string(data: &str) -> Option<String> {
    for (i, c) in data.chars().enumerate() {
        if c == 't' || c == 'u' {
            return Some(data[..i].to_string());
        }
    }
    None
}
