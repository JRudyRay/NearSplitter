/*!
 * Minimal FT Mock Contract for Integration Tests
 * 
 * This is a minimal implementation that provides just enough functionality
 * to test NearSplitter's ft_metadata fetching and caching.
 */

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{near_bindgen, AccountId};
use serde::{Deserialize, Serialize};

/// NEP-148 Fungible Token Metadata
#[derive(Clone, Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
#[borsh(crate = "near_sdk::borsh")]
pub struct FungibleTokenMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<String>,
    pub decimals: u8,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
#[borsh(crate = "near_sdk::borsh")]
pub struct FtMock {
    metadata: Option<FungibleTokenMetadata>,
}

#[near_bindgen]
impl FtMock {
    /// Initialize with custom metadata
    #[init]
    pub fn new(name: String, symbol: String, decimals: u8) -> Self {
        Self {
            metadata: Some(FungibleTokenMetadata {
                spec: "ft-1.0.0".to_string(),
                name,
                symbol,
                icon: None,
                reference: None,
                reference_hash: None,
                decimals,
            }),
        }
    }

    /// Initialize with default test metadata
    #[init]
    pub fn new_default() -> Self {
        Self {
            metadata: Some(FungibleTokenMetadata {
                spec: "ft-1.0.0".to_string(),
                name: "Mock Token".to_string(),
                symbol: "MOCK".to_string(),
                icon: None,
                reference: None,
                reference_hash: None,
                decimals: 18,
            }),
        }
    }

    /// NEP-148: Return fungible token metadata
    pub fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.clone().unwrap_or(FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: "Mock Token".to_string(),
            symbol: "MOCK".to_string(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 18,
        })
    }

    /// Minimal ft_transfer_call implementation for testing
    /// Just accepts tokens and returns "0" (refund nothing)
    pub fn ft_on_transfer(
        &mut self,
        _sender_id: AccountId,
        _amount: String,
        _msg: String,
    ) -> String {
        // Return "0" to indicate all tokens are kept
        "0".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ft_metadata_default() {
        let contract = FtMock::new_default();
        let metadata = contract.ft_metadata();
        assert_eq!(metadata.name, "Mock Token");
        assert_eq!(metadata.symbol, "MOCK");
        assert_eq!(metadata.decimals, 18);
    }

    #[test]
    fn test_ft_metadata_custom() {
        let contract = FtMock::new("USD Coin".to_string(), "USDC".to_string(), 6);
        let metadata = contract.ft_metadata();
        assert_eq!(metadata.name, "USD Coin");
        assert_eq!(metadata.symbol, "USDC");
        assert_eq!(metadata.decimals, 6);
    }
}
