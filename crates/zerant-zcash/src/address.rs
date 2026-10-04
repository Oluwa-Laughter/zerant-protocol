use std::convert::Infallible;

use serde::Serialize;
use zcash_address::{ConversionError, Converter, ZcashAddress, unified};
use zcash_protocol::{PoolType, consensus::NetworkType};
use zerant_core::{Error, Result};

pub const MAX_ADDRESS_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressSummary {
    pub canonical: String,
    pub network: &'static str,
    pub kind: &'static str,
    pub unified: bool,
    pub can_receive_memo: bool,
    pub transparent_only: bool,
    pub supports_orchard: bool,
    pub supports_sapling: bool,
    pub supports_transparent: bool,
}

#[derive(Debug, Clone, Copy)]
struct AddressClass {
    network: NetworkType,
    kind: &'static str,
}

struct Classifier;

impl Converter<AddressClass> for Classifier {
    type Error = Infallible;

    fn convert_sprout(
        &self,
        net: NetworkType,
        _data: [u8; 64],
    ) -> std::result::Result<AddressClass, ConversionError<Self::Error>> {
        Ok(AddressClass {
            network: net,
            kind: "sprout",
        })
    }

    fn convert_sapling(
        &self,
        net: NetworkType,
        _data: [u8; 43],
    ) -> std::result::Result<AddressClass, ConversionError<Self::Error>> {
        Ok(AddressClass {
            network: net,
            kind: "sapling",
        })
    }

    fn convert_unified(
        &self,
        net: NetworkType,
        _data: unified::Address,
    ) -> std::result::Result<AddressClass, ConversionError<Self::Error>> {
        Ok(AddressClass {
            network: net,
            kind: "unified",
        })
    }

    fn convert_transparent_p2pkh(
        &self,
        net: NetworkType,
        _data: [u8; 20],
    ) -> std::result::Result<AddressClass, ConversionError<Self::Error>> {
        Ok(AddressClass {
            network: net,
            kind: "transparent_p2pkh",
        })
    }

    fn convert_transparent_p2sh(
        &self,
        net: NetworkType,
        _data: [u8; 20],
    ) -> std::result::Result<AddressClass, ConversionError<Self::Error>> {
        Ok(AddressClass {
            network: net,
            kind: "transparent_p2sh",
        })
    }

    fn convert_tex(
        &self,
        net: NetworkType,
        _data: [u8; 20],
    ) -> std::result::Result<AddressClass, ConversionError<Self::Error>> {
        Ok(AddressClass {
            network: net,
            kind: "tex",
        })
    }
}

fn network_name(network: NetworkType) -> &'static str {
    match network {
        NetworkType::Main => "mainnet",
        NetworkType::Test => "testnet",
        NetworkType::Regtest => "regtest",
    }
}

pub fn inspect_address(encoded: &str) -> Result<AddressSummary> {
    if encoded.is_empty() {
        return Err(Error::Encoding);
    }
    if encoded.len() > MAX_ADDRESS_BYTES {
        return Err(Error::Size);
    }

    let address = ZcashAddress::try_from_encoded(encoded).map_err(|_| Error::Encoding)?;
    let canonical = address.encode();
    let class = address
        .clone()
        .convert_with(Classifier)
        .map_err(|_| Error::Encoding)?;

    Ok(AddressSummary {
        canonical,
        network: network_name(class.network),
        kind: class.kind,
        unified: class.kind == "unified",
        can_receive_memo: address.can_receive_memo(),
        transparent_only: address.is_transparent_only(),
        supports_orchard: address.can_receive_as(PoolType::ORCHARD),
        supports_sapling: address.can_receive_as(PoolType::SAPLING),
        supports_transparent: address.can_receive_as(PoolType::TRANSPARENT),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTNET_TADDR: &str = "tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU";
    const SAPLING_MAINNET: &str =
        "zs1z7rejlpsa98s2rrrfkwmaxu53e4ue0ulcrw0h4x5g8jl04tak0d3mm47vdtahatqrlkngh9slya";

    #[test]
    fn classifies_transparent_testnet_address() {
        let summary = inspect_address(TESTNET_TADDR).unwrap();
        assert_eq!(summary.network, "testnet");
        assert_eq!(summary.kind, "transparent_p2pkh");
        assert!(summary.transparent_only);
        assert!(summary.supports_transparent);
        assert!(!summary.supports_sapling);
        assert!(!summary.supports_orchard);
        assert!(!summary.can_receive_memo);
    }

    #[test]
    fn classifies_sapling_mainnet_address() {
        let summary = inspect_address(SAPLING_MAINNET).unwrap();
        assert_eq!(summary.network, "mainnet");
        assert_eq!(summary.kind, "sapling");
        assert!(summary.supports_sapling);
        assert!(summary.can_receive_memo);
        assert!(!summary.transparent_only);
    }

    #[test]
    fn rejects_non_zcash_and_oversized_inputs() {
        assert!(inspect_address("not-a-zcash-address").is_err());
        assert_eq!(
            inspect_address(&"x".repeat(MAX_ADDRESS_BYTES + 1)),
            Err(Error::Size)
        );
    }
}
