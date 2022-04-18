use cosmwasm_std::{Addr, Coin, CosmosMsg, Response, StdResult, SubMsg, to_binary, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;

use structured_note_package::anchor::{AnchorCW20HookMsg, AnchorMarketMsg};

use crate::state::Config;
use crate::SubmsgIds;

pub fn deposit_stable(anchor_market_contract: Addr, coin : Coin) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: anchor_market_contract.to_string(),
            msg: to_binary(&AnchorMarketMsg::DepositStable {})?,
            funds: vec![coin.clone()],
        }), SubmsgIds::DepositStable.id(),
        ))
        .add_attributes(vec![
            ("action", "deposit_stable_to_anchor_market"),
            ("amount", &coin.amount.to_string()),
        ]))
}

pub fn redeem_stable(config: Config, amount: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.aterra_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.anchor_market_contract.to_string(),
                    amount,
                    msg: to_binary(&AnchorCW20HookMsg::RedeemStable {})?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::RedeemStable.id(),
        ))
        .add_attributes(vec![
            ("action", "redeem_stable"),
            ("aterra_amount", &amount.to_string()),
        ]))
}