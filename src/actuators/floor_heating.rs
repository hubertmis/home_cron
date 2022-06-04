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


    async fn get_action_list(&self) -> Vec<Action<'static, Decimal>> {
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

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(7, 0, 0)),
                    morning_action_list,
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(23, 0, 0)),
                    evening_action_list,
                ));
                println!("Heating");
            },
            HcState::CoolingActive | HcState::CoolingPassive => {
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("gbrfh", Decimal::new(200, 1)));
                evening_action_list.push(("mbrfh", Decimal::new(200, 1)));
                evening_action_list.push(("kfh", Decimal::new(200, 1)));

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(23, 0, 0)),
                    evening_action_list,
                ));
                println!("Cooling");
            },
        }

        actions
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
            |r, v| async move { self.set_temperature(r, &v).await }
        ).await;
    }

    async fn set_temperature(&self, rsrc: &str, target: &Decimal) -> Result<(), String> {
        let addr = coap::ServiceDiscovery::new(self.local_endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(self.local_endpoint.clone()).send_setter_with_writer(&addr, rsrc, |msg_wrt| {
            let mut payload = BTreeMap::new();
            payload.insert("s", coap::CborParser::from_decimal(target).map_err(|_e| async_coap::Error::InvalidArgument)?);

            ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
            Ok(())
        }).await
    }
}
