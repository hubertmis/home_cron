use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use chrono::prelude::*;
use std::sync::Arc;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap;
use crate::state::{HcState, HvacState};
use crate::web;

pub struct Shades {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
    hvac_state: Arc<HvacState>,
}

impl Shades {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
               hvac_state: Arc<HvacState>) -> Self {
        Shades {
            local_endpoint,
            hvac_state,
        }
    }

    async fn get_action_list(&self) -> Vec<Action<'static, u16>> {
        let mut actions = Vec::new();
        
        match self.hvac_state.get_state().await {
            HcState::HeatingActive | HcState::HeatingPassive => {
                let mut morning_action_list = Vec::new();
                morning_action_list.push(("lr", 0));
                morning_action_list.push(("dr1", 0));
                morning_action_list.push(("dr2", 0));
                morning_action_list.push(("dr3", 0));

                let mut evening_action_list = Vec::new();
                evening_action_list.push(("lr", 256));
                evening_action_list.push(("dr1", 256));
                evening_action_list.push(("dr2", 256));
                evening_action_list.push(("dr3", 256));

                actions.push(Action::new(
                    web::Twilight::new().get_pair().await.unwrap()[0],
                    morning_action_list,
                ));
                actions.push(Action::new(
                    web::Twilight::new().get_pair().await.unwrap()[1],
                    evening_action_list,
                ));
                println!("Heating");
            },
            HcState::CoolingActive | HcState::CoolingPassive => {
                let mut morning_action_list = Vec::new();
                morning_action_list.push(("lr", 128));
                morning_action_list.push(("dr1", 128));
                morning_action_list.push(("dr2", 128));
                morning_action_list.push(("dr3", 128));
                
                let mut noon_action_list = Vec::new();
                noon_action_list.push(("lr", 0));
                noon_action_list.push(("dr1", 0));
                noon_action_list.push(("dr2", 0));
                noon_action_list.push(("dr3", 0));

                actions.push(Action::new(
                    web::Twilight::new().get_pair().await.unwrap()[0],
                    morning_action_list,
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(12, 0, 0)),
                    noon_action_list,
                ));
                println!("Cooling");
            },
        }

        actions
    }

    async fn move_shades(&self, rsrc: &str, target: u16) -> Result<(), String> {
        println!("{} {}", rsrc, target);

        let addr = coap::ServiceDiscovery::new(self.local_endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(self.local_endpoint.clone()).send_setter(&addr, rsrc, "val", target).await
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
            |r, v| async move { self.move_shades(r, v).await }
        ).await;
    }
}
