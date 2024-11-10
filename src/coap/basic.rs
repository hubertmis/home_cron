use home_mng::Coap;

use crate::coap::{CborMap, ServiceDiscovery};

pub async fn set_actuator(rsrc: &str, payload: CborMap) -> Result<(), String> {
    let coap = Coap::new();
    let addr = ServiceDiscovery::new(&coap).discover_single(rsrc).await?;

    coap.set(&addr, rsrc, &payload.as_ciborium_map()).await
        .map_err(|e| e.to_string())
}
