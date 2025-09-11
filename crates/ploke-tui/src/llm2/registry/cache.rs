use futures::stream::{self, StreamExt, TryStreamExt};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::llm2::{
    request::{
        endpoint::{Endpoint, EndpointData},
        models,
    }, router_only::{HasEndpoint, HasModelId, HasModels, Router}, EndpointsResponse, ModelId, ModelKey
};

/// Cache for items from OpenRouter's `{author}/{slug}:{variant}/endpoints` API
// - Later will add other endpoints and make a generic set of items that should be included across
// different routers/providers, maybe with extra fields per-router
// - Serialize/Deserialize for persistence via local saved `.json`
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct EndpointCache {
    /// Keyed by {author}/{slug}, differentiate on optional `:{variant}` by checking `Endpoint`
    /// field
    // Endpoint non-trivial clone size, store in `Arc` as it is essentially immutable, replacements
    // will remove old values (can use e.g. `take()`) on update
    cache: HashMap<ModelKey, Vec<Arc<Endpoint>>>,
    /// Duration to live, default to 30 mins
    ttl: Duration,
    /// unix timestamp of last update
    last_update: u32,
}

impl EndpointCache {
    /// Create a new endpoint cache with default TTL (30 minutes)
    pub(crate) fn new() -> Self {
        Self {
            cache: HashMap::new(),
            ttl: Duration::from_secs(30 * 60), // 30 minutes
            last_update: 0,
        }
    }

    /// Create a new endpoint cache with custom TTL
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
            last_update: 0,
        }
    }
}
/// Cache for items from OpenRouter's `/models` endpoint
// - Later will add other endpoints and make a generic set of items that should be included across
// different routers/providers, maybe with extra fields per-router
// - Serialize/Deserialize for persistence via local saved `.json`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct ModelCache {
    /// Keyed by {author}/{slug}, differentiate on optional `:{variant}` by checking
    /// `id` of `models::ResponseItem`
    cache: HashMap<ModelKey, Arc<models::ResponseItem>>,
    /// Duration to live, default to 12 hours
    ttl: Duration,
    /// unix timestamp of last update
    last_update: u32,
}

impl ModelCache {
    /// Create a new model cache with default TTL (12 hours)
    pub(crate) fn new() -> Self {
        Self {
            cache: HashMap::new(),
            ttl: Duration::from_secs(12 * 60 * 60), // 12 hours
            last_update: 0,
        }
    }

    /// Create a new model cache with custom TTL
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
            last_update: 0,
        }
    }
}

pub(crate) trait ApiCache {
    // TODO: Convenience methods for updating generically across Router
    type Item;

    fn get(&self, key: &ModelKey) -> Option<&Self::Item>;
    fn update(&mut self, data: HashMap<ModelKey, Self::Item>);
    fn cache(&self) -> &HashMap<ModelKey, Self::Item>;
    fn cache_mut(&mut self) -> &mut HashMap<ModelKey, Self::Item>;

    fn last_update(&self) -> u32;
    fn ttl(&self) -> Duration;
    fn set_last_update(&mut self, timestamp: u32);

    fn is_stale(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        now > self.last_update() + self.ttl().as_secs() as u32
    }

    fn touch(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        self.set_last_update(now);
    }

    async fn extend_from_router<T: Router + HasModels + HasEndpoint>(
        &mut self,
        client: &reqwest::Client,
        max_in_flight: usize,
    ) -> color_eyre::Result<()>;
}

impl ApiCache for EndpointCache {
    type Item = Vec<Arc<Endpoint>>;

    fn get(&self, key: &ModelKey) -> Option<&Self::Item> {
        self.cache.get(key)
    }

    fn update(&mut self, data: HashMap<ModelKey, Self::Item>) {
        self.cache = data;
        self.touch();
    }

    fn cache(&self) -> &HashMap<ModelKey, Self::Item> {
        &self.cache
    }

    fn cache_mut(&mut self) -> &mut HashMap<ModelKey, Self::Item> {
        &mut self.cache
    }

    fn last_update(&self) -> u32 {
        self.last_update
    }

    fn ttl(&self) -> Duration {
        self.ttl
    }

    fn set_last_update(&mut self, timestamp: u32) {
        self.last_update = timestamp;
    }

    async fn extend_from_router<T>(
        &mut self,
        client: &reqwest::Client,
        max_in_flight: usize,
    ) -> color_eyre::Result<()>
    where
        T: Router + HasModels + HasEndpoint,
    {
        // Fetch models and map them to the router-specific ModelId
        let models = T::fetch_models_iter(client).await?;
        let ids = models
            .into_iter()
            .map(|m| m.model_id())
            .map(T::RouterModelId::from);

        // Bounded concurrency over endpoint fetches; cancel-safe if caller drops the future
        let responses: Vec<T::EpResponse> = stream::iter(ids)
            .map(|id| async move { T::fetch_model_endpoints(client, id).await })
            .buffer_unordered(max_in_flight)
            .try_collect()
            .await?;

        // Convert to a common EndpointsResponse and flatten the endpoints
        for resp in responses {
            let er: EndpointsResponse = resp.into();
            let EndpointData {
                id,
                name,
                created,
                description,
                architecture,
                endpoints,
            } = er.data;
            self.cache_mut().entry(id.key).and_modify(|entry| {
                entry.extend(endpoints.into_iter().map(|ep| Arc::new(ep)));
            });
        }
        self.touch();
        Ok(())
    }
}

impl ApiCache for ModelCache {
    type Item = Arc<models::ResponseItem>;

    fn get(&self, key: &ModelKey) -> Option<&Self::Item> {
        self.cache.get(key)
    }

    fn update(&mut self, data: HashMap<ModelKey, Self::Item>) {
        self.cache = data;
        self.touch();
    }

    fn cache(&self) -> &HashMap<ModelKey, Self::Item> {
        &self.cache
    }

    fn cache_mut(&mut self) -> &mut HashMap<ModelKey, Self::Item> {
        &mut self.cache
    }

    fn last_update(&self) -> u32 {
        self.last_update
    }

    fn ttl(&self) -> Duration {
        self.ttl
    }

    fn set_last_update(&mut self, timestamp: u32) {
        self.last_update = timestamp;
    }

    async fn extend_from_router<T>(
        &mut self,
        client: &reqwest::Client,
        max_in_flight: usize,
    ) -> color_eyre::Result<()>
    where
        T: Router + HasModels + HasEndpoint,
    {
        let models = T::fetch_models_iter(client).await?;
        let ids = models
            .into_iter()
            .map(Into::<models::ResponseItem>::into)
            .map(|m| (m.id.key.clone(), Arc::new(m)));
        self.cache_mut().extend(ids);
        self.touch();
        Ok(())
    }
}
