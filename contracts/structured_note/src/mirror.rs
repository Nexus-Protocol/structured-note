use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Addr, CosmosMsg, Deps, QueryRequest, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg, WasmQuery};
use cosmwasm_storage::to_length_prefixed;
use cw20::Cw20ExecuteMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terraswap::asset::{Asset, AssetInfo};

use structured_note_package::mirror::{CDPState, MirrorAssetConfigResponse, MirrorCDPResponse, MirrorMintConfigResponse, MirrorMintCW20HookMsg, MirrorMintExecuteMsg};

use crate::state::{Config, DepositingState, load_config};
use crate::SubmsgIds;

pub fn query_mirror_ts_factory(deps: Deps) -> StdResult<String> {
    let config = load_config(deps.storage)?;

    let mirror_mint_config: MirrorMintConfigResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: config.mirror_mint_contract.to_string(),
            key: Binary::from(b"config"),
        }))?;
    Ok(mirror_mint_config.terraswap_factory)
}

pub fn query_masset_config(deps: Deps, masset_token: &Addr) -> StdResult<MirrorAssetConfigResponse> {
    let config = load_config(deps.storage)?;

    let masset_config: StdResult<MirrorAssetConfigResponse> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: config.mirror_mint_contract.to_string(),
            key: Binary::from(concat(
                &to_length_prefixed(b"asset_config"),
                masset_token.as_bytes(),
            )),
        }));

    match masset_config {
        Ok(a) => Ok(MirrorAssetConfigResponse {
            token: a.token,
            auction_discount: a.auction_discount,
            min_collateral_ratio: a.min_collateral_ratio,
            end_price: a.end_price,
            ipo_params: a.ipo_params,
        }),
        Err(_) => Err(StdError::generic_err("Mirror asset config query failed".to_string()))
    }
}

pub fn query_cdp(deps: Deps, cdp_idx: Uint128) -> StdResult<CDPState> {
    let config = load_config(deps.storage)?;

    let cdp: StdResult<MirrorCDPResponse> =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: config.mirror_mint_contract.to_string(),
            key: Binary::from(concat(
                &to_length_prefixed(b"position"),
                cdp_idx.as_bytes(),
            )),
        }));

    match cdp {
        Ok(cdp) => Ok(CDPState {
            collateral_amount: cdp.collateral.amount,
            loan_amount: cdp.asset.amount,
        }),
        Err(_) => Err(StdError::generic_err("Mirror position query failed".to_string()))
    }
}

pub fn open_cdp(deps: Deps, depositing_state: &DepositingState) -> StdResult<Response> {
    let config = load_config(deps.storage)?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.aterra_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.mirror_mint_contract.to_string(),
                    amount: depositing_state.amount_aust_to_collateral,
                    msg: to_binary(&MirrorMintCW20HookMsg::OpenPosition {
                        asset_info: AssetInfo::Token {
                            contract_addr: depositing_state.masset_token.to_string()
                        },
                        collateral_ratio,
                        short_params: None,
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::SellAsset.id(),
        ))
        .add_attributes(vec![
            ("action", "open_cdp"),
            ("collateral_amount", depositing_state.amount_aust_to_collateral.to_string()),
            ("masset_addr", depositing_state.masset_token.to_string()),
            ("initial_collateral_ratio", depositing_state.collateral_rate.to_string()),
        ]))
}

pub fn deposit_to_cdp(deps: Deps, depositing_state: &DepositingState) -> StdResult<Response> {
    let config = load_config(deps.storage)?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.aterra_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.mirror_mint_contract.to_string(),
                    amount: depositing_state.amount_aust_to_collateral,
                    msg: to_binary(&MirrorMintCW20HookMsg::Deposit {
                        position_idx: cdp_idx.into()
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::SellAsset.id(),
        ))
        .add_attributes(vec![
            ("action", "deposit_to_cdp"),
            ("collateral_amount", depositing_state.amount_aust_to_collateral.to_string()),
            ("masset_addr", depositing_state.masset_token.to_string()),
            ("initial_collateral_ratio", depositing_state.collateral_rate.to_string()),
        ]))
}