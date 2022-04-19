use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_std::{Binary, Coin, ContractResult, Deps, DepsMut, entry_point, Env, Fraction, MessageInfo, Reply, Response, StdError, StdResult, to_binary, Uint128};

use structured_note_package::structured_note::{ExecuteMsg, InstantiateMsg, QueryMsg};

use crate::anchor::{redeem_stable, deposit_stable_to_anc};
use crate::commands::{calculate_withdraw_amount, deposit, exit, is_aim_state, raw_deposit, raw_withdraw, return_stable, withdraw};
use crate::mirror::{burn_masset, deposit_to_cdp, mint_masset, open_cdp, withdraw_collateral};
use crate::state::{add_farmer_to_cdp, Config, decrease_position_collateral, decrease_position_loan, increase_iteration_index, increase_position_collateral, increase_position_loan, load_cdp, load_config, load_deposit_state, load_is_open, load_is_raw, load_position, load_positions_by_farmer_addr, load_withdraw_state, may_load_position, Position, save_config, save_position, update_is_open};
use crate::SubmsgIds;
use crate::terraswap::{buy_masset, sell_masset};
use crate::utils::{decimal_division, decimal_multiplication, deduct_tax, get_amount_from_response_asset_as_string_attr, get_amount_from_response_raw_attr, query_balance};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    save_config(deps.storage, &Config {
        stable_denom: msg.stable_denom,
        governance_contract: deps.api.addr_validate(&msg.governance_contract)?,
        mirror_mint_contract: deps.api.addr_validate(&msg.mirror_mint_contract)?,
        anchor_market_contract: deps.api.addr_validate(&msg.anchor_market_contract)?,
        aterra_addr: deps.api.addr_validate(&msg.aterra_addr)?,
        nexus_treasury: deps.api.addr_validate(&msg.nexus_treasury)?,
        protocol_fee: msg.protocol_fee,
        min_over_collateralization: msg.min_over_collateralization,
    })?;
    Ok(Response::default())
}
//TODO: v.0.2 check working hours
//TODO: v.0.2 check liquidity
//TODO: v.0.2 check slippage
//TODO: v.0.2 avoid send zero tokens issue: check deposit is enough to -> mint enough aterra to -> borrow enough masset to -> buy enough UST -> etc
#[entry_point]
pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Deposit {
            masset_token,
            leverage,
            aim_collateral_ratio,
        } => {
            deposit(deps, info, masset_token, leverage, aim_collateral_ratio)
        }
        ExecuteMsg::RawDeposit { masset_token } => {
            raw_deposit(deps, info, masset_token)
        }
        ExecuteMsg::Withdraw { masset_token, aim_collateral, aim_collateral_ratio } => {
            withdraw(deps, info, masset_token, aim_collateral, aim_collateral_ratio)
        }
        ExecuteMsg::RawWithdraw { masset_token, amount } => {
            raw_withdraw(deps, info, masset_token, amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    let events = match msg.result {
        ContractResult::Ok(result) => result.events,
        ContractResult::Err(_) => return Err(StdError::generic_err("Fail to parse reply response")),
    };

    let submessage_enum = SubmsgIds::try_from(msg.id)?;
    match submessage_enum {
        SubmsgIds::DepositStableToAnc => {
            let received_aterra_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "mint_amount".to_string())?)?;
            let config = load_config(deps.storage)?;
            let state = load_deposit_state(deps.storage)?;
            if load_is_open(deps.storage)? {
                open_cdp(config, state, received_aterra_amount)
            } else {
                let cdp = load_cdp(deps.storage, &state.masset_token)?;
                deposit_to_cdp(config, cdp.idx, received_aterra_amount)
            }
        }
        SubmsgIds::OpenCDP => {
            update_is_open(deps.storage, false)?;
            let state = increase_iteration_index(deps.storage)?;
            let cdp_idx = Uint128::from_str(&get_amount_from_response_raw_attr(events.clone(), "position_idx".to_string())?)?;
            let minted_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events.clone(), "mint_amount".to_string())?)?;
            let collateral_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "collateral_amount".to_string())?)?;
            save_position(deps.storage, &Position {
                farmer_addr: state.farmer_addr.clone(),
                masset_token: state.masset_token.clone(),
                cdp_idx,
                leverage: state.leverage,
                loan: minted_amount,
                collateral: collateral_amount,
                aim_collateral_ratio: state.aim_collateral_ratio,
            })?;
            add_farmer_to_cdp(deps.storage, cdp_idx, state.farmer_addr.clone(), state.masset_token.clone())?;
            sell_masset(env, &state, minted_amount)
        }
        SubmsgIds::DepositToCDP => {
            let config = load_config(deps.storage)?;
            let state = increase_iteration_index(deps.storage)?;
            let deposit_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "deposit_amount".to_string())?)?;
            let position = increase_position_collateral(deps.storage, &state.farmer_addr, &state.masset_token, deposit_amount)?;
            if load_is_raw(deps.storage)? || state.cur_iteration_index > state.leverage {
                return exit(position);
            };
            //aim_collateral_ratio = collateral_value / aim_loan_value = collateral_amount / (aim_loan_amount * asset_price_in_collateral_asset)
            // aim_loan_amount = collateral_amount/(aim_collateral_ratio * asset_price_in_collateral_asset)
            let coef = decimal_multiplication(&state.aim_collateral_ratio, &state.asset_price_in_collateral_asset);
            let aim_loan_amount = Uint128::from(position.collateral.u128() * coef.denominator() / coef.numerator());

            if aim_loan_amount <= position.loan {
                // impossible case because to decrease loan_amount contract needs to burn some masset_tokens which are not considered to be in the contract atm
                return Err(StdError::generic_err("Aim loan amount is less or equals to actual loan amount. Deposit doesn't handle burning borrowed asset tokens."));
            };
            let mint_amount = aim_loan_amount - position.loan;
            mint_masset(config, position.cdp_idx, state.masset_token.to_string(), mint_amount)
        }
        SubmsgIds::SellMAsset => {
            let return_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "return_amount".to_string())?)?;
            let config = load_config(deps.storage)?;

            let return_amount_without_taxes = deduct_tax(deps.as_ref(), Coin {
                denom: config.stable_denom.clone(),
                amount: return_amount.into(),
            })?;

            deposit_stable_to_anc(config.anchor_market_contract, return_amount_without_taxes)
        }
        SubmsgIds::MintMAsset => {
            let state = load_deposit_state(deps.storage)?;
            let minted_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "mint_amount".to_string())?)?;
            increase_position_loan(deps.storage, &state.farmer_addr, &state.masset_token, minted_amount)?;
            sell_masset(env, &state, minted_amount)
        }
        SubmsgIds::Exit => {
            let state = load_deposit_state(deps.storage)?;
            exit(load_position(deps.storage, &state.farmer_addr, &state.masset_token)?)
        }
        SubmsgIds::WithdrawCollateral => {
            let received_aterra_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "withdraw_amount".to_string())?)?;
            let state = load_withdraw_state(deps.storage)?;
            decrease_position_collateral(deps.storage, &state.farmer_addr, &state.masset_token, received_aterra_amount)?;
            redeem_stable(load_config(deps.storage)?, received_aterra_amount)
        }
        SubmsgIds::RedeemStable => {
            let state = load_withdraw_state(deps.storage)?;
            let config = load_config(deps.storage)?;
            if load_is_raw(deps.storage)? {
                return return_stable(deps, env);
            };
            if let Some(position) = may_load_position(deps.storage, &state.farmer_addr, &state.masset_token)? {
                if is_aim_state(&position, &state) {
                    return return_stable(deps, env);
                };

                let repay_to_aim_value = (position.loan - state.aim_loan) * state.masset_price;
                let stable_balance = query_balance(&deps.querier, &env.contract.address, &config.stable_denom)?;

                let offer_amount = stable_balance.min(repay_to_aim_value);
                buy_masset(config, state, env.contract.address.to_string(), offer_amount)
            } else {
                Err(StdError::generic_err(format!(
                    "There isn't position: farmer_addr: {}, masset_token: {}.",
                    &state.farmer_addr.to_string(),
                    &state.masset_token.to_string())))
            }
        }
        SubmsgIds::BuyMAsset => {
            let state = load_withdraw_state(deps.storage)?;
            let position = load_position(deps.storage, &state.farmer_addr, &state.masset_token)?;
            let return_amount = Uint128::from_str(&get_amount_from_response_raw_attr(events, "return_amount".to_string())?)?;
            burn_masset(load_config(deps.storage)?, load_withdraw_state(deps.storage)?, position.cdp_idx, return_amount)
        }
        SubmsgIds::BurnMAsset => {
            let state = load_withdraw_state(deps.storage)?;
            let burn_amount = Uint128::from_str(&get_amount_from_response_asset_as_string_attr(events, "burn_amount".to_string())?)?;
            let position = decrease_position_loan(deps.storage, &state.farmer_addr, &state.masset_token, burn_amount)?;
            let state = load_withdraw_state(deps.storage)?;
            let config = load_config(deps.storage)?;
            if is_aim_state(&position, &state) {
                return return_stable(deps, env);
            };
            let masset_price_in_collateral_asset = decimal_division(state.collateral_price, state.masset_price)?;
            let amount_to_withdraw = calculate_withdraw_amount(position.collateral, position.loan, state.aim_collateral, masset_price_in_collateral_asset, state.safe_collateral_ratio);
            withdraw_collateral(config, position.cdp_idx, amount_to_withdraw)
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&load_config(deps.storage)?),
        QueryMsg::Position { masset_token } => {
            let masset_token = deps.api.addr_validate(&masset_token)?;
            match may_load_position(deps.storage, &env.contract.address, &masset_token)?{
                Some(position) => to_binary(&position),
                None => Err(StdError::generic_err("There is no position")),
            }
        },
        QueryMsg::FarmersPositions {farmer_addr } => {
            let farmer_addr = deps.api.addr_validate(&farmer_addr)?;
            to_binary(&load_positions_by_farmer_addr(deps.storage, &farmer_addr)?)
        },
    }
}