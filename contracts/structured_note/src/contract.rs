use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Binary, ContractResult, Deps, DepsMut, entry_point, Env, Fraction, MessageInfo, Reply, Response, StdError, StdResult, to_binary, Uint128};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::anchor::redeem_stable;
use crate::commands::{close, close_on_reply, deposit, deposit_stable_on_reply, exit, withdraw};
use crate::mirror::{burn_asset, deposit_to_cdp, mint_asset, open_cdp};
use crate::state::{add_farmer_to_cdp, decrease_position_loan, increase_position_loan, increment_iteration_index, load_config, load_state, may_load_position, Position, save_position};
use crate::SubmsgIds;
use crate::terraswap::{buy_asset, sell_asset};
use crate::utils::{decimal_multiplication, get_action_name, get_amount_from_response_asset_as_string_attr, get_amount_from_response_raw_attr};

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
        ExecuteMsg::PlaneDeposit { masset_token } => {},
        ExecuteMsg::ClosePosition { masset_token } => {
            close(deps, info, masset_token)
        }
        ExecuteMsg::Withdraw { masset_token, amount, aim_collateral_ratio } => {
            withdraw(deps, info, masset_token, amount, aim_collateral_ratio)
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
            open_cdp(load_config(deps.storage)?, load_state(deps.storage)?, received_aterra_amount)
        }
        SubmsgIds::DepositToCDP => {
            let received_aterra_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "mint_amount".to_string())?)?;
            deposit_to_cdp(deps, received_aterra_amount)
        }
        SubmsgIds::MintAsset => {
            let config = load_config(deps.storage)?;
            let state = load_state(deps.storage)?;

            let (cdp_idx, collateral_amount, loan_amount) = if let Some(position) = may_load_position(deps.storage, &state.farmer_addr, &state.masset_token)? {
                (position.cdp_idx, position.collateral_amount, position.loan_amount)
            } else {
                return Err(StdError::generic_err(format!(
                    "There isn't position: farmer_addr: {}, masset_token: {}.",
                    &state.farmer_addr.to_string(),
                    &state.masset_token.to_string())));
            };
            //aim_collateral_ratio = collateral_value / aim_loan_value = collateral_ratio / (aim_loan_amount * asset_price_in_collateral_asset)
            // aim_loan_amount = collateral_amount/(aim_collateral_ratio * asset_price_in_collateral_asset)
            let coef = decimal_multiplication(&state.aim_collateral_ratio, &state.asset_price_in_collateral_asset);
            let aim_loan_amount = Uint128::from(collateral_amount.u128() * coef.denominator() / coef.numerator());

            if aim_loan_amount <= loan_amount {
                // impossible case because to decrease loan_amount contract needs to burn some masset_tokens which are not considered to be in the contract atm
                return Err(StdError::generic_err("Aim loan amount is less or equals to actual loan amount. Deposit doesn't handle burning borrowed asset tokens."));
            };
            let mint_amount = aim_loan_amount - loan_amount;

            increase_position_loan(deps.storage, &state.farmer_addr, &state.masset_token, mint_amount)?;
            mint_asset(config, cdp_idx, state.masset_token.to_string(), mint_amount)
        }
        SubmsgIds::SellAsset => {
            let state = load_state(deps.storage)?;
            let minted_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "mint_amount".to_string())?)?;
            let collateral_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "collateral_amount".to_string())?)?;
            if get_action_name(events.clone())? == "open_position".to_string() {
                let cdp_idx = Uint128::from_str(&get_amount_from_response_raw_attr(events, "position_idx".to_string())?)?;
                save_position(deps.storage, &Position {
                    farmer_addr: state.farmer_addr.clone(),
                    masset_token: state.masset_token.clone(),
                    cdp_idx,
                    leverage: state.leverage,
                    loan_amount: minted_amount,
                    collateral_amount,
                    aim_collateral_ratio: state.aim_collateral_ratio,
                })?;
                add_farmer_to_cdp(deps.storage, cdp_idx, state.farmer_addr, state.masset_token)?;
            }
            sell_asset(env, &state, minted_amount)
        }
        SubmsgIds::DepositOnReply => {
            let received_stable = Uint256::from_str(&get_amount_from_response_raw_attr(events, "return_amount".to_string())?)?;
            deposit_stable_on_reply(deps, received_stable)
        }
        SubmsgIds::Exit => {
            exit()
        }
        SubmsgIds::RedeemStable => {
            let received_aterra_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "withdraw_amount".to_string())?)?;
            redeem_stable(load_config(deps.storage)?, received_aterra_amount)
        }
        SubmsgIds::BuyAsset => {
            let received_stable_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "redeem_amount".to_string())?)?;
            buy_asset(load_config(deps.storage)?, load_state(deps.storage)?, env.contract.address.to_string(), received_stable_amount)
        }
        SubmsgIds::BurnAsset => {
            let return_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "return_amount".to_string())?)?;
            //increment iteration index for using this function on withdraw not only closure
            let state = increment_iteration_index(deps.storage)?;
            let position = decrease_position_loan(deps.storage, &state.farmer_addr, &state.masset_token, return_amount)?;
            burn_asset(load_config(deps.storage)?, state, position.cdp_idx, return_amount)
        }
        SubmsgIds::CloseOnReply => {
            let state = load_state(deps.storage)?;
            close_on_reply(deps, state)
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
