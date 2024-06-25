pub mod u256 {
    use {
        ethers::types::U256,
        serde::{
            de::Error,
            Deserialize,
            Deserializer,
            Serializer,
        },
    };

    pub fn serialize<S>(b: &U256, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(b.to_string().as_str())
    }

    pub fn deserialize<'de, D>(d: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(d)?;
        U256::from_dec_str(s.as_str()).map_err(|err| D::Error::custom(err.to_string()))
    }
}
pub mod signature {
    use {
        ethers::types::Signature,
        serde::{
            de::Error,
            Deserialize,
            Deserializer,
            Serializer,
        },
        std::str::FromStr,
    };

    pub fn serialize<S>(b: &Signature, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(b.to_string().as_str())
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Signature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(d)?;
        Signature::from_str(s.as_str()).map_err(|err| D::Error::custom(err.to_string()))
    }
}

pub mod nullable_datetime {
    use {
        serde::{
            de::Error,
            Deserialize,
            Deserializer,
            Serializer,
        },
        time::{
            format_description::well_known::Rfc3339,
            OffsetDateTime,
        },
    };

    pub fn serialize<S>(b: &Option<OffsetDateTime>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match b {
            Some(b) => s.serialize_str(b.to_string().as_str()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<OffsetDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Deserialize::deserialize(d)?;
        match s {
            Some(s) => OffsetDateTime::parse(s.as_str(), &Rfc3339)
                .map(Some)
                .map_err(|err| D::Error::custom(err.to_string())),
            None => Ok(None),
        }
    }
}

pub mod nullable_u256 {
    use {
        ethers::types::U256,
        serde::{
            de::Error,
            Deserialize,
            Deserializer,
            Serializer,
        },
    };

    pub fn serialize<S>(b: &Option<U256>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match b {
            Some(b) => s.serialize_str(b.to_string().as_str()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<U256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Deserialize::deserialize(d)?;
        match s {
            Some(s) => U256::from_dec_str(s.as_str())
                .map(Some)
                .map_err(|err| D::Error::custom(err.to_string())),
            None => Ok(None),
        }
    }
}
