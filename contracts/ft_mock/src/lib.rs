/*!
 * Minimal FT Mock Contract for Integration Tests
 * 
 * This is a minimal implementation that provides just enough functionality
 * to test NearSplitter's ft_metadata fetching and caching.
 */

use near_contract_standards::fungible_token::metadata::FungibleTokenMetadata;
use near_sdk::near;
use near_sdk::{AccountId, PanicOnDefault};

// Store metadata fields directly to avoid serialization conflicts with external types
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct FtMock {
    name: String,
    symbol: String,
    decimals: u8,
}

#[near]
impl FtMock {
    /// Initialize with custom metadata
    #[init]
    pub fn new(name: String, symbol: String, decimals: u8) -> Self {
        Self {
            name,
            symbol,
            decimals,
        }
    }

    /// Initialize with default test metadata
    #[init]
    pub fn new_default() -> Self {
        Self {
            name: "Mock Token".to_string(),
            symbol: "MOCK".to_string(),
            decimals: 18,
        }
    }

    /// NEP-148: Return fungible token metadata
    /// Constructs the metadata on demand from stored fields
    pub fn ft_metadata(&self) -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            spec: "ft-1.0.0".to_string(),
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: self.decimals,
        }
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
