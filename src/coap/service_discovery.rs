use home_mng::Coap;
use std::net::SocketAddr;

pub struct ServiceDiscovery<'a> {
    coap: &'a Coap,
}

impl <'a> ServiceDiscovery<'a> {
    pub fn new(coap: &'a Coap) -> Self {
        Self {
            coap,
        }
    }

    pub async fn discover_single(&self, rsrc: &str) -> Result<SocketAddr, String> {
        Ok(self.coap.service_discovery_single(rsrc, None).await
            .map_err(|e| e.to_string())?
            .ok_or(format!("Could not discover address of {}", rsrc))?
            .0)
    }
}
