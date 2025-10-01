use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{near_bindgen, PanicOnDefault};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct TestContract {
    value: u64,
}

#[near_bindgen]
impl TestContract {
    #[init]
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn get_value(&self) -> u64 {
        self.value
    }
}
