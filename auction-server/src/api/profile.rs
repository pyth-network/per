use {
    super::Auth,
    crate::{
        api::{
            ErrorBodyResponse,
            RestError,
        },
        models::ProfileId,
        state::Store,
    },
    axum::{
        extract::State,
        Json,
    },
    email_address::EmailAddress,
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::{
        ToResponse,
        ToSchema,
    },
};

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct CreateProfile {
    /// name of the profile to create
    #[schema(example = "John Doe", value_type = String)]
    pub name:  String,
    /// email of the profile to create
    #[schema(example = "example@example.com", value_type = String)]
    pub email: String,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct Profile {
    /// name of the profile to create
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    id:    ProfileId,
    /// name of the profile to create
    #[schema(example = "John Doe", value_type = String)]
    name:  String,
    /// name of the profile to create
    #[schema(example = "example@example.com", value_type = String)]
    email: EmailAddress,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct CreateAccessToken {
    /// The token for later use
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    profile_id: ProfileId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct AccessToken {
    /// The token for later use
    #[schema(example = "accesstoken", value_type = String)]
    token: String,
}

/// Create a new profile.
///
/// Returns the created profile object.
#[utoipa::path(post, path = "/v1/profiles",
security(
    ("bearerAuth" = []),
),request_body = CreateProfile, responses(
(status = 200, description = "The created profile", body = Profile),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn post_profile(
    _auth: Auth,
    State(store): State<Arc<Store>>,
    Json(versioned_params): Json<CreateProfile>,
) -> Result<Json<Profile>, RestError> {
    let profile = store.create_profile(versioned_params.clone()).await?;
    Ok(Json(Profile {
        id:    profile.id,
        name:  profile.name,
        email: profile.email.value,
    }))
}

/// Create a new profile access token if no valid token exists.
///
/// Returns the created access token object.
#[utoipa::path(post, path = "/v1/profiles/access_tokens",
security(
    ("bearerAuth" = []),
),request_body = CreateAccessToken, responses(
(status = 200, description = "The access token for the profile", body = AccessToken),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn post_profile_access_token(
    State(store): State<Arc<Store>>,
    Json(versioned_params): Json<CreateAccessToken>,
) -> Result<Json<AccessToken>, RestError> {
    let (access_token, _) = store
        .get_or_create_access_token(versioned_params.profile_id)
        .await?;
    Ok(Json(AccessToken {
        token: access_token.token,
    }))
}

/// Revoke the authenticated profile access token.
///
/// Returns empty response.
#[utoipa::path(delete, path = "/v1/profiles/access_tokens",
security(
    ("bearerAuth" = []),
),
request_body = CreateAccessToken, responses(
(status = 200, description = "The token successfully revoked"),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn delete_profile_access_token(
    auth: Auth,
    State(store): State<Arc<Store>>,
) -> Result<(), RestError> {
    match auth.token_id {
        Some(token_id) => store.revoke_access_token(token_id).await,
        None => Ok(()),
    }
}
