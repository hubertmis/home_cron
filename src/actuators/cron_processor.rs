use chrono::prelude::*;
use futures::prelude::*;
use std::time::{Duration, SystemTime};

#[derive(Debug)]
pub struct Action<'a, V> {
    time: SystemTime,
    action_list: Vec<(&'a str, V)>,
}

impl<'a, V> Action<'a, V> {
    pub fn new(time: SystemTime, action_list: Vec<(&'a str, V)>) -> Self {
        Action {
            time,
            action_list,
        }
    }
}

pub struct CronProcessor ();

impl CronProcessor {
    pub fn new() -> Self {
        CronProcessor {}
    }

    pub async fn process<'a, V, FG, FGFut, FA, FAFut>(&self, get_actions: FG, run_action: FA)
        where
        V: Copy + std::fmt::Debug,
        FG: Fn() -> FGFut,
        FGFut: Future<Output = Vec<Action<'a, V>>>,
        FA: Fn(&'a str, V) -> FAFut,
        FAFut: 'a + Future<Output = Result<(), String>>,
    {
        loop {
            let actions = get_actions().await;

            {
                let mut next_action: Option<Action<V>> = None;
                let now = SystemTime::now();
                for action in actions {
                    if action.time <= now {
                        continue;
                    }
                    if next_action.is_none() || action.time < next_action.as_ref().unwrap().time {
                        next_action = Some(action);
                    }
                }
                
                println!("Next action is: {:?}", next_action);

                let next_action = next_action.unwrap(); // TODO: handle errors
                let now = SystemTime::now();
                let sleep_time = next_action.time.duration_since(now).map_err(|e| e.to_string()).unwrap(); // TODO: Handle errors
                println!("Sleeping for {:?}", sleep_time);
                tokio::time::sleep(sleep_time).await;

                self.run_action(&next_action.action_list, &run_action, Some(4)).await;
            }
        }
    }

    async fn run_action<'a, F, C, Fut>(&self,
                                       resources: &[(&'a str, C)],
                                       action: &F,
                                       num_tries: Option<u32>)
        where F: Fn(&'a str, C) -> Fut,
              C: Sized + Copy,
              Fut: futures::Future<Output = Result<(), String>>,
              Fut: 'a,
    {
        // TODO: spawn threads for each of the resources to manage them in parallel?
        for rsrc in resources {
            let mut loop_cnt = num_tries.unwrap_or(4);
            if loop_cnt == 0 { loop_cnt = 1 } // TODO: Infinite number of retries for 0?

            loop {
                let result = action(rsrc.0, rsrc.1).await;
                match result {
                    Ok(_) => break,
                    Err(e) => {
                        println!("Error handling action for resource {}: {}", rsrc.0, e); // TODO: Better error handlig
                        loop_cnt -= 1;
                        if loop_cnt == 0 {
                            break;
                        }

                        tokio::time::sleep(Duration::from_secs(15)).await;
                    }
                }
            }
        }
    }
    
    pub fn time_to_timestamp(time: NaiveTime) -> SystemTime {
        let now = Local::now();
        let today = now.date();
        let tomorrow = today.succ();
        let today_time = today.and_time(time).unwrap();
        let tomorrow_time = tomorrow.and_time(time).unwrap();

        let target_time = if now > today_time { tomorrow_time } else { today_time };
        target_time.try_into().unwrap()
    }
}
