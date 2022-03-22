use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{BalanceResponse, BankQuery, Decimal, Deps, DepsMut, Env, Fraction, MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128};
use cw20::Cw20ExecuteMsg;

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::{get_assets_prices, query_masset_config, query_mirror_mint_config, withdraw_collateral};
use crate::state::{add_farmer_to_cdp, decrease_position_collateral, DepositState, load_config, may_load_cdp, may_load_position, Position, remove_farmer_from_cdp, remove_position, save_is_closure, save_position, store_deposit_state, store_withdraw_state, WithdrawState, WithdrawType};
use crate::terraswap::query_pair_addr;
use crate::utils::{decimal_division, decimal_multiplication};

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

    let (collateral_price, asset_price) = get_assets_prices(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
    let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price)?;

    let min_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
    if aim_collateral_ratio < min_collateral_ratio {
        return Err(StdError::generic_err("Aim collateral ratio too low"));
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
        store_deposit_state(deps.storage, &DepositState {
            farmer_addr: p.farmer_addr,
            masset_token: p.masset_token,
            leverage: p.leverage,
            cur_iteration_index: 0,
            asset_price_in_collateral_asset,
            pair_addr,
            aim_collateral_ratio,
        })?;
    } else {
        if let Some(leverage) = leverage {
            if !(1..=5).contains(&leverage) {
                return Err(StdError::generic_err("Invalid message: leverage iterations amount should be from 1 to 5."));
            }
            store_deposit_state(deps.storage, &DepositState {
                farmer_addr: info.sender.clone(),
                masset_token: masset_token.clone(),
                leverage,
                cur_iteration_index: 0,
                asset_price_in_collateral_asset,
                pair_addr,
                aim_collateral_ratio,
            })?;
        } else {
            return Err(StdError::generic_err(format!(
                "There isn't position: farmer_addr: {}, masset_token: {}.",
                &info.sender.to_string(),
                &masset_token.to_string())));
        }
        if let Some(cdp) = may_load_cdp(deps.storage, &masset_token)? {
            let position = save_position(deps.storage, &Position {
                farmer_addr: info.sender.clone(),
                masset_token: masset_token.clone(),
                cdp_idx: cdp.idx,
                leverage: 0,
                loan_amount: Default::default(),
                collateral_amount: Default::default(),
                aim_collateral_ratio,
            })?;
            add_farmer_to_cdp(deps.storage, cdp.idx, info.sender, masset_token)?;
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
    anc_deposit_stable(load_config(deps.storage)?, false, deposit_amount)
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

pub fn exit() -> StdResult<Response> {
    //TODO: load position and share it in attributes?
    Ok(Response::new()
        .add_attribute("method", "exit"))
}

pub fn close(deps: DepsMut, info: MessageInfo, masset_token: String) -> StdResult<Response> {
    let masset_token = deps.api.addr_validate(&masset_token)?;
    let config = load_config(deps.storage)?;
    let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

    let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

    let (collateral_price, asset_price) = get_assets_prices(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
    let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price)?;

    let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;

    let min_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);

    if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        store_deposit_state(deps.storage, &DepositState {
            farmer_addr: position.farmer_addr,
            masset_token: position.masset_token,
            leverage: position.leverage,
            cur_iteration_index: 0,
            asset_price_in_collateral_asset: asset_price_in_collateral_asset.clone(),
            pair_addr,
            aim_collateral_ratio: min_collateral_ratio,
        })?;

        let withdrawable_collateral = position.collateral_amount - position.loan_amount * asset_price_in_collateral_asset * min_collateral_ratio;

        decrease_position_collateral(deps.storage, &info.sender, &masset_token, withdrawable_collateral)?;

        withdraw_collateral(config, position.cdp_idx, withdrawable_collateral)
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}

pub fn close_on_reply(deps: DepsMut, state: DepositState) -> StdResult<Response> {
    if let Some(position) = may_load_position(deps.storage, &state.farmer_addr, &state.masset_token)? {
        if position.collateral_amount.is_zero() {
            return exit_on_close(deps, state);
        };

        let withdrawable_collateral = position.collateral_amount - position.loan_amount * state.asset_price_in_collateral_asset * state.aim_collateral_ratio;

        withdraw_collateral(load_config(deps.storage)?, position.cdp_idx, withdrawable_collateral)
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &state.farmer_addr.to_string(),
            &state.masset_token.to_string())));
    }
}

pub fn withdraw(deps: DepsMut, info: MessageInfo, masset_token: String, aim_collateral_amount: Uint128, aim_collateral_ratio: Decimal) -> StdResult<Response> {
    let masset_token = deps.api.addr_validate(&masset_token)?;

    if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        if aim_collateral_amount > position.collateral_amount {
            return Err(StdError::generic_err(format!("aim_collateral_amount greater than current_collateral amount: {}", &position.collateral_amount)));
        }

        let config = load_config(deps.storage)?;

        let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;

        let safe_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
        if aim_collateral_ratio < safe_collateral_ratio {
            return Err(StdError::generic_err(format!("aim_collateral_ratio lower than safe_collateral_ratio: {}", &safe_collateral_ratio)));
        };

        let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;
        let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;
        let (collateral_price, asset_price) = get_assets_prices(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
        let asset_price_in_collateral_asset = decimal_division(collateral_price, asset_price)?;

        let loan_value_in_collateral_asset = position.loan_amount * asset_price_in_collateral_asset;
        let current_collateral_ratio = Decimal::from_ratio(position.collateral_amount, loan_value_in_collateral_asset);
        if aim_collateral_ratio > current_collateral_ratio {
            return Err(StdError::generic_err(format!("aim_collateral_ratio greater than current_collateral_ratio: {}", &current_collateral_ratio)));
        };

        let repay_value_in_collateral_asset = loan_value_in_collateral_asset - Uint128::from(aim_collateral_amount.u128() * aim_collateral_ratio.denominator() / aim_collateral_ratio.numerator());
        let repay_value = repay_value_in_collateral_asset * collateral_price;
        //simple withdraw
        if Decimal::from_ratio(aim_collateral_amount, loan_value_in_collateral_asset) > safe_collateral_ratio {
            store_withdraw_state(deps.storage, &WithdrawState {
                withdraw_type: WithdrawType::Simple,
                farmer_addr: position.farmer_addr.clone(),
                masset_token: position.masset_token.clone(),
                aim_collateral_amount,
                aim_collateral_ratio,
                pair_addr: pair_addr.clone(),
                repay_value,
            })?;
            let collateral_diff = position.collateral_amount - aim_collateral_amount;
            decrease_position_collateral(deps.storage, &position.farmer_addr, &position.masset_token, collateral_diff)?;
            withdraw_collateral(config.clone(), position.cdp_idx, collateral_diff);
        };

        let max_withdraw_amount = position.collateral_amount - (loan_value_in_collateral_asset * safe_collateral_ratio);

        //double withdraw
        if repay_value_in_collateral_asset < max_withdraw_amount {
            //TODO:
            // - withdraw collateral to safe_cr (max_withdraw)
            // - burn all received aUST to get stable
            // - buy and burn mAsset for repay_value
            // - withdraw collateral to aim_collateral
            // - burn all received aUST
            // - sent all UST in contract to the farmer
            store_withdraw_state(deps.storage, &WithdrawState {
                withdraw_type: WithdrawType::Double,
                farmer_addr: position.farmer_addr,
                masset_token: position.masset_token,
                aim_collateral_amount,
                aim_collateral_ratio,
                pair_addr,
                repay_value: Uint128::default(), //not used, repay all stable in contract
            })?;
            decrease_position_collateral(deps.storage, &position.farmer_addr, &position.masset_token, max_withdraw_amount)?;
            withdraw_collateral(config, position.cdp_idx, max_withdraw_amount);
            // recursive withdraw
        } else {
            //TODO:
            // - withdraw collateral to safe_cr (max_withdraw)
            // - burn all received aUST to get stable
            // - buy and burn mAsset for all stable in contract
            // - call this fn again
        }
        Ok()
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}

fn exit_on_close(deps: DepsMut, state: DepositState) -> StdResult<Response> {
    remove_position(deps.storage, &state.farmer_addr, &state.masset_token);
    remove_farmer_from_cdp(deps.storage, &state.farmer_addr, &state.masset_token)?;
    Ok(Response::new()
        .add_attribute("action", "close_position"))
}

pub fn return_stable(deps: DepsMut, env: Env) -> StdResult<Response> {
    if let Some(position) = may_load_position(deps.storage, farmer_addr, masset)? {
        let config = load_config(deps.storage)?;
        if position == Uint128::zero() {
            remove_position(deps.storage, &position.farmer_addr, &position.masset_token)?;
        };
        let balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
            address: env.contract.address.to_string(),
            denom: config.stable_denom,
        }))?;
        Ok(Response::new()
            .add_message(Cw20ExecuteMsg::Transfer {
                recipient: position.farmer_addr.to_string(),
                amount: balance.amount.amount,
            })
            .add_attributes(vec![
                ("action", "return_stable"),
                ("return_amount", balance.amount.amount.to_string()),
            ]))
    } else {
        return Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())));
    }
}