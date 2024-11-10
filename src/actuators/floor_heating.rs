use chrono::prelude::*;
use rust_decimal::prelude::*;
use std::sync::Arc;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap;
use crate::coap::{basic, CborMap};
use crate::state::{HcState, HvacState};

pub struct FloorHeating {
    hvac_state: Arc<HvacState>,
}

impl FloorHeating {
    pub fn new(hvac_state: Arc<HvacState>) -> Self {
        FloorHeating {
            hvac_state,
        }
    }


    async fn get_action_list(&self) -> Vec<Action> {
        let disabled = Decimal::new(175, 1);
        let mut actions = Vec::new();

        match self.hvac_state.get_state().await {
            HcState::HeatingActive | HcState::HeatingPassive => {
                let mut morning_action_list = Vec::new();
                morning_action_list.push(("gbrfh", Decimal::new(240, 1)));
                morning_action_list.push(("mbrfh", Decimal::new(240, 1)));
                morning_action_list.push(("kfh", Decimal::new(245, 1)));

                let mut evening_action_list = Vec::new();
                evening_action_list.push(("gbrfh", disabled));
                evening_action_list.push(("mbrfh", disabled));
                evening_action_list.push(("kfh", disabled));

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(7, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::set_temperature(r, &v).await}, None).await
                    }
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(23, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_temperature(r, &v).await}, None).await
                    }
                ));
                println!("Heating");
            },
            HcState::CoolingActive | HcState::CoolingPassive => {
                let mut evening_action_list = Vec::new();
                evening_action_list.push(("gbrfh", disabled));
                evening_action_list.push(("mbrfh", disabled));
                evening_action_list.push(("kfh", disabled));

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(23, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_temperature(r, &v).await}, None).await
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

    async fn set_temperature(rsrc: &str, target: &Decimal) -> Result<(), String> {
        let payload = [
                ("s", coap::CborParser::from_decimal(target).map_err(|e| e.to_string())?),
        ];
        let payload = CborMap::from_slice(&payload);

        basic::set_actuator(rsrc, payload).await
    }
}
