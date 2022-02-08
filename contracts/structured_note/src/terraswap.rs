use cosmwasm_std::{Addr, CosmosMsg, Deps, Env, QueryRequest, Response, StdResult, SubMsg, to_binary, Uint256, WasmMsg, WasmQuery};
use cw20::Cw20ExecuteMsg;
use terraswap::asset::{AssetInfo, PairInfo};
use terraswap::pair::Cw20HookMsg::Swap;
use terraswap::querier::query_pair_info;

use crate::state::{DepositingState, load_config};
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

pub fn sell_asset(env: Env, depositing_state: &DepositingState, pair_addr: &Addr) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: depositing_state.masset_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: pair_addr.to_string(),
                    amount: depositing_state.masset_amount_to_sell,
                    msg: to_binary(&Swap {
                        belief_price: None,
                        max_spread: None,
                        to: Some(env.contract.address.to_string()),
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::DepositToCDP.id(),
        ))
        .add_attributes(vec![
            ("action", "sell_asset"),
            ("masset_addr", depositing_state.masset_token.to_string()),
            ("amount_to_sell", depositing_state.masset_amount_to_sell.to_string()),
        ]))
}