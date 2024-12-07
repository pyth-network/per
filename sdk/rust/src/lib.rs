use {
    express_relay_api_types::{
        opportunity::{
            Opportunity,
            Route,
        },
        AccessLevel,
        RouteTrait,
    },
    reqwest::Response,
    url::Url,
};

pub struct Client {
    pub host:    Url,
    pub api_key: Option<String>,

    client: reqwest::Client,
}

#[derive(Debug)]
pub enum ClientError {
    InvalidHost(String),
    RequestFailed(reqwest::Error),
    DecodeResponseFailed(reqwest::Error),
    AuthenticationRequired,
}

impl Client {
    async fn send(&self, route: impl RouteTrait) -> Result<Response, ClientError> {
        if self.api_key.is_none() && route.access_level() != AccessLevel::Public {
            return Err(ClientError::AuthenticationRequired);
        }

        let url = self
            .host
            .join(route.full_path().as_str())
            .map_err(|e| ClientError::InvalidHost(e.to_string()))?;
        let request = self.client.request(route.method(), url);
        request.send().await.map_err(ClientError::RequestFailed)
    }

    pub fn try_new(host: &str, api_key: Option<&str>) -> Result<Self, ClientError> {
        Ok(Self {
            host:    Url::parse(host).map_err(|e| ClientError::InvalidHost(e.to_string()))?,
            api_key: api_key.map(|s| s.to_string()),
            client:  reqwest::Client::new(),
        })
    }

    pub async fn get_opportunities(&self) -> Result<Vec<Opportunity>, ClientError> {
        let response = self.send(Route::GetOpportunities).await?;
        response
            .json()
            .await
            .map_err(ClientError::DecodeResponseFailed)
    }
}
