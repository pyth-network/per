use serde::{
    de::DeserializeOwned,
    Serialize,
};

pub trait TokenAmount:
    Serialize + DeserializeOwned + From<Self::ApiTokenAmount> + Into<Self::ApiTokenAmount> + PartialEq
{
    type ApiTokenAmount;
}

#[cfg(test)]
pub mod test {
    use {
        super::*,
        mockall::mock,
        serde::{
            Deserialize,
            Deserializer,
            Serializer,
        },
    };

    mock! {
        #[derive(Serialize, Deserialize)]
        pub TokenAmount {
        }

        impl TokenAmount for TokenAmount {
            type ApiTokenAmount = MockTokenAmount;
        }

        impl PartialEq for TokenAmount {
            fn eq(&self, other: &Self) -> bool;
        }
    }

    impl Serialize for MockTokenAmount {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_unit()
        }
    }

    impl<'de> Deserialize<'de> for MockTokenAmount {
        fn deserialize<D>(_: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            Ok(Self::default())
        }
    }
}
