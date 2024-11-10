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
        let addr = self.coap.service_discovery(Some(rsrc), None).await.map_err(|e| e.to_string())?;
        if addr.len() != 1 {
            return Err(format!("Unexpected number of discovered services: {} {}", addr.len(), rsrc));
        }

        Ok(addr[0].2)
    }
}
