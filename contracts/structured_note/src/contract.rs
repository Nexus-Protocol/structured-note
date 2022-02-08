use std::convert::TryFrom;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    Addr, Binary, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, Response,
    StdError, StdResult, SubMsg, to_binary, WasmMsg,
};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::commands::{deposit_stable, validate_masset};
use crate::mirror::{deposit_to_cdp, open_cdp, query_masset_config};
use crate::state::{Config, DepositingState, load_config};
use crate::SubmsgIds;
use crate::terraswap::sell_asset;

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
            initial_collateral_ratio
        } => {
            // check msq
            match leverage_iter_amount {
                Some(value) => {
                    if value < 1 || value > 5 {
                        return Err(StdError::generic_err("Invalid message: leverage iterations amount should be from 1 to 5.".to_string()));
                    }
                    if initial_collateral_ratio.is_none() {
                        return Err(StdError::generic_err("Invalid message: initial_collateral_ratio is none".to_string()));
                    }
                },
                None => {
                    if initial_collateral_ratio.is_some() {
                        return Err(StdError::generic_err("Invalid message: leverage_iter_amount is none".to_string()));
                    }
                },
            };
            // check masset
            let masset_token = deps.api.addr_validate(&masset_token)?;
            let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;
            validate_masset(masset_config)?;
            let depositing_state = DepositingState::template(info.sender.clone(), masset_token, initial_collateral_ratio, leverage_iter_amount);
            deposit_stable(deps, info, &masset_config, depositing_state)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let submessage_enum = SubmsgIds::try_from(msg.id)?;
    match submessage_enum {
        SubmsgIds::OpenCDP => {
            open_cdp()
        },
        SubmsgIds::DepositToCDP => {
            deposit_to_cdp()
        },
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
