use crate::{
    EndpointsResponse,
    llm2::{
        error::LlmError,
        request::endpoint::Endpoint,
        router_only::{HasEndpoint, HasModelId, HasModels, Router},
    },
};
use futures::stream::{self, StreamExt, TryStreamExt};

// in progress/conceptual
enum Comman {
    Update(update::Cmd),
    Search(search::Cmd),
    Error(LlmError),
}

// in progress/conceptual
mod update {
    pub(super) struct Cmd;
}

// in progress/conceptual
mod search {
    pub(super) struct Cmd;
}

pub(crate) async fn update_endpoints<T>(
    client: &reqwest::Client,
    _router: T,
) -> color_eyre::Result<Vec<Endpoint>>
where
    T: Router + HasModels + HasEndpoint,
{
    update_endpoints_bounded::<T>(client, 8).await
}

pub(crate) async fn update_endpoints_bounded<T>(
    client: &reqwest::Client,
    max_in_flight: usize,
) -> color_eyre::Result<Vec<Endpoint>>
where
    T: Router + HasModels + HasEndpoint,
{
    // Fetch models and map them to the router-specific ModelId
    let models = T::fetch_models_iter(client).await?;
    let ids = models
        .into_iter()
        .map(|m| m.model_id())
        .map(T::ModelId::from);

    // Bounded concurrency over endpoint fetches; cancel-safe if caller drops the future
    let responses: Vec<T::EpResponse> = stream::iter(ids)
        .map(|id| async move { T::fetch_model_endpoints(client, id).await })
        .buffer_unordered(max_in_flight)
        .try_collect()
        .await?;

    // Convert to a common EndpointsResponse and flatten the endpoints
    let mut out = Vec::new();
    for resp in responses {
        let er: EndpointsResponse = resp.into();
        out.extend(er.data.endpoints);
    }
    Ok(out)
}
