use chrono::prelude::*;
use futures::prelude::*;
use std::boxed::Box;
use std::pin::Pin;
use std::time::{Duration, SystemTime};

pub struct Action
{
    time: SystemTime,
    function: Pin<Box<dyn Future<Output=()> + Send>>,
}

impl Action
{
    pub fn new(time: SystemTime, function: impl Future<Output=()> + Send + 'static) -> Self
    {
        Action {
            time,
            function: Box::pin(function),
        }
    }
}

pub struct CronProcessor ();

impl CronProcessor {
    pub fn new() -> Self {
        CronProcessor {}
    }

    pub async fn process<FG, FGFut>(&self, get_actions: FG)
        where
        FG: Fn() -> FGFut,
        FGFut: Future<Output = Vec<Action>>,
    {
        loop {
            let actions = get_actions().await;

            {
                let mut next_action: Option<Action> = None;
                let now = SystemTime::now();
                for action in actions {
                    if action.time <= now {
                        continue;
                    }
                    if next_action.is_none() || action.time < next_action.as_ref().unwrap().time {
                        next_action = Some(action);
                    }
                }
                
                let next_action = next_action.unwrap(); // TODO: handle errors
                let now = SystemTime::now();
                let sleep_time = next_action.time.duration_since(now).map_err(|e| e.to_string()).unwrap(); // TODO: Handle errors
                println!("Sleeping for {:?}", sleep_time);
                tokio::time::sleep(sleep_time).await;

                next_action.function.await;
            }
        }
    }

    pub async fn run_action<'a, F, C, Fut>(resources: &[(&'a str, C)],
                                           action: F,
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
        let today = now.date_naive();
        let tomorrow = today.succ_opt().unwrap();
        let today_time = today.and_time(time);
        let tomorrow_time = tomorrow.and_time(time);
        let today_time_with_tz = today_time.and_local_timezone(Local).earliest().unwrap(); // TODO: handle gap

        let target_time = if now > today_time_with_tz { tomorrow_time } else { today_time };
        target_time.and_utc().try_into().unwrap()
    }
}
