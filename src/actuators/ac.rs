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

    async fn get_action_list(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        
        match self.hvac_state.get_state().await {
            HcState::HeatingActive | HcState::HeatingPassive | HcState::CoolingPassive => {
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("bac", (false, 'a', 27)));
                evening_action_list.push(("dac", (false, 'a', 27)));
                evening_action_list.push(("lac", (false, 'a', 27)));
                evening_action_list.push(("oac", (false, 'a', 27)));

                let evening_endpoint = self.local_endpoint.clone();

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(22, 0, 0)),
                    async move {
                        let endpoint = &evening_endpoint.clone();
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_ac(endpoint, r, v).await}, None).await
                    }
                ));
            },
            HcState::CoolingActive => {
                let mut morning_action_list = Vec::new();
                morning_action_list.push(("bac", (true, 'a', 26)));
                morning_action_list.push(("dac", (true, 'a', 26)));
                morning_action_list.push(("lac", (true, 'a', 26)));
                morning_action_list.push(("oac", (true, 'a', 26)));
                
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("bac", (true, 'a', 27)));
                evening_action_list.push(("dac", (true, 'a', 28)));
                evening_action_list.push(("lac", (true, 'a', 28)));
                evening_action_list.push(("oac", (true, 'a', 28)));

                let morning_endpoint = self.local_endpoint.clone();
                let evening_endpoint = self.local_endpoint.clone();

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(7, 0, 0)),
                    async move {
                        let endpoint = &morning_endpoint.clone();
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::set_ac(endpoint, r, v).await}, None).await
                    }
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(22, 0, 0)),
                    async move {
                        let endpoint = &evening_endpoint.clone();
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_ac(endpoint, r, v).await}, None).await
                    }
                ));
            },
        }

        actions
    }

    async fn set_ac(endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: (bool, char, u8)) -> Result<(), String> {
        let addr = coap::ServiceDiscovery::new(endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(endpoint.clone()).send_setter_with_writer(&addr, rsrc, |msg_wrt| {
             let mut payload = BTreeMap::new();
             payload.insert("o", ciborium::value::Value::Bool(target.0));
             payload.insert("f", ciborium::value::Value::Integer((target.1 as u8).try_into().unwrap()));
             payload.insert("t", ciborium::value::Value::Integer(target.2.try_into().unwrap()));
             payload.insert("m", ciborium::value::Value::Integer(('c' as u8).try_into().unwrap()));

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
