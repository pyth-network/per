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
    /// The name of the profile to create
    #[schema(example = "John Doe")]
    pub name:  String,
    /// The email of the profile to create
    #[schema(example = "example@example.com", value_type = String)]
    pub email: String,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct Profile {
    /// The id of the profile
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    id:    ProfileId,
    /// The name of the profile
    #[schema(example = "John Doe")]
    name:  String,
    /// The email of the profile
    #[schema(example = "example@example.com", value_type = String)]
    email: EmailAddress,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct CreateAccessToken {
    /// The id of the profile to create token for
    #[schema(example = "obo3ee3e-58cc-4372-a567-0e02b2c3d479", value_type = String)]
    profile_id: ProfileId,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, ToResponse)]
pub struct AccessToken {
    /// The token for later use
    #[schema(example = "_q9zUYP-tQg8F7kQi2Rfl5c6sSy7xcc2yWh2H-nI-iI", value_type = String)]
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
    State(store): State<Arc<Store>>,
    Json(params): Json<CreateProfile>,
) -> Result<Json<Profile>, RestError> {
    let profile = store.create_profile(params.clone()).await?;
    Ok(Json(Profile {
        id:    profile.id,
        name:  profile.name,
        email: profile.email.0,
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
    Json(params): Json<CreateAccessToken>,
) -> Result<Json<AccessToken>, RestError> {
    let (access_token, _) = store.get_or_create_access_token(params.profile_id).await?;
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
responses(
(status = 200, description = "The token successfully revoked"),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn delete_profile_access_token(
    auth: Auth,
    State(store): State<Arc<Store>>,
) -> Result<(), RestError> {
    match auth {
        Auth::Authorized(token, _) => store.revoke_access_token(&token).await,
        _ => Ok(()),
    }
}
