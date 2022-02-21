use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{DepsMut, Response, StdError, StdResult};

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::query_cdp;
use crate::state::{DepositingState, load_cdp, load_depositing_state, load_position, update_cdp, upsert_position};

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
            let cdp_res = load_cdp(deps.storage, &depositing_state.masset_token);
            if let Ok(cdp) = cdp_res {
                let cdp_state = query_cdp(deps.as_ref(), cdp.idx)?;
                depositing_state.cdp_idx = cdp.idx;
                depositing_state.initial_cdp_collateral_amount = cdp_state.collateral_amount;
                depositing_state.initial_cdp_loan_amount = cdp_state.loan_amount;
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

pub fn validate_masset(masset_config: &MirrorAssetConfigResponse) -> StdResult<Response> {
    if masset_config.end_price.is_some() {
        return Err(StdError::generic_err("Invalid mirror asset: delisted  or migrated".to_string()));
    };
    if masset_config.ipo_params.is_some() {
        return Err(StdError::generic_err("Invalid mirror asset: pre ipo state".to_string()));
    };
    Ok(Default::default())
}

pub fn store_position_and_exit(deps: DepsMut) -> StdResult<Response> {
    let depositing_state = load_depositing_state(deps.storage)?;
    let current_cdp_state = query_cdp(deps.as_ref(), depositing_state.cdp_idx)?;
    let loan_diff = current_cdp_state.loan_amount - depositing_state.initial_cdp_loan_amount;
    let collateral_diff = current_cdp_state.collateral_amount - depositing_state.initial_cdp_collateral_amount;

    upsert_position(deps.storage, &depositing_state, loan_diff, collateral_diff)?;

    update_cdp(deps.storage, &depositing_state)?;

    Ok(Response::new()
        .add_attribute("method", "store_and_exit"))
}
