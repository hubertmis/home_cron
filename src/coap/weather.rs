use home_mng::Coap;
use rust_decimal::prelude::*;

use crate::coap::{CborParser, ServiceDiscovery};

pub struct Weather {
    coap: Coap,
}

impl Weather {
    pub fn new() -> Self {
        Weather {
            coap: Coap::new(),
        }
    }

    pub async fn get_temperature(&self) -> Result<Decimal, String> {
        let addr = ServiceDiscovery::new(&self.coap).discover_single("bac").await?;
        let mut temps = self.coap.get(&addr, "bac/temp", None).await
            .map_err(|e| e.to_string())?
            .ok_or("No temperature content returned by bac/temp")?
            .as_cbor_map().ok_or("Unexpected temperature content returned by bac/temp")?
            .iter()
            .filter(|e| e.0.as_text()
                    .is_some_and(|t| t == "e"))
            .map(|e| CborParser::to_decimal(&e.1))
            .collect::<Vec<_>>();

        if temps.len() != 1 {
            return Err("Unexpected structure returned by bac/temp".to_string());
        }
        temps.remove(0)
    }
}
