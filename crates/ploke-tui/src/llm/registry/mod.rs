use futures::stream::{self, StreamExt, TryStreamExt};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    EndpointsResponse, ModelId, ModelKey,
    request::{
        endpoint::{Endpoint, EndpointData},
        models,
    },
    router_only::{HasEndpoint, HasModelId, HasModels, Router},
};

pub(crate) mod cache;
pub(crate) mod user_prefs;


