use std::ops::{Div, Mul};

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr, BlockInfo, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, from_binary, MessageInfo, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::{deposit_stable as anc_deposit_stable, get_mint_amount_form_deposit_response};
use crate::mirror::{deposit_to_cdp, open_cdp, query_cdp};
use crate::state::{Config, DepositingState, load_cdp, load_config, load_position, load_positions_by_user_addr};
use crate::utils::decimal_multiplication;

pub fn deposit_stable(
    deps: DepsMut,
    info: MessageInfo,
    masset_config: &MirrorAssetConfigResponse,
    deposit_state: DepositingState,
) -> StdResult<Response> {
    let config: Config = load_config(deps.storage)?;

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

    if deposit_state.init_collateral_ratio > 0.0 {
        let min_collateral_ratio = decimal_multiplication(masset_config.min_collateral_ratio, config.min_over_collateralization);
        if deposit_state.init_collateral_ratio < min_collateral_ratio {
            return Err(StdError::generic_err("Initial collateral ration too low".to_string()));
        } else {
            deposit_stable_inner(deps, depositing_state, deposit_amount);
        }
    };

    Ok(Default::default())
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

fn deposit_stable_inner(
    deps: DepsMut,
    depositing_state: &mut DepositingState,
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
    anc_deposit_stable(deps, depositing_state, deposit_amount)
}

fn open_new_cdp(config: &Config, deposit_amount: Uint256, masset_token: &Addr, initial_collateral_ratio: Decimal) -> StdResult<Response> {}