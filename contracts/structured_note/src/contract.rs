use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Binary, ContractResult, Decimal, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response, StdError, StdResult, SubMsgExecutionResponse, to_binary, Uint128};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::anchor::redeem_stable;
use crate::commands::{deposit_stable, deposit_stable_on_reply, store_position_and_exit, validate_masset, withdraw_stable};
use crate::mirror::{deposit_to_cdp, get_asset_price_in_collateral_asset, mint_to_cdp, open_cdp, query_masset_config, query_mirror_mint_config};
use crate::state::{load_config, load_state, State};
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
            new_position_info,
        } => {
            let config = load_config(deps.storage)?;

            let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

            let masset_token = deps.api.addr_validate(&masset_token)?;
            let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;
            validate_masset(&masset_config)?;
            // check msq
            let mut state = State {
                cdp_idx: None,
                farmer_addr: info.sender,
                masset_token,
                max_iteration_index: None,
                cur_iteration_index: 0,
                asset_price_in_collateral_asset: get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?,
                mirror_ts_factory_addr: deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?,
                aim_collateral_ratio: None,
                initial_cdp_collateral_amount: None,
                initial_cdp_loan_amount: None,
            };

            if let Some(v) = new_position_info {
                if !(1..=5).contains(&v.leverage_iter_amount) {
                    return Err(StdError::generic_err("Invalid message: leverage iterations amount should be from 1 to 5."));
                }

                let min_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
                if v.aim_collateral_ratio < min_collateral_ratio {
                    return Err(StdError::generic_err("Aim collateral ration too low"));
                }

                state.aim_collateral_ratio = Some(v.aim_collateral_ratio);
                state.max_iteration_index = Some(v.leverage_iter_amount);
            };

            let deposit_amount: Uint256 = info
                .funds
                .iter()
                .find(|c| c.denom == config.stable_denom)
                .map(|c| Uint256::from(c.amount))
                .unwrap_or_else(Uint256::zero);

            // Cannot deposit zero amount
            if deposit_amount.is_zero() {
                return Err(StdError::generic_err("Deposit amount is zero"));
            };

            deposit_stable(deps, state, deposit_amount)
        }
        ExecuteMsg::WithdrawStable { masset_token, amount } => {
            let masset_token = deps.api.addr_validate(&masset_token)?;
            if amount == Uint128::zero() {
                return Err(StdError::generic_err("Mirrored asset amount to withdraw is zero!"));
            };
            withdraw_stable(deps, &info.sender, &masset_token, amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let submessage_enum = SubmsgIds::try_from(msg.id)?;
    match submessage_enum {
        SubmsgIds::OpenCDP => {
            let events = msg.result.unwrap().events;
            let received_aterra_amount = get_amount_from_response_raw_attr(events, "mint_amount".to_string())?;
            open_cdp(deps, Uint128::from_str(&received_aterra_amount)?)
        }
        SubmsgIds::DepositToCDP => {
            let events = msg.result.unwrap().events;
            let received_farmer_amount = get_amount_from_response_raw_attr(events, "mint_amount".to_string())?;
            deposit_to_cdp(deps, Uint128::from_str(&received_farmer_amount)?)
        }
        SubmsgIds::MintAssetWithAimCollateralRatio => {
            let events = msg.result.unwrap().events;
            let deposited_amount = get_amount_from_response_asset_as_string_attr(events.clone(), "deposit_amount".to_string())?;
            let cdp_idx = get_amount_from_response_raw_attr(events, "position_idx".to_string())?;

            let mut state = load_state(deps.storage)?;
            if let None = state.cdp_idx {
                state.cdp_idx = Some(Uint128::from_str(&cdp_idx)?);
            }

            let deposited_amount = Uint128::from_str(&deposited_amount)?;
            //TODO: check calculation results!!!
            let mint_amount = deposited_amount * state.asset_price_in_collateral_asset * reverse_decimal(state.aim_collateral_ratio.unwrap());

            mint_to_cdp(deps.as_ref(), &state, mint_amount)
        }
        SubmsgIds::SellAsset => {
            let events = msg.result.unwrap().events;
            let minted_amount = get_amount_from_response_asset_as_string_attr(events.clone(), "mint_amount".to_string())?;
            let cdp_idx = get_amount_from_response_raw_attr(events, "position_idx".to_string())?;
            let mut state = load_state(deps.storage)?;
            if let None = state.cdp_idx {
                state.cdp_idx = Some(Uint128::from_str(&cdp_idx)?);
            }
            sell_asset(env, &state, Uint128::from_str(&minted_amount)?)
        }
        SubmsgIds::DepositStableOnReply => {
            let events = msg.result.unwrap().events;
            let received_stable = get_amount_from_response_raw_attr(events, "return_amount".to_string())?;
            let state = load_state(deps.storage)?;
            deposit_stable_on_reply(deps, state, Uint256::from_str(&received_stable)?)
        }
        SubmsgIds::Exit => {
            store_position_and_exit(deps)
        }
        SubmsgIds::RedeemStable => {
            let events = msg.result.unwrap().events;
            let received_aterra_amount = get_amount_from_response_asset_as_string_attr(events, "withdraw_amount".to_string())?;
            let config = load_config(deps.storage)?;
            redeem_stable(config, Uint128::from_str(&received_aterra_amount)?)
        }
        SubmsgIds::BuyAsset => {
            let events = msg.result.unwrap().events;
            let received_stable_amount = get_amount_from_response_raw_attr(events, "redeem_amount".to_string())?;
            let config = load_config(deps.storage)?;

            buy_asset(env, masset_token, mirror_ts_factory_addr, Uint128::from_str(&received_stable_amount)?);
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
