use {
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

/// Similar to OpportunityParams, but with the opportunity id included.
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

/// Create a new profile.
///
/// You will a profile object in response.
#[utoipa::path(post, path = "/v1/profiles", request_body = CreateProfile, responses(
(status = 200, description = "The created profile", body = Profile),
(status = 400, response = ErrorBodyResponse),
),)]
pub async fn post_profile(
    State(store): State<Arc<Store>>,
    Json(versioned_params): Json<CreateProfile>,
) -> Result<Json<Profile>, RestError> {
    let profile = store.create_profile(versioned_params.clone()).await?;
    Ok(Json(Profile {
        id:    profile.id,
        name:  profile.name,
        email: profile.email,
    }))
}
