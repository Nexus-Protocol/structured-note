use std::ops::{Div, Mul};

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, BlockInfo, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, from_binary, MessageInfo, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::{deposit_to_cdp, open_cdp, query_asset_price, query_cdp, query_collateral_price, query_mirror_mint_config};
use crate::state::{Config, DepositingState, load_cdp, load_config, load_position, load_positions_by_user_addr};
use crate::utils::{decimal_division, decimal_multiplication};

pub fn deposit_stable(
    deps: DepsMut,
    mut depositing_state: DepositingState,
    deposit_amount: Uint256,
) -> StdResult<Response> {
    let position_res = load_position(deps.storage, &depositing_state.farmer_addr, &depositing_state.masset_token);
    match position_res {
        Ok(position) => {
            let cdp_state = query_cdp(deps.as_ref(), position.cdp_idx)?;
            depositing_state.cdp_idx = position.cdp_idx;
            depositing_state.initial_cdp_collateral_amount = cdp_state.collateral_amount;
            depositing_state.initial_cdp_loan_amount = cdp_state.loan_amount;
        }
        Err(_) => {
            let cdp_res = load_cdp(deps.storage, masset_token);
            match cdp_res {
                Ok(cdp) => {
                    let cdp_state = query_cdp(deps.as_ref(), cdp.idx)?;
                    depositing_state.cdp_idx = position.cdp_idx;
                    depositing_state.initial_cdp_collateral_amount = cdp_state.collateral_amount;
                    depositing_state.initial_cdp_loan_amount = cdp_state.loan_amount;
                }
                Err(_) => {}
            }
        }
    };
    anc_deposit_stable(deps, &mut depositing_state, deposit_amount)
}

pub fn deposit_stable_on_reply(
    deps: DepsMut,
    mut depositing_state: DepositingState,
    deposit_amount: Uint256,
) -> StdResult<Response> {
    anc_deposit_stable(deps, &mut depositing_state, deposit_amount)
}

pub fn validate_masset(masset_config: MirrorAssetConfigResponse) -> StdResult<Response> {
    if masset_config.end_price.is_some() {
        return Err(StdError::generic_err("Invalid mirror asset: delisted  or migrated".to_string()));
    };
    if masset_config.ipo_params.is_some() {
        return Err(StdError::generic_err("Invalid mirror asset: pre ipo state".to_string()));
    };
    Ok(Default::default())
}

pub fn calculate_asset_price_in_collateral_asset(config: Config) -> Decimal {
    let mirror_mint_config = query_mirror_mint_config(config.mirror_mint_contract.to_string())?;

    let collateral_oracle = deps.api.addr_validate(mirror_mint_config.collateral_oracle.as_str())?;
    let collateral_price = query_collateral_price(deps.as_ref(), &collateral_oracle, &config.aterra_addr)?;

    let oracle_addr = deps.api.addr_validate(mirror_mint_config.oracle.as_str())?;
    let asset_price = query_asset_price(deps.as_ref(), &oracle_addr, &depositing_state.masset_token, config.stable_denom)?;

    decimal_division(collateral_price, asset_price)
}
