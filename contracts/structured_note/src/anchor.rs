use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Coin, CosmosMsg, DepsMut, Response, StdError, StdResult, SubMsg, to_binary, WasmMsg};

use structured_note_package::anchor::AnchorMarketMsg;

use crate::state::{DepositingState, load_config, store_depositing_state};
use crate::SubmsgIds;

pub fn deposit_stable(deps: DepsMut, depositing_state: &mut DepositingState) -> StdResult<Response> {
    let config = load_config(deps.storage)?;

    let deposit_coin = Coin {
        denom: config.stable_denom.clone(),
        amount: depositing_state.amount_to_deposit_to_anc,
    };

    // Every iteration starts with iteration index incrementation, cause every iteration starts/ends here
    depositing_state.cur_iteration_index += 1;

    let mut submsg_id = Default::default();

    if depositing_state.cur_iteration_index == depositing_state.max_iteration_index {
        submsg_id = SubmsgIds::Exit.id();
    }

    if depositing_state.cdp_idx.is_none() {
        submsg_id = SubmsgIds::OpenCDP.id();
    } else {
        //To calculate total position collateral and loan afterwards
        if depositing_state.cur_iteration_index == 1 {
            //TODO: implement queries to get CDP current collateral and loan amounts
            depositing_state.initial_cdp_collateral_amount == Default::default();
            depositing_state.initial_cdp_loan_amount == Default::default();
        }
        submsg_id = SubmsgIds::DepositToCDP.id();
    }

    store_depositing_state(deps.storage, depositing_state)?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.anchor_market_contract.to_string(),
            msg: to_binary(&AnchorMarketMsg::DepositStable {})?,
            funds: vec![deposit_coin],
        }), submsg_id)
            .add_attributes(vec![
                ("action", "deposit_stable_to_anchor_market"),
                ("amount", &amount.to_string()),
            ])))
}

pub fn get_minted_amount_form_deposit_response(response: Response) -> StdResult<String> {
    response.events
        .into_iter()
        .map(|event| event.attributes)
        .flatten()
        .find(|attr| attr.key == "mint_amount")
        .map(|attr| attr.value)
        .ok_or_else(|| {
            StdError::generic_err("Fail to deposit UST to Anchor Money Market")
        })
}