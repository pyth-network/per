use {
    super::Auth,
    crate::{
        api::{
            ErrorBodyResponse,
            RestError,
        },
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
    uuid::Uuid,
};

pub type ProfileId = Uuid;

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
#[utoipa::path(post, path = "/v1/profiles", request_body = CreateProfile, responses(
(status = 200, description = "The created profile", body = Profile),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn post_profile(
    auth: Auth,
    State(store): State<Arc<Store>>,
    Json(versioned_params): Json<CreateProfile>,
) -> Result<Json<Profile>, RestError> {
    println!("auth: {:?}", auth.is_admin);
    let profile = store.create_profile(versioned_params.clone()).await?;
    Ok(Json(Profile {
        id:    profile.id,
        name:  profile.name,
        email: profile.email,
    }))
}

/// Create a new profile access token if no valid token exists.
///
/// Returns the created access token object.
#[utoipa::path(get, path = "/v1/profiles/access_tokens", request_body = CreateAccessToken, responses(
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
