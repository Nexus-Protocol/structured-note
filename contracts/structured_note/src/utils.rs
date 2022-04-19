use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, BalanceResponse, BankQuery, Coin, Decimal, Deps, Event, QuerierWrapper, QueryRequest, StdError, StdResult, Uint128};
use terra_cosmwasm::TerraQuerier;

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

//Paring
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

pub fn query_balance(
    querier: &QuerierWrapper,
    account_addr: &Addr,
    denom: &str,
) -> StdResult<Uint128> {
    let balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: account_addr.to_string(),
        denom: denom.to_string(),
    }))?;
    Ok(balance.amount.amount)
}

pub fn get_taxed(deps: Deps, denom: &str, amount: Uint256) -> StdResult<Uint256> {
    let terra_querier = TerraQuerier::new(&deps.querier);
    let rate = Decimal256::from((terra_querier.query_tax_rate()?).rate);
    let cap = Uint256::from((terra_querier.query_tax_cap(denom)?).cap);

    let tax = if amount.is_zero() {
        Uint256::zero()
    } else {
        let rate_part = Decimal256::one() - Decimal256::one() / (Decimal256::one() + rate);
        ceiled_mul_uint_decimal(amount, rate_part)
    };

    let tax_capped = std::cmp::min(tax, cap);
    Ok(amount - std::cmp::max(tax_capped, Uint256::one()))
}

fn ceiled_mul_uint_decimal(a: Uint256, b: Decimal256) -> Uint256 {
    let decimal_output = Decimal256::from_uint256(a) * b;
    let floored_output = Uint256::from(decimal_output.0 / Decimal256::DECIMAL_FRACTIONAL);

    // Check for rounding error
    if decimal_output != Decimal256::from_uint256(floored_output) {
        floored_output + Uint256::one()
    } else {
        floored_output
    }
}