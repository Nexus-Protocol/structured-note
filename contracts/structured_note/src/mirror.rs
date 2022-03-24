use cosmwasm_std::{Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, QueryRequest, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg, WasmQuery};
use cosmwasm_storage::to_length_prefixed;
use cw20::Cw20ExecuteMsg;
use terraswap::asset::{Asset, AssetInfo};

use structured_note_package::mirror::{CDPState, MirrorAssetConfigResponse, MirrorCDPResponse, MirrorCollateralOracleQueryMsg, MirrorCollateralPriceResponse, MirrorMintConfigResponse, MirrorMintCW20HookMsg, MirrorMintExecuteMsg, MirrorOracleQueryMsg, MirrorPriceResponse};

use crate::{concat, SubmsgIds};
use crate::state::{Config, DepositState, increase_position_collateral, increment_iteration_index, load_config, WithdrawState, WithdrawType};
use crate::utils::decimal_division;

pub fn query_mirror_mint_config(deps: Deps, mirror_mint_contract: String) -> StdResult<MirrorMintConfigResponse> {
    let mirror_mint_config: MirrorMintConfigResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Raw {
            contract_addr: mirror_mint_contract,
            key: Binary::from(b"config"),
        }))?;
    Ok(mirror_mint_config)
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
                cdp_idx.to_string().as_bytes(),
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

pub fn query_collateral_price(deps: Deps, collateral_oracle_addr: &Addr, aterra_addr: &Addr) -> StdResult<Decimal> {
    let res: MirrorCollateralPriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: collateral_oracle_addr.to_string(),
        msg: to_binary(&MirrorCollateralOracleQueryMsg::CollateralPrice {
            asset: aterra_addr.to_string(),
            block_height: None,
        })?,
    }))?;
    Ok(res.rate)
}

pub fn query_asset_price(deps: Deps, oracle_addr: &Addr, asset_addr: &Addr, base_asset: String) -> StdResult<Decimal> {
    let res: MirrorPriceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: oracle_addr.to_string(),
        msg: to_binary(&MirrorOracleQueryMsg::Price {
            base_asset,
            quote_asset: asset_addr.to_string(),
        })?,
    }))?;
    Ok(res.rate)
}

pub fn get_assets_prices(deps: Deps, mirror_mint_config: &MirrorMintConfigResponse, config: &Config, masset_token: &Addr) -> StdResult<(Decimal, Decimal)> {
    let collateral_oracle = deps.api.addr_validate(&mirror_mint_config.collateral_oracle)?;
    let collateral_price = query_collateral_price(deps, &collateral_oracle, &config.aterra_addr)?;

    let oracle_addr = deps.api.addr_validate(&mirror_mint_config.oracle)?;
    let masset_price = query_asset_price(deps, &oracle_addr, masset_token, config.stable_denom.clone())?;

    Ok((collateral_price, masset_price))
}

pub fn open_cdp(config: Config, state: DepositState, received_aterra_amount: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.aterra_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.mirror_mint_contract.to_string(),
                    amount: received_aterra_amount,
                    msg: to_binary(&MirrorMintCW20HookMsg::OpenPosition {
                        asset_info: AssetInfo::Token {
                            contract_addr: state.masset_token.to_string()
                        },
                        collateral_ratio: state.aim_collateral_ratio,
                        short_params: None,
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::SellAsset.id(),
        ))
        .add_attributes(vec![
            ("action", "open_cdp"),
            ("collateral_amount", &received_aterra_amount.to_string()),
            ("masset_token", &state.masset_token.to_string()),
            ("aim_collateral_ratio", &state.aim_collateral_ratio.to_string()),
        ]))
}

pub fn deposit_to_cdp(deps: DepsMut, received_aterra_amount: Uint128) -> StdResult<Response> {
    let config = load_config(deps.storage)?;
    //Increment iteration index here 'cause exit is on this step
    let state = increment_iteration_index(deps.storage)?;

    let submsg_id =
        if state.cur_iteration_index > state.leverage {
            SubmsgIds::Exit.id()
        } else {
            SubmsgIds::MintAsset.id()
        };

    let position = increase_position_collateral(deps.storage, &state.farmer_addr, &state.masset_token, received_aterra_amount)?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.aterra_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.mirror_mint_contract.to_string(),
                    amount: received_aterra_amount,
                    msg: to_binary(&MirrorMintCW20HookMsg::Deposit {
                        position_idx: position.cdp_idx,
                    })?,
                })?,
                funds: vec![],
            }),
            submsg_id,
        ))
        .add_attributes(vec![
            ("action", "deposit_to_cdp"),
            ("collateral_amount", &received_aterra_amount.to_string()),
            ("masset_token", &state.masset_token.to_string()),
        ]))
}

pub fn mint_asset(config: Config, cdp_idx: Uint128, masset_token: String, amount_to_mint: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.mirror_mint_contract.to_string(),
            msg: to_binary(&MirrorMintExecuteMsg::Mint {
                position_idx: cdp_idx,
                asset: Asset {
                    info: AssetInfo::Token { contract_addr: masset_token },
                    amount: amount_to_mint,
                },
                short_params: None,
            })?,
            funds: vec![],
        }), SubmsgIds::SellAsset.id(),
        ))
        .add_attributes(vec![
            ("action", "mint_asset"),
            ("masset_token", &masset_token.to_string()),
            ("mint_amount", &amount_to_mint.to_string()),
        ]))
}

pub fn withdraw_collateral(config: Config, cdp_idx: Uint128, amount_to_withdraw: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.mirror_mint_contract.to_string(),
            msg: to_binary(&MirrorMintExecuteMsg::Withdraw {
                position_idx: cdp_idx,
                collateral: Some(Asset {
                    info: AssetInfo::Token { contract_addr: config.aterra_addr.to_string() },
                    amount: amount_to_withdraw,
                }),
            })?,
            funds: vec![],
        }), SubmsgIds::WithdrawCollateral.id(),
        )).add_attributes(vec![
        ("action", "withdraw_collateral"),
        ("cdp_idx", &cdp_idx.to_string()),
        ("amount", &amount_to_withdraw.to_string()),
    ]))
}

pub fn burn_asset(config: Config, state: WithdrawState, cdp_idx: Uint128, return_amount: Uint128) -> StdResult<Response> {
    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.masset_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.mirror_mint_contract.to_string(),
                    amount: return_amount,
                    msg: to_binary(&MirrorMintCW20HookMsg::Burn {
                        position_idx: cdp_idx,
                    })?,
                })?,
                funds: vec![],
            }),
            SubmsgIds::BurnAsset.id(),
        ))
        .add_attributes(vec![
            ("action", "burn_asset"),
            ("cdp_idx", &cdp_idx.to_string()),
            ("amount", &return_amount.to_string()),
        ]))
}