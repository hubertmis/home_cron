use chrono::prelude::*;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap::{basic, CborMap};
use crate::state::{HcState, HvacState};
use crate::web;

pub struct Shades {
    hvac_state: Arc<HvacState>,
    weather: Arc<web::Weather>,
}

impl Shades {
    pub fn new(hvac_state: Arc<HvacState>,
               weather: Arc<web::Weather>,
              ) -> Self {
        Self {
            hvac_state,
            weather,
        }
    }

    async fn get_twilight_pair() -> [SystemTime; 2]
    {
        // TODO: Align it to the time of the year
        let morning_datetime = CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(6, 30, 0).unwrap());
        let evening_datetime = CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(19, 0, 0).unwrap());

        web::Twilight::new().get_pair().await.or::<Result<[SystemTime; 2], String>>(
            Ok([morning_datetime.try_into().unwrap(),
                evening_datetime.try_into().unwrap()]))
            .unwrap()
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
                morning_action_list.push(("k", 0));

                let mut evening_action_list = Vec::new();
                evening_action_list.push(("lr", 256));
                evening_action_list.push(("dr1", 256));
                evening_action_list.push(("dr2", 256));
                evening_action_list.push(("dr3", 256));
                evening_action_list.push(("k", 256));

                let twilight_pair =  Shades::get_twilight_pair().await;
                let morning_time = twilight_pair[0];
                let evening_time = twilight_pair[1];

                actions.push(Action::new(
                    morning_time,
                    async move {
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::move_shades(r, v).await}, None).await
                    }
                ));
                actions.push(Action::new(
                    evening_time,
                    async move {
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::move_shades(r, v).await}, None).await
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

                let morning_weather = self.weather.clone();
                let morning_time = Shades::get_twilight_pair().await[0];

                actions.push(Action::new(
                    morning_time,
                    async move {
                        let forecast = morning_weather.get_forecast(&Duration::from_secs(3600*6)).await;
                        if forecast.is_ok() {
                            let forecast = forecast.unwrap();
                            if forecast.get_cloudiness() > 50 {
                                println!("Expected morning clouds: {}. Skip shading", forecast.get_cloudiness());
                                return ()
                            }
                        }

                        CronProcessor::run_action(&morning_action_list, |r, v| async move {
                            Self::move_shades(r, v).await
                        }, None).await
                    }
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&noon_action_list, |r, v| async move {Self::move_shades(r, v).await}, None).await
                    }
                ));
                println!("Cooling");
            },
        }

        actions
    }

    async fn move_shades(rsrc: &str, target: u16) -> Result<(), String> {
        println!("{} {}", rsrc, target);

        let payload = [
                ("val", ciborium::value::Value::Integer(target.try_into().unwrap())),
        ];
        let payload = CborMap::from_slice(&payload);

        basic::set_actuator(rsrc, payload).await
    }

    pub async fn process(&self) {
        let cp = CronProcessor::new();

        cp.process(
            || async { self.get_action_list().await },
        ).await;
    }
}
