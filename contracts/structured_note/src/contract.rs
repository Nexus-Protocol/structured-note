use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    Addr, Binary, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response,
    StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg,
};
use protobuf::Message;

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::anchor::get_minted_amount_from_deposit_response;
use crate::commands::{deposit_stable, validate_masset};
use crate::mirror::{deposit_to_cdp, get_deposited_amount_from_deposit_to_cdp_response, mint_asset_to_aim_collateral_ratio, open_cdp, query_asset_price, query_collateral_price, query_collateral_price_and_multiplier, query_masset_config, query_mirror_mint_config};
use crate::state::{Config, DepositingState, load_config, load_depositing_state};
use crate::SubmsgIds;
use crate::terraswap::sell_asset;
use crate::utils::{decimal_division, reverse_decimal};

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
            deposit_stable(deps, info, &masset_config, depositing_state)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let submessage_enum = SubmsgIds::try_from(msg.id)?;
    match submessage_enum {
        SubmsgIds::OpenCDP => {
            let events = msg.result.unwrap().events;
            let received_aust_amount = get_minted_amount_from_deposit_response(events)?;
            open_cdp(deps, received_aust_amount.into())
        },
        SubmsgIds::DepositToCDP => {
            let events = msg.result.unwrap().events;
            let received_aust_amount = get_minted_amount_from_deposit_response(events)?;
            deposit_to_cdp(deps, received_aust_amount.into())
        },
        SubmsgIds::MintAssetWithAimCollateralRatio => {
            let events = msg.result.unwrap().events;
            let deposited_amount = get_deposited_amount_from_deposit_to_cdp_response(events)?;

            let config = load_config(deps.storage)?;
            let depositing_state = load_depositing_state(deps.storage)?;
            let mirror_mint_config = query_mirror_mint_config(deps.as_ref())?;

            let collateral_oracle = deps.api.addr_validate(mirror_mint_config.collateral_oracle.as_str())?;
            let (collateral_price, collateral_multiplier) = query_collateral_price(deps.as_ref(), &collateral_oracle, &config.aterra_addr);

            let oracle_addr = deps.api.addr_validate(mirror_mint_config.oracle.as_str())?;
            let asset_price = query_asset_price(deps.as_ref(), &oracle_addr, &depositing_state.masset_token, config.stable_denom)?;

            let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price);

            let mint_amount = Uint128::from_str(deposited_amount.as_str()) * asset_price_in_collateral_asset * reverse_decimal(depositing_state.aim_collateral_ratio);

            //TODO:
            mint_asset_to_aim_collateral_ratio()
        }
        SubmsgIds::SellAsset => {
            sell_asset()
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
