use std::convert::TryFrom;

use cosmwasm_std::StdError;

pub mod state;
mod anchor;
mod mirror;
mod terraswap;
mod contract;
mod commands;
mod utils;

pub enum SubmsgIds {
    OpenCDP,
    DepositToCDP,
    MintAssetWithAimCollateralRatio,
    SellAsset,
    DepositStableOnReply,
    //last deposit to anchor and exit (no submsgs)
    Exit,
}

impl TryFrom<u64> for SubmsgIds {
    type Error = StdError;

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        match v {
            x if x == SubmsgIds::OpenCDP.id() => Ok(SubmsgIds::OpenCDP),
            x if x == SubmsgIds::DepositToCDP.id() => Ok(SubmsgIds::DepositToCDP),
            x if x == SubmsgIds::MintAssetWithAimCollateralRatio.id() => Ok(SubmsgIds::MintAssetWithAimCollateralRatio),
            x if x == SubmsgIds::SellAsset.id() => Ok(SubmsgIds::SellAsset),
            x if x == SubmsgIds::DepositStableOnReply.id() => Ok(SubmsgIds::DepositStableOnReply),
            x if x == SubmsgIds::Exit.id() => Ok(SubmsgIds::Exit),
            unknown => Err(StdError::generic_err(format!(
                "unknown reply message id: {}",
                unknown
            ))),
        }
    }
}