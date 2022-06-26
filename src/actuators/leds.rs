use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use chrono::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap;

pub struct Leds {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
}

impl Leds {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) -> Self {
        Self {
            local_endpoint,
        }
    }

    async fn get_action_list(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        
        let mut morning_action_list = Vec::new();
        morning_action_list.push(("bbl", (0, 0, 0, 255, 1000)));
        morning_action_list.push(("bwl", (0, 0, 0, 0, 0)));
        
        let mut evening_action_list = Vec::new();
        evening_action_list.push(("bbl", (0, 0, 0, 0, 1000)));
        evening_action_list.push(("bwl", (0, 0, 0, 5, 0)));

        let morning_endpoint = self.local_endpoint.clone();
        let evening_endpoint = self.local_endpoint.clone();

        actions.push(Action::new(
            CronProcessor::time_to_timestamp(NaiveTime::from_hms(9, 0, 0)),
            async move {
                let endpoint = &morning_endpoint.clone();
                CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::set_led(endpoint, r, v).await}, None).await
            }
        ));
        actions.push(Action::new(
            CronProcessor::time_to_timestamp(NaiveTime::from_hms(20, 0, 0)),
            async move {
                let endpoint = &evening_endpoint.clone();
                CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_led(endpoint, r, v).await}, None).await
            }
        ));

        actions
    }

    async fn set_led(endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: (u16, u16, u16, u16, u32)) -> Result<(), String> {
        let addr = coap::ServiceDiscovery::new(endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(endpoint.clone()).send_setter_with_writer(&addr, rsrc, |msg_wrt| {
             let mut payload = BTreeMap::new();
             payload.insert("r", ciborium::value::Value::Integer(target.0.try_into().unwrap()));
             payload.insert("g", ciborium::value::Value::Integer(target.1.try_into().unwrap()));
             payload.insert("b", ciborium::value::Value::Integer(target.2.try_into().unwrap()));
             payload.insert("w", ciborium::value::Value::Integer(target.3.try_into().unwrap()));
             payload.insert("d", ciborium::value::Value::Integer(target.4.try_into().unwrap()));

             ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
             Ok(())
        }).await
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
        ).await;
    }
}
