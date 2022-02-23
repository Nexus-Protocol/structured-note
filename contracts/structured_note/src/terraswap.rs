use cosmwasm_std::{Addr, CosmosMsg, Deps, Env, Response, StdResult, SubMsg, to_binary, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use terraswap::asset::{AssetInfo, PairInfo};
use terraswap::pair::Cw20HookMsg::Swap;
use terraswap::querier::query_pair_info;

use structured_note_package::mirror::MirrorMintConfigResponse;

use crate::state::{Config, load_config, Position, State};
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

pub fn sell_asset(env: Env, state: &State, minted_amount: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.masset_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: state.mirror_ts_factory_addr.to_string(),
                    amount: minted_amount,
                    msg: to_binary(&Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(env.contract.address.to_string()),
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::DepositStableOnReply.id(),
        ))
        .add_attributes(vec![
            ("action", "sell_asset"),
            ("masset_token", &state.masset_token.to_string()),
            ("amount_to_sell", &minted_amount.to_string()),
        ]))
}

pub fn buy_asset(env: Env, masset_token: &Addr, mirror_ts_factory_addr: &Addr, minted_amount: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: masset_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: mirror_ts_factory_addr.to_string(),
                    amount: minted_amount,
                    msg: to_binary(&Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(env.contract.address.to_string()),
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::BurnAsset.id(),
        ))
        .add_attributes(vec![
            ("action", "buy_asset"),
            ("masset_token", &masset_token.to_string()),
        ]))
}