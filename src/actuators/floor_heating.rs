use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use chrono::prelude::*;
use rust_decimal::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap;
use crate::state::{HcState, HvacState};

pub struct FloorHeating {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
    hvac_state: Arc<HvacState>,
}

impl FloorHeating {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
               hvac_state: Arc<HvacState>) -> Self {
        FloorHeating {
            local_endpoint,
            hvac_state,
        }
    }


    async fn get_action_list(&self) -> Vec<Action> {
        let mut actions = Vec::new();

        match self.hvac_state.get_state().await {
            HcState::HeatingActive | HcState::HeatingPassive => {
                let mut morning_action_list = Vec::new();
                morning_action_list.push(("gbrfh", Decimal::new(235, 1)));
                morning_action_list.push(("mbrfh", Decimal::new(235, 1)));
                morning_action_list.push(("kfh", Decimal::new(260, 1)));

                let mut evening_action_list = Vec::new();
                evening_action_list.push(("gbrfh", Decimal::new(200, 1)));
                evening_action_list.push(("mbrfh", Decimal::new(200, 1)));
                evening_action_list.push(("kfh", Decimal::new(200, 1)));

                let morning_endpoint = self.local_endpoint.clone();
                let evening_endpoint = self.local_endpoint.clone();

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(7, 0, 0)),
                    async move {
                        let endpoint = &morning_endpoint.clone();
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::set_temperature(endpoint, r, &v).await}, None).await
                    }
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(23, 0, 0)),
                    async move {
                        let endpoint = &evening_endpoint.clone();
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_temperature(endpoint, r, &v).await}, None).await
                    }
                ));
                println!("Heating");
            },
            HcState::CoolingActive | HcState::CoolingPassive => {
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("gbrfh", Decimal::new(200, 1)));
                evening_action_list.push(("mbrfh", Decimal::new(200, 1)));
                evening_action_list.push(("kfh", Decimal::new(200, 1)));

                let evening_endpoint = self.local_endpoint.clone();

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(23, 0, 0)),
                    async move {
                        let endpoint = &evening_endpoint.clone();
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_temperature(endpoint, r, &v).await}, None).await
                    }
                ));
                println!("Cooling");
            },
        }

        actions
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await }
        ).await;
    }

    async fn set_temperature(endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: &Decimal) -> Result<(), String> {
        let addr = coap::ServiceDiscovery::new(endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(endpoint.clone()).send_setter_with_writer(&addr, rsrc, |msg_wrt| {
            let mut payload = BTreeMap::new();
            payload.insert("s", coap::CborParser::from_decimal(target).map_err(|_e| async_coap::Error::InvalidArgument)?);

            ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
            Ok(())
        }).await
    }
}
