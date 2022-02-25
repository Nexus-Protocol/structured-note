use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Addr, DepsMut, MessageInfo, Response, StdError, StdResult, Uint128};
use terraswap::querier::query_pair_info;

use structured_note_package::mirror::MirrorAssetConfigResponse;
use structured_note_package::structured_note::LeverageInfo;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::{get_asset_price_in_collateral_asset, query_cdp, query_masset_config, query_mirror_mint_config, withdraw_collateral};
use crate::state::{InitialCDPState, load_cdp, load_config, load_initial_cdp_state, load_leverage_info, load_position, load_state, may_load_cdp, may_load_position, Position, State, store_initial_cdp_state, store_leverage_info, store_state, update_cdp, upsert_position};
use crate::terraswap::query_pair_addr;
use crate::utils::decimal_multiplication;

pub fn deposit_stable(
    deps: DepsMut,
    info: MessageInfo,
    leverage_info: Option<LeverageInfo>,
    masset_token: String,
) -> StdResult<Response> {
    let mut open_cdp = false;
    let config = load_config(deps.storage)?;

    let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

    let masset_token = deps.api.addr_validate(&masset_token)?;
    let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;

    let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

    validate_masset(&masset_config)?;

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

    let mut state = State {
        cdp_idx: None,
        farmer_addr: info.sender,
        masset_token,
        cur_iteration_index: 0,
        asset_price_in_collateral_asset: get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?,
        pair_addr,
    };

    if let Some(p) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        let cdp_state = query_cdp(deps.as_ref(), p.cdp_idx)?;
        store_leverage_info(deps.storage, &LeverageInfo {
            leverage_iter_amount: p.leverage_iter_amount,
            aim_collateral_ratio: p.aim_collateral_ratio,
        })?;
        store_initial_cdp_state(deps.storage, &InitialCDPState {
            collateral_amount: cdp_state.collateral_amount,
            loan_amount: cdp_state.loan_amount,
        })?;
        state.cdp_idx = Some(p.cdp_idx);
    } else {
        if let Some(v) = leverage_info {
            if !(1..=5).contains(&v.leverage_iter_amount) {
                return Err(StdError::generic_err("Invalid message: leverage iterations amount should be from 1 to 5."));
            }

            let min_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
            if v.aim_collateral_ratio < min_collateral_ratio {
                return Err(StdError::generic_err("Aim collateral ration too low"));
            };
            store_leverage_info(deps.storage, &v)?;
        } else {
            return Err(StdError::generic_err(format!(
                "There isn't position: farmer_addr: {}, masset_token: {}. User has to provide leverage_info",
                &info.sender.to_string(),
                &masset_token.to_string())));
        }
        if let Some(cdp) = may_load_cdp(deps.storage, &masset_token)? {
            let cdp_state = query_cdp(deps.as_ref(), cdp.idx)?;
            state.cdp_idx = Some(cdp.idx);
            store_initial_cdp_state(deps.storage, &InitialCDPState {
                collateral_amount: cdp_state.collateral_amount,
                loan_amount: cdp_state.loan_amount,
            })?;
        } else {
            open_cdp = true;

            store_initial_cdp_state(deps.storage, &InitialCDPState {
                collateral_amount: Uint128::zero(),
                loan_amount: Uint128::zero(),
            })?;
        }
    }
    store_state(deps.storage, &state)?;

    anc_deposit_stable(config, open_cdp, deposit_amount)
}

pub fn deposit_stable_on_reply(
    deps: DepsMut,
    mut state: State,
    deposit_amount: Uint256,
) -> StdResult<Response> {
    let config = load_config(deps.storage)?;
    anc_deposit_stable(config, false, deposit_amount)
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
    let leverage_info = load_leverage_info(deps.storage)?;
    let initial_cdp_state = load_initial_cdp_state(deps.storage)?;

    let cdp_idx = if let Some(i) = state.cdp_idx {
        i
    } else {
        return Err(StdError::generic_err("cdp_idx has to be stored by now"));
    };

    let current_cdp_state = query_cdp(deps.as_ref(), cdp_idx)?;

    let loan_diff = current_cdp_state.loan_amount - initial_cdp_state.loan_amount;
    let collateral_diff = current_cdp_state.collateral_amount - initial_cdp_state.collateral_amount;

    upsert_position(deps.storage, &state, &leverage_info, loan_diff, collateral_diff)?;

    update_cdp(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "store_and_exit"))
}

pub fn withdraw_stable(deps: DepsMut, info: MessageInfo, masset_token: String, amount: Uint128) -> StdResult<Response> {
    let config = load_config(deps.storage)?;
    let masset_token = deps.api.addr_validate(&masset_token)?;
    let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

    let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

    let asset_price_in_collateral_asset = get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
    if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        let cdp_state = query_cdp(deps.as_ref(), position.cdp_idx)?;
        store_state(deps.storage, &State {
            cdp_idx: Some(position.cdp_idx),
            farmer_addr: info.sender,
            masset_token,
            cur_iteration_index: 0,
            asset_price_in_collateral_asset: asset_price_in_collateral_asset.clone(),
            pair_addr,
        })?;
        store_initial_cdp_state(deps.storage, &InitialCDPState {
            collateral_amount: cdp_state.collateral_amount,
            loan_amount: cdp_state.loan_amount,
        })?;
        store_leverage_info(deps.storage, &LeverageInfo {
            leverage_iter_amount: position.leverage_iter_amount,
            aim_collateral_ratio: position.aim_collateral_ratio,
        });
        let amount_to_withdraw = amount * asset_price_in_collateral_asset;
        withdraw_collateral(&config, position.cdp_idx, amount_to_withdraw)
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}