use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Binary, ContractResult, Decimal, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsgExecutionResponse, to_binary, Uint128};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, LeverageInfo, QueryMsg};

use crate::anchor::redeem_stable;
use crate::commands::{deposit_stable, deposit_stable_on_reply, store_position_and_exit, validate_masset, withdraw_stable};
use crate::mirror::{deposit_to_cdp, get_asset_price_in_collateral_asset, mint_to_cdp, open_cdp, query_cdp, query_masset_config, query_mirror_mint_config};
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
        ExecuteMsg::DepositStable {
            masset_token,
            leverage_info,
        } => {
            deposit_stable(deps, info, leverage_info, masset_token)
        }
        ExecuteMsg::WithdrawStable { masset_token, amount } => {
            // let masset_token = deps.api.addr_validate(&masset_token)?;
            // if amount == Uint128::zero() {
            //     return Err(StdError::generic_err("Mirrored asset amount to withdraw is zero!"));
            // };
            // let config = load_config(deps.storage)?;
            // let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;
            //
            // let asset_price_in_collateral_asset = get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
            //
            // let state = if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
            //     let cdp_state = query_cdp(deps.as_ref(), position.cdp_idx)?;
            //     State {
            //         cdp_idx: Some(position.cdp_idx),
            //         farmer_addr: info.sender,
            //         masset_token,
            //         max_iteration_index: Some(position.leverage_iter_amount),
            //         cur_iteration_index: 0,
            //         asset_price_in_collateral_asset,
            //         mirror_ts_factory_addr: deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?,
            //         aim_collateral_ratio: Some(position.aim_collateral_ratio),
            //         initial_cdp_collateral_amount: Some(cdp_state.collateral_amount),
            //         initial_cdp_loan_amount: Some(cdp_state.loan_amount)
            //     }
            // } else {
            //     return Err(StdError::generic_err(format!("There isn't position: farmer_addr: {}, masset_token: {}", &info.sender.to_string(), &masset_token.to_string())));
            // };
            //
            // withdraw_stable(deps, state, amount)
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
        SubmsgIds::MintAssetWithAimCollateralRatio => {
            let deposited_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "deposit_amount".to_string())?)?;
            let cdp_idx = Uint128::from_str(&get_amount_from_response_raw_attr(events, "position_idx".to_string())?)?;

            let state = insert_state_cdp_idx(deps.storage, cdp_idx)?;
            let leverage_info = load_leverage_info(deps.storage)?;

            let config = load_config(deps.storage)?;
            //TODO: check calculation results!!!
            let mint_amount = deposited_amount * state.asset_price_in_collateral_asset * reverse_decimal(leverage_info.aim_collateral_ratio);

            mint_to_cdp(config, &state, mint_amount)
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
            let config = load_config(deps.storage)?;
            redeem_stable(config, received_aterra_amount)
        }
        SubmsgIds::BuyAsset => {
            let received_stable_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "redeem_amount".to_string())?)?;
            let config = load_config(deps.storage)?;

            buy_asset(env, masset_token, mirror_ts_factory_addr, received_stable_amount)
        }
        SubmsgIds::BurnAsset => {}
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&load_config(deps.storage)?),
    }
}
