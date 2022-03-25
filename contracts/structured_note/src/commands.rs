use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{BalanceResponse, BankMsg, BankQuery, Coin, CosmosMsg, Decimal, DepsMut, Env, Fraction, MessageInfo, QueryRequest, Response, StdError, StdResult, Uint128};

use structured_note_package::mirror::MirrorAssetConfigResponse;

use crate::anchor::deposit_stable as anc_deposit_stable;
use crate::mirror::{get_assets_prices, query_masset_config, query_mirror_mint_config, withdraw_collateral};
use crate::state::{add_farmer_to_cdp, DepositState, load_config, load_position, load_withdraw_state, may_load_cdp, may_load_position, Position, remove_farmer_from_cdp, remove_position, save_deposit_state, save_is_open, save_position, save_withdraw_state, update_is_open, WithdrawState};
use crate::terraswap::query_pair_addr;
use crate::utils::{decimal_division, decimal_multiplication};

pub fn deposit(
    deps: DepsMut,
    info: MessageInfo,
    masset_token: String,
    leverage: Option<u8>,
    aim_collateral_ratio: Decimal,
) -> StdResult<Response> {
    save_is_open(deps.storage, false)?;
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
        save_deposit_state(deps.storage, &DepositState {
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
            save_deposit_state(deps.storage, &DepositState {
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
                "There isn't position: farmer_addr: {}, masset_token: {}. To create new position provide 'leverage'",
                &info.sender.to_string(),
                &masset_token.to_string())));
        }
        if let Some(cdp) = may_load_cdp(deps.storage, &masset_token)? {
            let position = save_position(deps.storage, &Position {
                farmer_addr: info.sender.clone(),
                masset_token: masset_token.clone(),
                cdp_idx: cdp.idx,
                leverage: 0,
                loan: Default::default(),
                collateral: Default::default(),
                aim_collateral_ratio,
            })?;
            add_farmer_to_cdp(deps.storage, cdp.idx, info.sender, masset_token)?;
        } else {
            update_is_open(deps.storage, true)?;
        }
    }
    anc_deposit_stable(config, deposit_amount)
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

pub fn exit(position: Position) -> StdResult<Response> {
    Ok(Response::new()
        .add_attributes(vec![
            ("action", "deposit_stable"),
            ("farmet_addr", &position.farmer_addr.to_string()),
            ("masset_token", &position.masset_token.to_string()),
            ("collateral", &position.collateral.to_string()),
            ("loan", &position.loan.to_string()),
        ]))
}

pub fn withdraw(deps: DepsMut, info: MessageInfo, masset_token: String, aim_collateral: Uint128, aim_collateral_ratio: Decimal) -> StdResult<Response> {
    let masset_token = deps.api.addr_validate(&masset_token)?;

    if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        if position.collateral < aim_collateral {
            return Err(StdError::generic_err("Invalid msg: aim_collateral is greater then current!"));
        };

        let config = load_config(deps.storage)?;
        let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;
        let safe_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
        if aim_collateral_ratio < safe_collateral_ratio {
            return Err(StdError::generic_err(format!("aim_collateral_ratio lower than safe_collateral_ratio: {}", &safe_collateral_ratio)));
        };

        let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;
        let (collateral_price, masset_price) = get_assets_prices(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
        let masset_price_in_collateral_asset = decimal_division(collateral_price, masset_price)?;
        let loan_in_collateral_asset = position.loan * masset_price_in_collateral_asset;
        let current_collateral_ratio = Decimal::from_ratio(position.collateral, loan_in_collateral_asset);
        if aim_collateral_ratio > current_collateral_ratio {
            return Err(StdError::generic_err(format!("aim_collateral_ratio greater than current_collateral_ratio: {}", &current_collateral_ratio)));
        };

        let aim_loan_in_collateral_asset = Uint128::from(aim_collateral.u128() * aim_collateral_ratio.denominator() / aim_collateral_ratio.numerator());
        let aim_loan = Uint128::from(aim_loan_in_collateral_asset.u128() * masset_price_in_collateral_asset.denominator() / masset_price_in_collateral_asset.numerator());

        let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

        save_withdraw_state(deps.storage, &WithdrawState {
            is_raw: false,
            farmer_addr: position.farmer_addr,
            masset_token: position.masset_token,
            aim_collateral,
            aim_loan,
            pair_addr,
            collateral_price,
            masset_price,
            safe_collateral_ratio,
        });
        let amount_to_withdraw = calculate_withdraw_amount(position.collateral, position.loan, aim_loan, masset_price_in_collateral_asset, safe_collateral_ratio);
        withdraw_collateral(config, position.cdp_idx, amount_to_withdraw)
    } else {
        Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())))
    }
}

pub fn raw_withdraw(deps: DepsMut, info: MessageInfo, masset_token: String, aim_collateral: Uint128) -> StdResult<Response> {
    let masset_token = deps.api.addr_validate(&masset_token)?;

    if let Some(position) = may_load_position(deps.storage, &info.sender, &masset_token)? {
        if position.collateral < aim_collateral {
            return Err(StdError::generic_err("Invalid msg: aim_collateral is greater then current!"));
        };
        let config = load_config(deps.storage)?;
        let mirror_mint_config = query_mirror_mint_config(deps.as_ref(), config.mirror_mint_contract.to_string())?;

        let (collateral_price, masset_price) = get_assets_prices(deps.as_ref(), &mirror_mint_config, &config, &masset_token)?;
        let masset_price_in_collateral_asset = decimal_division(collateral_price, masset_price)?;

        let masset_config = query_masset_config(deps.as_ref(), &masset_token)?;
        let safe_collateral_ratio = decimal_multiplication(&masset_config.min_collateral_ratio, &config.min_over_collateralization);
        let loan_in_collateral_asset = position.loan * masset_price_in_collateral_asset;
        let min_safe_collateral = Uint128::from(loan_in_collateral_asset.u128() * safe_collateral_ratio.denominator() / safe_collateral_ratio.numerator());
        if aim_collateral < min_safe_collateral {
            return Err(StdError::generic_err("aim_collateral too low for raw withdraw"));
        };

        let pair_addr = deps.api.addr_validate(&query_pair_addr(deps.as_ref(), &deps.api.addr_validate(&mirror_mint_config.terraswap_factory)?, &masset_token)?)?;

        save_withdraw_state(deps.storage, &WithdrawState {
            is_raw: true,
            farmer_addr: position.farmer_addr,
            masset_token: position.masset_token,
            aim_collateral,
            aim_loan: Uint128::default(),
            pair_addr,
            collateral_price,
            masset_price,
            safe_collateral_ratio,
        });
        withdraw_collateral(config, position.cdp_idx, position.collateral - aim_collateral)
    } else {
        Err(StdError::generic_err(format!(
            "There isn't position: farmer_addr: {}, masset_token: {}.",
            &info.sender.to_string(),
            &masset_token.to_string())))
    }
}

pub fn is_aim_state(position: &Position, state: &WithdrawState) -> bool {
    position.collateral == state.aim_collateral && position.loan == state.aim_loan
}

pub fn calculate_withdraw_amount(collateral: Uint128, loan: Uint128, aim_collateral: Uint128, masset_price_in_collateral_asset: Decimal, safe_collateral_ratio: Decimal) -> Uint128 {
    let loan_in_collateral_asset = loan * masset_price_in_collateral_asset;
    let min_safe_collateral = Uint128::from(loan_in_collateral_asset.u128() * safe_collateral_ratio.denominator() / safe_collateral_ratio.numerator());
    let max_safe_withdraw = collateral - min_safe_collateral;
    if aim_collateral < min_safe_collateral {
        collateral - aim_collateral
    } else {
        max_safe_withdraw
    }
}

pub fn return_stable(deps: DepsMut, env: Env) -> StdResult<Response> {
    let state = load_withdraw_state(deps.storage)?;
    let position = load_position(deps.storage, &state.farmer_addr, &state.masset_token)?;
    let config = load_config(deps.storage)?;
    if position.collateral == Uint128::zero() {
        remove_position(deps.storage, &position.farmer_addr, &position.masset_token);
        remove_farmer_from_cdp(deps.storage, &position.farmer_addr, &position.masset_token)?;
    };
    let balance: BalanceResponse = deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: env.contract.address.to_string(),
        denom: config.stable_denom.clone(),
    }))?;
    Ok(Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: position.farmer_addr.to_string(),
            amount: vec![
                Coin {
                    denom: config.stable_denom,
                    amount: balance.amount.amount,
                }],
        }))
        .add_attributes(vec![
            ("action", "return_stable"),
            ("return_amount", &balance.amount.amount.to_string()),
        ]))
}