use serde::{
    de::DeserializeOwned, Serialize
};

pub trait TokenAmount:
    Serialize + DeserializeOwned + From<Self::ApiTokenAmount> + Into<Self::ApiTokenAmount> + PartialEq
{
    type ApiTokenAmount;
}

#[cfg(test)]
pub mod test {
    use super::*;
    use serde::Deserialize;
    use serde_with::serde_as;


    #[serde_as]
    #[derive(PartialEq, Serialize, Deserialize)]
    pub struct MockTokenAmount {
        pub token: String,
        pub amount: u64,
    }

    impl TokenAmount for MockTokenAmount {
        type ApiTokenAmount = MockTokenAmount;
    }
}
