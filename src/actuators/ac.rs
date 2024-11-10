use chrono::prelude::*;
use std::sync::Arc;

use crate::actuators::cron_processor::{Action, CronProcessor};
use crate::coap::{basic, CborMap};
use crate::state::{HcState, HvacState};

pub struct Ac {
    hvac_state: Arc<HvacState>,
}

impl Ac {
    pub fn new(hvac_state: Arc<HvacState>) -> Self {
        Self {
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

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(22, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_ac(r, v).await}, None).await
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
                evening_action_list.push(("bac", (true, 'a', 26)));
                evening_action_list.push(("dac", (true, 'a', 28)));
                evening_action_list.push(("lac", (true, 'a', 28)));
                evening_action_list.push(("oac", (true, 'a', 28)));

                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(7, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&morning_action_list, |r, v| async move {Self::set_ac(r, v).await}, None).await
                    }
                ));
                actions.push(Action::new(
                    CronProcessor::time_to_timestamp(NaiveTime::from_hms_opt(22, 0, 0).unwrap()),
                    async move {
                        CronProcessor::run_action(&evening_action_list, |r, v| async move {Self::set_ac(r, v).await}, None).await
                    }
                ));
            },
        }

        actions
    }

    async fn set_ac(rsrc: &str, target: (bool, char, u8)) -> Result<(), String> {
        let payload = [
                ("o", ciborium::value::Value::Bool(target.0)),
                ("f", ciborium::value::Value::Integer((target.1 as u8).try_into().unwrap())),
                ("t", ciborium::value::Value::Integer(target.2.try_into().unwrap())),
                ("m", ciborium::value::Value::Integer(('c' as u8).try_into().unwrap())),
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
