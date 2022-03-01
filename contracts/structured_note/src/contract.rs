use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Binary, ContractResult, Decimal, Decimal256, Deps, DepsMut, entry_point, Env, Fraction, MessageInfo, Reply, Response, StdError, StdResult, SubMsgExecutionResponse, to_binary, Uint128};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::anchor::redeem_stable;
use crate::commands::{close, deposit, deposit_stable_on_reply, store_position_and_exit, validate_masset, withdraw};
use crate::mirror::{burn_asset, deposit_to_cdp, get_asset_price_in_collateral_asset, mint_asset, open_cdp, query_cdp, query_masset_config, query_mirror_mint_config, withdraw_collateral};
use crate::state::{increase_state_collateral_diff, increase_state_loan_diff, insert_state_cdp_idx, load_config, load_position, load_state, may_load_position, State};
use crate::SubmsgIds;
use crate::terraswap::{buy_asset, sell_asset};
use crate::utils::{decimal_division, decimal_multiplication, get_amount_from_response_asset_as_string_attr, get_amount_from_response_raw_attr, reverse_decimal};

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    unimplemented!()
}

//TODO: v.0.2 check liquidity
//TODO: v.0.2 check slippage
//TODO: v.0.2 avoid send zero tokens issue: check deposit is enough to -> mint enough aterra to -> borrow enough masset to -> buy enough UST -> etc
#[entry_point]
pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Deposit {
            masset_token,
            leverage,
            aim_collateral_ratio,
        } => {
            deposit(deps, info, masset_token, leverage, aim_collateral_ratio)
        },
        ExecuteMsg::ClosePosition { masset_token } => {
            close(deps, info, masset_token)
        }
        ExecuteMsg::Withdraw { masset_token, amount, aim_collateral_ratio } => {
            withdraw(deps, info, masset_token, amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let events = match msg.result {
        ContractResult::Ok(result) => result.events,
        ContractResult::Err(e) => return Err(StdError::generic_err("Fail to parse reply response")),
    };

    let submessage_enum = SubmsgIds::try_from(msg.id)?;
    match submessage_enum {
        SubmsgIds::OpenCDP => {
            let received_aterra_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "mint_amount".to_string())?)?;
            open_cdp(deps, received_aterra_amount)
        }
        SubmsgIds::DepositToCDP => {
            let received_farmer_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "mint_amount".to_string())?)?;
            deposit_to_cdp(deps, received_farmer_amount)
        }
        SubmsgIds::MintAsset => {
            let deposited_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "deposit_amount".to_string())?)?;
            let cdp_idx = Uint128::from_str(&get_amount_from_response_raw_attr(events, "position_idx".to_string())?)?;

            insert_state_cdp_idx(deps.storage, cdp_idx)?;
            let state = increase_state_collateral_diff(deps.storage, deposited_amount)?;

            let config = load_config(deps.storage)?;

            let (collateral_amount, loan_amount) = if let Some(position) = may_load_position(deps.storage, &state.farmer_addr, &state.masset_token)? {
                (position.total_collateral_amount + state.collateral_amount_diff, position.total_loan_amount + state.loan_amount_diff)
            } else {
                (state.collateral_amount_diff, state.loan_amount_diff)
            };
            //aim_collateral_ratio = collateral_value / aim_loan_value = collateral_ratio / (aim_loan_amount * asset_price_in_collateral_asset)
            // aim_loan_amount = collateral_amount/(aim_collateral_ratio * asset_price_in_collateral_asset)

            let coef = decimal_multiplication(&state.aim_collateral_ratio, &state.asset_price_in_collateral_asset);

            let aim_loan_amount = Uint128::from(collateral_amount.u128() * coef.denominator() / coef.numerator());

            if aim_loan_amount <= loan_amount {
                // impossible case because to decrease loan_amount contract needs to burn some masset_tokens which are not considered to be in contract
                return Err(StdError::generic_err("Aim loan amount is less of equals to actual loan amount. Deposit doesn't handle burning borrowed asset tokens."));
            };
            let mint_amount = aim_loan_amount - loan_amount;
            mint_asset(config, &state, mint_amount)
        }
        SubmsgIds::SellAsset => {
            let minted_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "mint_amount".to_string())?)?;
            let cdp_idx = Uint128::from_str(&get_amount_from_response_raw_attr(events, "position_idx".to_string())?)?;
            insert_state_cdp_idx(deps.storage, cdp_idx)?;
            let state = increase_state_loan_diff(deps.storage, minted_amount)?;
            sell_asset(env, &state, minted_amount)
        }
        SubmsgIds::DepositStableOnReply => {
            let received_stable = Uint256::from_str(&get_amount_from_response_raw_attr(events, "return_amount".to_string())?)?;
            let state = load_state(deps.storage)?;
            deposit_stable_on_reply(deps, state, received_stable)
        }
        SubmsgIds::Exit => {
            store_position_and_exit(deps)
        }
        SubmsgIds::RedeemStable => {
            let received_aterra_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "withdraw_amount".to_string())?)?;
            redeem_stable(deps.as_ref(), received_aterra_amount)
        }
        SubmsgIds::BuyAsset => {
            let received_stable_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "redeem_amount".to_string())?)?;
            buy_asset(deps.as_ref(), env, received_stable_amount)
        }
        SubmsgIds::BurnAsset => {
            let return_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "return_amount".to_string())?)?;
            burn_asset(deps, return_amount)
        }
        SubmsgIds::WithdrawCollateralOnReply => {
            //TODO: figure out amount to withdraw from collateral on reply
            // withdraw_collateral();
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&load_config(deps.storage)?),
        QueryMsg::Position { .. } => { unimplemented!() }
    }
}
