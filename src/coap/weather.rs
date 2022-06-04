use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use async_coap::prelude::*;
use rust_decimal::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::coap::Basic;
use crate::coap::CborParser;
use crate::coap::ServiceDiscovery;

pub struct Weather {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
}

impl Weather {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) -> Self {
        Weather {
            local_endpoint
        }
    }

    pub async fn get_temperature(&self) -> Result<Decimal, String> {
        let addr = ServiceDiscovery::new(self.local_endpoint.clone()).service_discovery("bac", None).await?;
        let mut curr_val = Decimal::new(0,0);
        Basic::new(self.local_endpoint.clone()).send_getter(&addr, "bac/temp", |context| {
            let data : BTreeMap<String, ciborium::value::Value> = 
                ciborium::de::from_reader(context?.message().payload())
                    .map_err(|_| async_coap::Error::ParseFailure)?;
            curr_val = CborParser::to_decimal(
                data.get(&"e".to_string()).unwrap())  // TODO: Handle None
                    .map_err(|_| async_coap::Error::ParseFailure)?;
            Ok(ResponseStatus::Done(()))
        }).await?;

        Ok(curr_val)
    }
}
