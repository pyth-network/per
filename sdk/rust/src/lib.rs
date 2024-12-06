use url::Url;

pub struct Client {
    pub host:    Url,
    pub api_key: Option<String>,
}

#[derive(Debug)]
pub enum ClientError {
    InvalidHost(String),
}

impl Client {
    pub fn try_new(host: &str, api_key: Option<&str>) -> Result<Self, ClientError> {
        Ok(Self {
            host:    Url::parse(host).map_err(|e| ClientError::InvalidHost(e.to_string()))?,
            api_key: api_key.map(|s| s.to_string()),
        })
    }

    pub fn test(&self) {
        println!("Testing client with host: {:?}", self.host);
    }
}
