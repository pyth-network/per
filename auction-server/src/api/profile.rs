use {
    super::Auth,
    crate::{
        api::{
            ErrorBodyResponse,
            RestError,
        },
        models,
        state::StoreNew,
    },
    axum::{
        extract::{
            Query,
            State,
        },
        http::StatusCode,
        response::IntoResponse,
        Json,
    },
    express_relay_api_types::profile::{
        AccessToken,
        CreateAccessToken,
        CreatePrivilege,
        CreateProfile,
        GetProfile,
        PrivilegeFeature,
        PrivilegeState,
        Profile,
        ProfileRole,
    },
    std::sync::Arc,
};

impl From<models::ProfileRole> for ProfileRole {
    fn from(role: models::ProfileRole) -> Self {
        match role {
            models::ProfileRole::Searcher => ProfileRole::Searcher,
            models::ProfileRole::Protocol => ProfileRole::Protocol,
        }
    }
}

impl From<ProfileRole> for models::ProfileRole {
    fn from(role: ProfileRole) -> Self {
        match role {
            ProfileRole::Searcher => models::ProfileRole::Searcher,
            ProfileRole::Protocol => models::ProfileRole::Protocol,
        }
    }
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
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<CreateProfile>,
) -> Result<Json<Profile>, RestError> {
    let profile = store.store.create_profile(params.clone()).await?;
    Ok(Json(Profile {
        id:    profile.id,
        name:  profile.name,
        email: profile.email.0,
        role:  profile.role.into(),
    }))
}

/// Get a profile by email.
///
/// Returns the created profile object.
#[utoipa::path(get, path = "/v1/profiles",
security(
("bearerAuth" = []),
), params(GetProfile), responses(
(status = 200, description = "The fetched profile with the matching email", body = Profile),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn get_profile(
    State(store): State<Arc<StoreNew>>,
    params: Query<GetProfile>,
) -> Result<Json<Profile>, RestError> {
    let email = params
        .email
        .clone()
        .try_into()
        .map_err(|_| RestError::BadParameters("Invalid email".to_string()))?;
    let profile = store
        .store
        .get_profile_by_email(email)
        .await?
        .map(|profile| Profile {
            id:    profile.id,
            name:  profile.name,
            email: profile.email.0,
            role:  profile.role.into(),
        });
    Ok(Json(profile.ok_or(RestError::ProfileNotFound)?))
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
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<CreateAccessToken>,
) -> Result<Json<AccessToken>, RestError> {
    let (access_token, _) = store
        .store
        .get_or_create_access_token(params.profile_id)
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
responses(
(status = 200, description = "The token successfully revoked"),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn delete_profile_access_token(
    auth: Auth,
    State(store): State<Arc<StoreNew>>,
) -> Result<(), RestError> {
    match auth {
        Auth::Authorized(token, _) => store.store.revoke_access_token(&token).await,
        _ => Ok(()),
    }
}

impl From<models::PrivilegeFeature> for PrivilegeFeature {
    fn from(feature: models::PrivilegeFeature) -> Self {
        match feature {
            models::PrivilegeFeature::CancelQuote => PrivilegeFeature::CancelQuote,
        }
    }
}

impl From<PrivilegeFeature> for models::PrivilegeFeature {
    fn from(feature: PrivilegeFeature) -> Self {
        match feature {
            PrivilegeFeature::CancelQuote => models::PrivilegeFeature::CancelQuote,
        }
    }
}

impl From<models::PrivilegeState> for PrivilegeState {
    fn from(state: models::PrivilegeState) -> Self {
        match state {
            models::PrivilegeState::Enabled => PrivilegeState::Enabled,
            models::PrivilegeState::Disabled => PrivilegeState::Disabled,
        }
    }
}

impl From<PrivilegeState> for models::PrivilegeState {
    fn from(state: PrivilegeState) -> Self {
        match state {
            PrivilegeState::Enabled => models::PrivilegeState::Enabled,
            PrivilegeState::Disabled => models::PrivilegeState::Disabled,
        }
    }
}

/// Create a privilege for a profile.
///
/// Returns empty response.
#[utoipa::path(post, path = "/v1/profiles/privileges",
security(
("bearerAuth" = []),
),request_body = CreatePrivilege, responses(
(status = 201, description = "The privilege successfully created"),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn post_privilege(
    State(store): State<Arc<StoreNew>>,
    Json(params): Json<CreatePrivilege>,
) -> Result<impl IntoResponse, RestError> {
    store.store.create_privilege(params).await?;
    Ok(StatusCode::CREATED)
}
