use serde::{
    de::DeserializeOwned,
    Serialize,
};

pub trait TokenAmount: Serialize + DeserializeOwned {}
