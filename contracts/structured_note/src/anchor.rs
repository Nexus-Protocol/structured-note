use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Coin, CosmosMsg, DepsMut, Response, StdResult, SubMsg, to_binary, Uint128, WasmMsg};

use structured_note_package::anchor::AnchorMarketMsg;

use crate::state::{DepositingState, load_config, store_depositing_state};
use crate::SubmsgIds;

pub fn deposit_stable(deps: DepsMut, depositing_state: &mut DepositingState, deposit_amount: Uint256) -> StdResult<Response> {
    let config = load_config(deps.storage)?;

    let deposit_coin = Coin {
        denom: config.stable_denom.clone(),
        amount: deposit_amount.into(),
    };

    let submsg_id =
        if depositing_state.cdp_idx == Uint128::zero() {
            SubmsgIds::OpenCDP.id()
        } else {
            SubmsgIds::DepositToCDP.id()
        };

    store_depositing_state(deps.storage, depositing_state)?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.anchor_market_contract.to_string(),
            msg: to_binary(&AnchorMarketMsg::DepositStable {})?,
            funds: vec![deposit_coin],
        }),
                                                 submsg_id,
        ))
        .add_attributes(vec![
            ("action", "deposit_stable_to_anchor_market"),
            ("amount", &deposit_amount.to_string()),
        ]))
}

pub fn redeem_stable() -> StdResult<Response> {}