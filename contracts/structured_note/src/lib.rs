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
    //Deposit
    OpenCDP,
    DepositToCDP,
    MintAssetWithAimCollateralRatio,
    SellAsset,
    DepositStableOnReply,
    Exit,
    //Withdraw
    RedeemStable,
    BuyAsset,
    BurnAsset,
    WithdrawCollateralOnReply,
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
            x if x == SubmsgIds::RedeemStable.id() => Ok(SubmsgIds::RedeemStable),
            x if x == SubmsgIds::BuyAsset.id() => Ok(SubmsgIds::BuyAsset),
            x if x == SubmsgIds::BurnAsset.id() => Ok(SubmsgIds::BurnAsset),
            x if x == SubmsgIds::WithdrawCollateralOnReply.id() => Ok(SubmsgIds::WithdrawCollateralOnReply),
            unknown => Err(StdError::generic_err(format!(
                "unknown reply message id: {}",
                unknown
            ))),
        }
    }
}

impl SubmsgIds {
    pub const fn id(&self) -> u64 {
        match self {
            SubmsgIds::OpenCDP => 0,
            SubmsgIds::DepositToCDP => 1,
            SubmsgIds::MintAssetWithAimCollateralRatio => 2,
            SubmsgIds::SellAsset => 3,
            SubmsgIds::DepositStableOnReply => 4,
            SubmsgIds::Exit => 5,
            SubmsgIds::RedeemStable => 6,
            SubmsgIds::BuyAsset => 7,
            SubmsgIds::BurnAsset => 8,
            SubmsgIds::WithdrawCollateralOnReply => 9,
        }
    }
}

#[inline]
fn concat(namespace: &[u8], key: &[u8]) -> Vec<u8> {
    let mut k = namespace.to_vec();
    k.extend_from_slice(key);
    k
}