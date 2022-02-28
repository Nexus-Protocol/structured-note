use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Binary, ContractResult, Decimal, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsgExecutionResponse, to_binary, Uint128};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, LeverageInfo, QueryMsg};

use crate::anchor::redeem_stable;
use crate::commands::{deposit, deposit_stable, deposit_stable_on_reply, open_position, store_position_and_exit, validate_masset, withdraw};
use crate::mirror::{burn_asset, deposit_to_cdp, get_asset_price_in_collateral_asset, mint_asset, open_cdp, query_cdp, query_masset_config, query_mirror_mint_config, withdraw_collateral};
use crate::state::{insert_state_cdp_idx, load_config, load_leverage_info, load_state, may_load_position, State, store_leverage_info};
use crate::SubmsgIds;
use crate::terraswap::{buy_asset, sell_asset};
use crate::utils::{decimal_multiplication, get_amount_from_response_asset_as_string_attr, get_amount_from_response_raw_attr, reverse_decimal};

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
        ExecuteMsg::OpenPosition {
            masset_token,
            leverage,
            initial_collateral_ratio,
        } => {
            deposit(deps, info, masset_token, Some(leverage), initial_collateral_ratio)
        },
        ExecuteMsg::Deposit {
            masset_token,
            aim_collateral_ratio
        } => {
            deposit(deps, info, masset_token, None, aim_collateral_ratio)
        },
        ExecuteMsg::ClosePosition { masset_token } => {
            withdraw(deps, info, masset_token)
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

            let state = insert_state_cdp_idx(deps.storage, cdp_idx)?;

            let config = load_config(deps.storage)?;

            //current collateral_amount = position.total_collateral + deposited_amount
            //current loan_amount = position.total_loan
            // ---- update position every iteration? or extend state with loan and collateral diff?
            //mint amount = X - current loan_amount
            // X = collateral_amount / (aim_collateral_ratio * asset_price_in_collateral_asset)
            //TODO: collateral_ratio adjustment
            let mint_amount = deposited_amount * state.asset_price_in_collateral_asset * reverse_decimal(state.aim_collateral_ratio);

            mint_asset(config, &state, mint_amount)
        }
        SubmsgIds::SellAsset => {
            let minted_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "mint_amount".to_string())?)?;
            let cdp_idx = Uint128::from_str(&get_amount_from_response_raw_attr(events, "position_idx".to_string())?)?;
            let state = insert_state_cdp_idx(deps.storage, cdp_idx)?;
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
    }
}
