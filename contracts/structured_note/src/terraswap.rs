use cosmwasm_std::{Addr, Coin, CosmosMsg, Deps, Env, Response, StdResult, SubMsg, to_binary, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use terraswap::asset::{Asset, AssetInfo, PairInfo};
use terraswap::pair::Cw20HookMsg::Swap as Cw20HookSwap;
use terraswap::pair::ExecuteMsg::Swap;
use terraswap::querier::query_pair_info;

use crate::state::{Config, DepositState, load_config};
use crate::SubmsgIds;

pub fn query_pair_addr(deps: Deps, terraswap_factory_addr: &Addr, masset_token: &Addr) -> StdResult<String> {
    let config = load_config(deps.storage)?;
    let pair_info: PairInfo = query_pair_info(
        &deps.querier,
        terraswap_factory_addr.clone(),
        &[
            AssetInfo::NativeToken {
                denom: config.stable_denom,
            },
            AssetInfo::Token {
                contract_addr: masset_token.to_string(),
            },
        ],
    )?;
    Ok(pair_info.contract_addr)
}

pub fn sell_asset(env: Env, state: &DepositState, minted_amount: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.masset_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: state.pair_addr.to_string(),
                    amount: minted_amount,
                    msg: to_binary(&Cw20HookSwap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(env.contract.address.to_string()),
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::DepositOnReply.id(),
        ))
        .add_attributes(vec![
            ("action", "sell_asset"),
            ("masset_token", &state.masset_token.to_string()),
            ("amount_to_sell", &minted_amount.to_string()),
        ]))
}

pub fn buy_asset(config: Config, state: DepositState, contract_addr: String, offer_amount: Uint128) -> StdResult<Response> {
    let offer_asset = Coin {
        denom: config.stable_denom.clone(),
        amount: offer_amount,
    };
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: state.pair_addr.to_string(),
            msg: to_binary(&Swap {
                offer_asset: Asset {
                    info: AssetInfo::NativeToken {
                        denom: config.stable_denom,
                    },
                    amount: stable_amount,
                },
                belief_price: None,
                max_spread: None,
                to: Some(contract_addr),
            })?,
            funds: vec![offer_asset],
        }), SubmsgIds::BurnAsset.id()))
        .add_attributes(vec![
            ("action", "buy_asset"),
            ("offered_amount", &stable_amount.to_string()),
        ]))
}