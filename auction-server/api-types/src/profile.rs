use {
    crate::{
        AccessLevel,
        RouteTrait,
    },
    email_address::EmailAddress,
    serde::{
        Deserialize,
        Serialize,
    },
    strum::AsRefStr,
    utoipa::{
        IntoParams,
        ToResponse,
        ToSchema,
    },
    uuid::Uuid,
};

pub type ProfileId = Uuid;

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ProfileRole {
    Searcher,
    Protocol,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse, Debug)]
pub struct CreateProfile {
    /// The name of the profile to create.
    #[schema(example = "John Doe")]
    pub name:  String,
    /// The email of the profile to create.
    #[schema(example = "example@example.com", value_type = String)]
    pub email: String,
    /// The role of the profile to create.
    pub role:  ProfileRole,
}

#[derive(Serialize, Deserialize, Clone, Debug, IntoParams)]
pub struct GetProfile {
    /// The email of the profile to fetch.
    #[param(example = "example@example.com", value_type = String)]
    pub email: String,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct Profile {
    /// The id of the profile.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub id:    ProfileId,
    /// The name of the profile.
    #[schema(example = "John Doe")]
    pub name:  String,
    /// The email of the profile.
    #[schema(example = "example@example.com", value_type = String)]
    pub email: EmailAddress,
    /// The role of the profile.
    pub role:  ProfileRole,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct CreateAccessToken {
    /// The id of the profile to create token for.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub profile_id: ProfileId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct AccessToken {
    /// The token for later use.
    #[schema(example = "_q9zUYP-tQg8F7kQi2Rfl5c6sSy7xcc2yWh2H-nI-iI", value_type = String)]
    pub token: String,
}

#[derive(AsRefStr, Clone)]
#[strum(prefix = "/")]
pub enum Route {
    #[strum(serialize = "")]
    PostProfile,
    #[strum(serialize = "")]
    GetProfile,
    #[strum(serialize = "access_tokens")]
    PostProfileAccessToken,
    #[strum(serialize = "access_tokens")]
    DeleteProfileAccessToken,
}

impl RouteTrait for Route {
    fn get_access_level(&self) -> AccessLevel {
        match self {
            Route::PostProfile => AccessLevel::Admin,
            Route::GetProfile => AccessLevel::Admin,
            Route::PostProfileAccessToken => AccessLevel::Admin,
            Route::DeleteProfileAccessToken => AccessLevel::LoggedIn,
        }
    }

    fn method(&self) -> http::Method {
        match self {
            Route::PostProfile => http::Method::POST,
            Route::GetProfile => http::Method::GET,
            Route::PostProfileAccessToken => http::Method::POST,
            Route::DeleteProfileAccessToken => http::Method::DELETE,
        }
    }
}
