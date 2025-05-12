use {
    crate::{
        AccessLevel,
        Routable,
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
#[serde(rename_all = "snake_case")]
pub enum PermissionFeature {
    /// The feature for which the permission is for.
    CancelQuote,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    /// The permission is enabled.
    Enabled,
    /// The permission is disabled.
    Disabled,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct CreatePermission {
    /// The id of the profile to create permission for.
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    pub profile_id: ProfileId,
    /// The feature which the permission is for.
    #[schema(example = "cancel_quote", value_type = PermissionFeature)]
    pub feature:    PermissionFeature,
    /// The state of the permission.
    #[schema(example = "disabled", value_type = PermissionState)]
    pub state:      PermissionState,
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
    #[strum(serialize = "permissions")]
    PostPermission,
}

impl Routable for Route {
    fn properties(&self) -> crate::RouteProperties {
        let full_path = format!(
            "{}{}{}",
            crate::Route::V1.as_ref(),
            crate::Route::Profile.as_ref(),
            self.as_ref()
        )
        .trim_end_matches("/")
        .to_string();
        match self {
            Route::PostProfile => crate::RouteProperties {
                access_level: AccessLevel::Admin,
                method: http::Method::POST,
                full_path,
            },
            Route::GetProfile => crate::RouteProperties {
                access_level: AccessLevel::Admin,
                method: http::Method::GET,
                full_path,
            },
            Route::PostProfileAccessToken => crate::RouteProperties {
                access_level: AccessLevel::Admin,
                method: http::Method::POST,
                full_path,
            },
            Route::DeleteProfileAccessToken => crate::RouteProperties {
                access_level: AccessLevel::LoggedIn,
                method: http::Method::DELETE,
                full_path,
            },
            Route::PostPermission => crate::RouteProperties {
                access_level: AccessLevel::Admin,
                method: http::Method::POST,
                full_path,
            },
        }
    }
}
