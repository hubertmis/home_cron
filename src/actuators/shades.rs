use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use chrono::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap;
use crate::state::{HcState, HvacState};
use crate::web;

pub struct Shades {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
    hvac_state: Arc<HvacState>,
    weather: Arc<web::Weather>,
}

impl Shades {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
               hvac_state: Arc<HvacState>,
               weather: Arc<web::Weather>,
              ) -> Self {
        Self {
            local_endpoint,
            hvac_state,
            weather,
        }
    }

    async fn get_action_list(&self) -> Vec<Action>
    {
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

                let morning_endpoint = self.local_endpoint.clone();
                let evening_endpoint = self.local_endpoint.clone();

                actions.push(Action::new(
                    web::Twilight::new().get_pair().await.unwrap()[0],
                    async move {
                        let endpoint = &morning_endpoint.clone();
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::move_shades(endpoint, r, v).await}, None).await
                    }
                ));
                actions.push(Action::new(
                    web::Twilight::new().get_pair().await.unwrap()[1],
                    async move {
                        let endpoint = &evening_endpoint.clone();
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::move_shades(endpoint, r, v).await}, None).await
                    }
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

                let morning_endpoint = self.local_endpoint.clone();
                let noon_endpoint = self.local_endpoint.clone();

                let morning_weather = self.weather.clone();

                actions.push(Action::new(
                    web::Twilight::new().get_pair().await.unwrap()[0],
                    async move {
                        let forecast = morning_weather.get_forecast(&Duration::from_secs(3600*6)).await;
                        if forecast.is_ok() {
                            let forecast = forecast.unwrap();
                            if forecast.get_cloudiness() > 50 {
                                return ()
                            }
                        }

                        let endpoint = &morning_endpoint.clone();
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {
                            Self::move_shades(endpoint, r, v).await
                        }, None).await
                    }
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms(12, 0, 0)),
                    async move {
                        let endpoint = &noon_endpoint.clone();
                        CronProcessor::run_action(&noon_action_list, |r, v| async move {Self::move_shades(endpoint, r, v).await}, None).await
                    }
                ));
                println!("Cooling");
            },
        }

        actions
    }

    async fn move_shades(endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: u16) -> Result<(), String> {
        println!("{} {}", rsrc, target);

        let addr = coap::ServiceDiscovery::new(endpoint.clone()).service_discovery(rsrc, None).await?;
        coap::Basic::new(endpoint.clone()).send_setter(&addr, rsrc, "val", target).await
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
        ).await;
    }
}
