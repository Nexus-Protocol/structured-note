use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Addr, Decimal, DepsMut, MessageInfo, Response, StdError, StdResult, Uint128};
use terraswap::querier::query_pair_info;

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::{get_asset_price_in_collateral_asset, query_cdp, query_masset_config, query_mirror_mint_config, withdraw_collateral};
use crate::state::{increase_state_collateral_diff, insert_state_cdp_idx, load_config, load_state, may_load_cdp, may_load_position, Position, State, store_state, update_cdp, upsert_position};
use crate::terraswap::query_pair_addr;
use crate::utils::decimal_multiplication;

pub fn deposit(
    deps: DepsMut,
    info: MessageInfo,
    masset_token: String,
    leverage: Option<u8>,
    aim_collateral_ratio: Decimal,
) -> StdResult<Response> {
    let mut open_cdp = false;
    let config = load_config(deps.storage)?;

    let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

    let masset_token = deps.api.addr_validate(&masset_token)?;
    let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;

    let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

    let asset_price_in_collateral_asset = get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;

    let min_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
    if aim_collateral_ratio < min_collateral_ratio {
        return Err(StdError::generic_err("Aim collateral ration too low"));
    };

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

    if let Some(p) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        store_state(deps.storage, &State {
            cdp_idx: Some(p.cdp_idx),
            farmer_addr: p.farmer_addr,
            masset_token: p.masset_token,
            leverage: p.leverage,
            cur_iteration_index: 0,
            asset_price_in_collateral_asset,
            pair_addr,
            aim_collateral_ratio,
            loan_amount_diff: Default::default(),
            collateral_amount_diff: Default::default(),
        })?;
    } else {
        if let Some(leverage) = leverage {
            if !(1..=5).contains(&leverage) {
                return Err(StdError::generic_err("Invalid message: leverage iterations amount should be from 1 to 5."));
            }
            store_state(deps.storage, &State {
                cdp_idx: None,
                farmer_addr: info.sender,
                masset_token,
                leverage,
                cur_iteration_index: 0,
                asset_price_in_collateral_asset,
                pair_addr,
                aim_collateral_ratio,
                loan_amount_diff: Default::default(),
                collateral_amount_diff: Default::default(),
            })?;
        } else {
            return Err(StdError::generic_err(format!(
                "There isn't position: farmer_addr: {}, masset_token: {}.",
                &info.sender.to_string(),
                &masset_token.to_string())));
        }
        if let Some(cdp) = may_load_cdp(deps.storage, &masset_token)? {
            let cdp_state = query_cdp(deps.as_ref(), cdp.idx)?;
            insert_state_cdp_idx(deps.storage, cdp.idx)?;
        } else {
            open_cdp = true;
        }
    }

    anc_deposit_stable(config, open_cdp, deposit_amount)
}

pub fn deposit_stable_on_reply(
    deps: DepsMut,
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

    let cdp_idx = if let Some(i) = state.cdp_idx {
        i
    } else {
        return Err(StdError::generic_err("cdp_idx has to be stored by now"));
    };

    upsert_position(deps.storage, &state, state.loan_amount_diff, state.collateral_amount_diff)?;

    update_cdp(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "store_and_exit"))
}

pub fn close(deps: DepsMut, info: MessageInfo, masset_token: String) -> StdResult<Response> {
    let masset_token = deps.api.addr_validate(&masset_token)?;
    let config = load_config(deps.storage)?;
    let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

    let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

    let asset_price_in_collateral_asset = get_asset_price_in_collateral_asset(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
    if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        store_state(deps.storage, &State {
            cdp_idx: Some(position.cdp_idx),
            farmer_addr: info.sender,
            masset_token,
            leverage: position.leverage,
            cur_iteration_index: 0,
            asset_price_in_collateral_asset: asset_price_in_collateral_asset.clone(),
            pair_addr,
            aim_collateral_ratio: Default::default(),
            loan_amount_diff: Default::default(),
            collateral_amount_diff: Default::default(),
        })?;

        let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;

        let withdrawable_collateral = position.total_collateral_amount - position.total_loan_amount * asset_price_in_collateral_asset * position.aim_collateral_ratio * (masset_config.min_collateral_ratio + Decimal::percent(10));

        increase_state_collateral_diff(deps.storage, withdrawable_collateral)?;

        withdraw_collateral(config, position.cdp_idx, withdrawable_collateral)
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}

pub fn close_on_reply(state: State) -> StdResult<Response> {
    if let Some(position) = may_load_position(deps.storage, &state.farmer_addr, &state.masset_token)? {
        // can this variable be negative, another world total_collateral_amount less than collateral_amount_diff
        let rest_collateral = position.total_collateral_amount - state.collateral_amount_diff;
        if rest_collateral.is_zero() {
            return exit_on_close();
        };

        let masset_config = query_masset_config(deps.as_ref(), &state.masset_token)?;

        let withdrawable_collateral = position.total_collateral_amount - state.collateral_amount_diff - (position.total_loan_amount - state.loan_amount_diff) * asset_price_in_collateral_asset * position.aim_collateral_ratio * (masset_config.min_collateral_ratio + Decimal::percent(10));

        withdraw_collateral(config, position.cdp_idx, withdrawable_collateral)
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}

pub fn withdraw(deps: DepsMut, info: MessageInfo, masset_token: String, amount: Uint128) -> StdResult<Response> {
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
            leverage: 0,
            cur_iteration_index: 0,
            asset_price_in_collateral_asset: asset_price_in_collateral_asset.clone(),
            pair_addr,
            aim_collateral_ratio: Default::default(),
            loan_amount_diff: Default::default(),
            collateral_amount_diff: Default::default(),
        })?;
        let amount_to_withdraw = amount * asset_price_in_collateral_asset;
        withdraw_collateral(config, position.cdp_idx, amount_to_withdraw)
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}