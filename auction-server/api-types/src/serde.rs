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
            ser,
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
            Some(b) => {
                // This formatting is critical because the server expects incoming date-time values in the Rfc3339 format for deserialization.
                let formatted = b.format(&Rfc3339).map_err(ser::Error::custom)?;
                s.serialize_str(formatted.as_str())
            }
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

pub mod transaction_svm {
    use {
        base64::{
            engine::general_purpose::STANDARD,
            Engine as _,
        },
        serde::{
            de::Error as _,
            ser::Error,
            Deserialize,
            Deserializer,
            Serializer,
        },
        solana_sdk::transaction::VersionedTransaction,
    };

    pub fn serialize<S>(t: &VersionedTransaction, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let serialized = bincode::serialize(t).map_err(|e| S::Error::custom(e.to_string()))?;
        let base64_encoded = STANDARD.encode(serialized);
        s.serialize_str(base64_encoded.as_str())
    }

    pub fn deserialize<'de, D>(d: D) -> Result<VersionedTransaction, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(d)?;
        let base64_decoded = STANDARD
            .decode(s)
            .map_err(|e| D::Error::custom(e.to_string()))?;
        let transaction: VersionedTransaction =
            bincode::deserialize(&base64_decoded).map_err(|e| D::Error::custom(e.to_string()))?;
        Ok(transaction)
    }
}

pub mod nullable_signature_svm {
    use {
        serde::{
            de::Error,
            Deserialize,
            Deserializer,
            Serializer,
        },
        solana_sdk::signature::Signature,
        std::str::FromStr,
    };

    pub fn serialize<S>(b: &Option<Signature>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match b {
            Some(b) => s.serialize_str(b.to_string().as_str()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Signature>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Deserialize::deserialize(d)?;
        match s {
            Some(s) => Signature::from_str(s.as_str())
                .map(Some)
                .map_err(|err| D::Error::custom(err.to_string())),
            None => Ok(None),
        }
    }
}
