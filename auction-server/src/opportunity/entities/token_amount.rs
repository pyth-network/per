use {
    crate::opportunity::api,
    serde::{
        de::DeserializeOwned,
        Serialize,
    },
};

pub trait TokenAmount:
    Serialize + DeserializeOwned + From<Self::ApiTokenAmount> + Into<Self::ApiTokenAmount>
{
    type ApiTokenAmount;
}
