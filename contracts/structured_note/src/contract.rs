use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    Addr, Binary, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response,
    StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg,
};
use protobuf::Message;

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::commands::{deposit_stable, deposit_stable_on_reply, validate_masset};
use crate::mirror::{deposit_to_cdp, mint_to_cdp, open_cdp, query_asset_price, query_collateral_price, query_masset_config, query_mirror_mint_config};
use crate::state::{Config, DepositingState, load_config, load_depositing_state};
use crate::SubmsgIds;
use crate::terraswap::{query_pair_addr, sell_asset};
use crate::utils::{decimal_division, decimal_multiplication, get_amount_from_response_asset_as_string_attr, get_amount_from_response_raw_attr, reverse_decimal};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    unimplemented!()
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::DepositStable {
            masset_token,
            leverage_iter_amount,
            aim_collateral_ratio
        } => {
            // check msq
            match leverage_iter_amount {
                Some(value) => {
                    if value < 1 || value > 5 {
                        return Err(StdError::generic_err("Invalid message: leverage iterations amount should be from 1 to 5.".to_string()));
                    }
                    if aim_collateral_ratio.is_none() {
                        return Err(StdError::generic_err("Invalid message: aim_collateral_ratio is none".to_string()));
                    }
                },
                None => {
                    if aim_collateral_ratio.is_some() {
                        return Err(StdError::generic_err("Invalid message: leverage_iter_amount is none".to_string()));
                    }
                },
            };
            // check masset
            let masset_token = deps.api.addr_validate(&masset_token)?;
            let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;
            validate_masset(masset_config)?;
            let depositing_state = DepositingState::template(info.sender.clone(), masset_token, aim_collateral_ratio, leverage_iter_amount);

            let deposit_amount: Uint256 = info
                .funds
                .iter()
                .find(|c| c.denom == config.stable_denom)
                .map(|c| Uint256::from(c.amount))
                .unwrap_or_else(Uint256::zero);

            // Cannot deposit zero amount
            if deposit_amount.is_zero() {
                return Err(StdError::generic_err("Deposit amount is zero".to_string()));
            };

            let config = load_config(&deps.storage)?;

            if depositing_state.aim_collateral_ratio > 0.0 {
                let min_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
                if depositing_state.aim_collateral_ratio < min_collateral_ratio {
                    return Err(StdError::generic_err("Aim collateral ration too low".to_string()));
                } else {
                    let mirror_mint_config = query_mirror_mint_config(config.mirror_mint_contract.to_string())?;

                    let collateral_oracle = deps.api.addr_validate(mirror_mint_config.collateral_oracle.as_str())?;
                    let collateral_price = query_collateral_price(deps.as_ref(), &collateral_oracle, &config.aterra_addr)?;

                    let oracle_addr = deps.api.addr_validate(mirror_mint_config.oracle.as_str())?;
                    let asset_price = query_asset_price(deps.as_ref(), &oracle_addr, &depositing_state.masset_token, config.stable_denom)?;

                    deposit_state.asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price);
                    deposit_state.mirror_ts_factory_addr = deps.api.addr_validate(mirror_mint_config.terraswap_factory.as_str())?;
                }
            };

            deposit_stable(deps, depositing_state, deposit_amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let submessage_enum = SubmsgIds::try_from(msg.id)?;
    match submessage_enum {
        SubmsgIds::OpenCDP => {
            let events = msg.result.unwrap().events;
            let received_aust_amount = get_attr_value_from_response(events, "mint_amount".to_string())?;
            open_cdp(deps, received_aust_amount.into())
        },
        SubmsgIds::DepositToCDP => {
            let events = msg.result.unwrap().events;
            let received_aust_amount = get_amount_from_response_raw_attr(events, "mint_amount".to_string())?;
            deposit_to_cdp(deps, received_aust_amount.into())
        },
        SubmsgIds::MintAssetWithAimCollateralRatio => {
            let events = msg.result.unwrap().events;
            let deposited_amount = get_amount_from_response_asset_as_string_attr(events, "deposit_amount".to_string())?;

            let depositing_state = load_depositing_state(deps.storage)?;

            let mint_amount = Uint128::from_str(deposited_amount.as_str()) * depositing_state.asset_price_in_collateral_asset * reverse_decimal(depositing_state.aim_collateral_ratio);

            mint_to_cdp(&depositing_state, mint_amount)
        }
        SubmsgIds::SellAsset => {
            let events = msg.result.unwrap().events;
            let minted_amount = get_amount_from_response_asset_as_string_attr(events, "mint_amount".to_string())?;
            let depositing_state = load_depositing_state(deps.storage)?;

            sell_asset(env, &depositing_state, minted_amount.into())
        },
        SubmsgIds::DepositStableOnReply => {
            let events = msg.result.unwrap().events;
            let received_stable = get_amount_from_response_raw_attr(events, "return_amount".to_string())?;
            deposit_stable_on_reply(deps, depositing_state, received_stable.into())
        },
        SubmsgIds::Exit => {
            store_position_and_exit()
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&load_config(deps.storage)?),
    }
}
