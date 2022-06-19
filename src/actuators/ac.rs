use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use chrono::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap;
use crate::state::{HcState, HvacState};

pub struct Ac {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
    hvac_state: Arc<HvacState>,
}

impl Ac {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
               hvac_state: Arc<HvacState>) -> Self {
        Self {
            local_endpoint,
            hvac_state,
        }
    }

    async fn get_action_list(&self) -> Vec<Action<'static, (bool, char) >> {
        let mut actions = Vec::new();
        
        match self.hvac_state.get_state().await {
            HcState::HeatingActive | HcState::HeatingPassive | HcState::CoolingPassive => {
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("bac", (false, 'a')));

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(22, 0, 0)),
                    evening_action_list,
                ));
            },
            HcState::CoolingActive => {
                let mut morning_action_list = Vec::new();
                morning_action_list.push(("bac", (true, 'a')));
                
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("bac", (true, '1')));

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(7, 0, 0)),
                    morning_action_list,
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(22, 0, 0)),
                    evening_action_list,
                ));
            },
        }

        actions
    }

    async fn set_ac(&self, rsrc: &str, target: (bool, char)) -> Result<(), String> {
        let addr = coap::ServiceDiscovery::new(self.local_endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(self.local_endpoint.clone()).send_setter_with_writer(&addr, rsrc, |msg_wrt| {
             let mut payload = BTreeMap::new();
             payload.insert("o", ciborium::value::Value::Bool(target.0));
             payload.insert("f", ciborium::value::Value::Integer((target.1 as u8).try_into().unwrap()));

             ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
             Ok(())
        }).await
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
            |r, v| async move { self.set_ac(r, v).await }
        ).await;
    }
}
