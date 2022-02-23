use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Addr, DepsMut, Response, StdError, StdResult, Uint128};

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::{get_asset_price_in_collateral_asset, query_cdp, query_mirror_mint_config, withdraw_collateral};
use crate::state::{load_cdp, load_config, load_position, load_state, may_load_cdp, may_load_position, Position, State, store_state, update_cdp, upsert_position};

pub fn deposit_stable(
    deps: DepsMut,
    mut state: State,
    deposit_amount: Uint256,
) -> StdResult<Response> {
    if let Some(p) = may_load_position(deps.storage, &state.farmer_addr, &state.masset_token)? {
        let cdp_state = query_cdp(deps.as_ref(), p.cdp_idx)?;
        state.cdp_idx = Some(p.cdp_idx);
        state.initial_cdp_collateral_amount = Some(cdp_state.collateral_amount);
        state.initial_cdp_loan_amount = Some(cdp_state.loan_amount);
        state.aim_collateral_ratio = Some(p.aim_collateral_ratio);
        state.max_iteration_index = Some(p.leverage_iter_amount);
    } else {
        if let Some(cdp) = may_load_cdp(deps.storage, &state.masset_token)? {
            let cdp_state = query_cdp(deps.as_ref(), cdp.idx)?;
            state.cdp_idx = Some(cdp.idx);
            state.initial_cdp_collateral_amount = Some(cdp_state.collateral_amount);
            state.initial_cdp_loan_amount = Some(cdp_state.loan_amount);
        } else {
            state.initial_cdp_collateral_amount = Some(Uint128::zero());
            state.initial_cdp_loan_amount = Some(Uint128::zero());
        }
    }
    store_state(deps.storage, &state)?;

    anc_deposit_stable(deps, &mut state, deposit_amount)
}

pub fn deposit_stable_on_reply(
    deps: DepsMut,
    mut state: State,
    deposit_amount: Uint256,
) -> StdResult<Response> {
    anc_deposit_stable(deps, &mut state, deposit_amount)
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
    let state = load_state(deps.storage)?;
    let current_cdp_state = query_cdp(deps.as_ref(), state.cdp_idx.unwrap())?;
    let loan_diff = current_cdp_state.loan_amount - state.initial_cdp_loan_amount.unwrap();
    let collateral_diff = current_cdp_state.collateral_amount - state.initial_cdp_collateral_amount.unwrap();

    upsert_position(deps.storage, &state, loan_diff, collateral_diff)?;

    update_cdp(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "store_and_exit"))
}

pub fn withdraw_stable(deps: DepsMut, farmer_addr: &Addr, masset_token: &Addr, amount: Uint128) -> StdResult<Response> {
    let position_res = load_position(deps.storage, farmer_addr, masset_token);
    match position_res {
        Err(_) => Err(StdError::generic_err("Fail to read position")),
        Ok(position) => {
            if &position.farmer_addr != farmer_addr {
                return Err(StdError::generic_err("Unauthorized"));
            }
            let config = load_config(deps.storage)?;
            let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

            let asset_price_in_collateral_asset = get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &position.masset_token)?;

            let state = State {
                cdp_idx: Some(position.cdp_idx),
                farmer_addr: position.farmer_addr,
                masset_token: position.masset_token,
                max_iteration_index: Some(position.leverage_iter_amount),
                cur_iteration_index: 0,
                asset_price_in_collateral_asset,
                mirror_ts_factory_addr: deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?,
                aim_collateral_ratio: Some(position.aim_collateral_ratio),
                initial_cdp_collateral_amount: Some(position.total_collateral_amount),
                initial_cdp_loan_amount: Some(position.total_loan_amount),
            };

            let amount_to_withdraw = amount * asset_price_in_collateral_asset;
            withdraw_collateral(&config, position.cdp_idx, amount_to_withdraw)
        }
    }
}