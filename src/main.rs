use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::prelude::*;
use rust_decimal::prelude::*;
use serde::Deserialize;

use socket2::{Socket, Domain, Type};
use async_coap::prelude::*;
use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use async_coap::uri::Uri;
use futures::prelude::*;

use rand::Rng;

async fn get_next_twilight() -> Result<[SystemTime; 2], String> {
    // TODO: Some kind of cache?
    #[derive(Deserialize)]
    struct SunData {
        results: BTreeMap<String, String>,
        status: String,
    }

    async fn sun_time_get<Tz: TimeZone>(day: Date<Tz>) -> Result<SunData, reqwest::Error> 
    where Tz::Offset: core::fmt::Display
    {
        let result = reqwest::get(format!("https://api.sunrise-sunset.org/json?lat=50.061389&lng=19.938333&date={}",
                                          &day.format("%Y-%m-%d").to_string()
                                         )).await?
                     .json::<SunData>().await?;
        Ok(result)
    }

    let today = Utc::today();
    let tomorrow = today.succ();

    let sun_data_today = sun_time_get(today).await.map_err(|e| e.to_string())?;
    let sun_data_tomorrow = sun_time_get(tomorrow).await.map_err(|e| e.to_string())?;

    if sun_data_today.status != "OK".to_string() {
        return Err("Status of retrieved today sun data is not OK".to_string());
    }
    if sun_data_tomorrow.status != "OK".to_string() {
        return Err("Status of retrieved tomorrow sun data is not OK".to_string());
    }

    if let (Some(twilight_begin_today_str), Some(twilight_end_today_str), 
            Some(twilight_begin_tomorrow_str), Some(twilight_end_tomorrow_str)) = 
            (sun_data_today.results.get(&"civil_twilight_begin".to_string()),
             sun_data_today.results.get(&"civil_twilight_end".to_string()),
             sun_data_tomorrow.results.get(&"civil_twilight_begin".to_string()),
             sun_data_tomorrow.results.get(&"civil_twilight_end".to_string())) {
        let twilight_beg_today_time = NaiveTime::parse_from_str(twilight_begin_today_str, "%r").map_err(|e| e.to_string())?;
        let twilight_end_today_time = NaiveTime::parse_from_str(twilight_end_today_str, "%r").map_err(|e| e.to_string())?;
        let twilight_beg_tomorrow_time = NaiveTime::parse_from_str(twilight_begin_tomorrow_str, "%r").map_err(|e| e.to_string())?;
        let twilight_end_tomorrow_time = NaiveTime::parse_from_str(twilight_end_tomorrow_str, "%r").map_err(|e| e.to_string())?;

        let now = Utc::now();
        let twilight_begin_today = today.and_time(twilight_beg_today_time).unwrap();
        let twilight_end_today = today.and_time(twilight_end_today_time).unwrap();
        let twilight_begin_tomorrow = tomorrow.and_time(twilight_beg_tomorrow_time).unwrap();
        let twilight_end_tomorrow = tomorrow.and_time(twilight_end_tomorrow_time).unwrap();

        let twilight_begin = if now > twilight_begin_today { twilight_begin_tomorrow } else { twilight_begin_today };
        let twilight_end = if now > twilight_end_today { twilight_end_tomorrow } else {twilight_end_today };

        Ok([twilight_begin.try_into().unwrap(), twilight_end.try_into().unwrap()])
    } else {
        Err("Missing twilight time in retrieved sun data".to_string())
    }
}

async fn wait_until_twilight(twilight_index: usize) -> Result<(), String> {
    let twilight = get_next_twilight().await?;

    let now = SystemTime::now();
    let sleep_time = twilight[twilight_index].duration_since(now).map_err(|e| e.to_string())?;
    tokio::time::sleep(sleep_time).await;
    Ok(())
}

async fn wait_until_time(time: NaiveTime) {
    let now = Local::now();
    let today = now.date();
    let tomorrow = today.succ();
    let today_time = today.and_time(time).unwrap();
    let tomorrow_time = tomorrow.and_time(time).unwrap();

    let target_time = if now > today_time { tomorrow_time } else { today_time };
    let sleep_time = (target_time - now).to_std().unwrap();
    tokio::time::sleep(sleep_time).await;
}

async fn wait_exp_retry<F, Fut>(f: F) 
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let mut retry_cnt: u32 = 0;
    let max_retry_cnt: u32 = 8;
    let min_wait_secs: i32 = 15;
    let rand_bound: i32 = (min_wait_secs/2).try_into().unwrap();

    loop {
        let result = f().await;

        match result {
            Ok(_) => break,
            Err(e) => {
                let rand_num;

                {
                    let mut rng = rand::thread_rng();
                    rand_num = rng.gen_range(-rand_bound..rand_bound);
                } 
                let wait_time = u64::try_from(2i32.pow(retry_cnt) * min_wait_secs + rand_num).unwrap();

                if retry_cnt < max_retry_cnt {
                    retry_cnt += 1;
                }

                println!("Error in waiting function: {}. Retrying in {} s", e.to_string(), wait_time);
                tokio::time::sleep(Duration::from_secs(wait_time)).await;
            }
        }
    }
}

async fn move_shades(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: &str) -> Result<(), String> {
    println!("{} {}", rsrc, target);

    let addr = service_discovery(local_endpoint, rsrc, None).await?;
    send_request(local_endpoint, &addr, rsrc, "val", target).await
}

fn convert_decimal_to_cborium(value: &Decimal) -> Result<ciborium::value::Value, std::num::TryFromIntError> {
    use ciborium::value::{Value, Integer};

    Ok(Value::Tag(4, Box::new(Value::Array(
            [Value::Integer(Integer::from(-i32::try_from(value.scale())?)),
             Value::Integer(Integer::from(i64::try_from(value.mantissa())?))
            ].to_vec()
    ))))
}

async fn set_temperature(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, rsrc: &str, target: &Decimal) -> Result<(), String> {
    let addr = service_discovery(local_endpoint, rsrc, None).await?;
    send_request_with_writer(local_endpoint, &addr, rsrc, |msg_wrt| {
        let mut payload = BTreeMap::new();
        payload.insert("s", convert_decimal_to_cborium(target).map_err(|_e| async_coap::Error::InvalidArgument)?);

        ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
        Ok(())
    }).await
}

async fn action_for_resources<'a, F, C, Fut>(local_endpoint: &'a Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
                                             resources: &[(&'a str, &'a C)],
                                             action: F,
                                             num_tries: Option<u32>)
    where F: Fn(&'a Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, &'a str, &'a C) -> Fut,
          C: ?Sized,
          Fut: futures::Future<Output = Result<(), String>>,
          Fut: 'a,
{
    // TODO: spawn threads for each of the resources to manage them in parallel?
    for rsrc in resources {
        let mut loop_cnt = num_tries.unwrap_or(4);
        if loop_cnt == 0 { loop_cnt = 1 } // TODO: Infinite number of retries for 0?

        loop {
            let result = action(local_endpoint, rsrc.0, rsrc.1).await;
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

async fn open_shades_on_dawn(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) {
    println!("Started shades opener");
    loop {
        wait_exp_retry(|| wait_until_twilight(0)).await;

        action_for_resources(local_endpoint,
                             &[("lr", "up"),
                               ("dr1", "up"),
                               ("dr2", "up"),
                               ("dr3", "up")],
                             move_shades,
                             Some(4)).await;
    }
}

async fn close_shades_on_dusk(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) {
    println!("Started shades closer");
    loop {
        wait_exp_retry(|| wait_until_twilight(1)).await;

        action_for_resources(local_endpoint,
                             &[("lr", "down"),
                               ("dr1", "down"),
                               ("dr2", "down"),
                               ("dr3", "down"),
                               ("k", "down")],
                             move_shades,
                             Some(4)).await;
    }
}

async fn enable_floor_heating_in_the_morning(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) {
    loop {
        wait_until_time(NaiveTime::from_hms(7, 0, 0)).await;

        action_for_resources(local_endpoint, 
                             &[("gbrfh", &Decimal::new(235, 1)),
                               ("mbrfh", &Decimal::new(235, 1)),
                               ("kfh", &Decimal::new(260, 1))],
                             set_temperature,
                             Some(4)
                            ).await;
    }
}

async fn disable_floor_heating_in_the_evening(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) {
    loop {
        wait_until_time(NaiveTime::from_hms(23, 0, 0)).await;

        action_for_resources(local_endpoint, 
                             &[("gbrfh", &Decimal::new(200, 1)),
                               ("mbrfh", &Decimal::new(200, 1)),
                               ("kfh", &Decimal::new(200, 1))],
                             set_temperature,
                             Some(4)
                            ).await;
    }
}

async fn service_discovery(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, srv_name: &str, srv_type: Option<&str>) -> Result<SocketAddr, String> {
    // TODO: Some kind of cache?
    let remote_endpoint = local_endpoint.remote_endpoint_from_uri(uri!("coap://[ff05::1]")).unwrap();

    let future_result= remote_endpoint.send_to(
        rel_ref!("sd"),
        CoapRequest::get()
            .multicast()
            .content_format(ContentFormat::APPLICATION_CBOR)
            .payload_writer(|msg_wrt| {
                let mut payload = BTreeMap::new();
                payload.insert("name", srv_name);
                if let Some(srv_type) = srv_type {
                    payload.insert("type", srv_type);
                }

                msg_wrt.set_msg_code(MsgCode::MethodGet);
                msg_wrt.set_msg_type(MsgType::Non);
                ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
                Ok(())
            })
            .use_handler(|context| {
                let data : BTreeMap<String, BTreeMap<String, String>> = ciborium::de::from_reader(context.unwrap().message().payload()).unwrap(); // TODO: Handle errors

                for (service, _details) in data.iter() {
                    if service == srv_name {
                        return Ok(ResponseStatus::Done(context.unwrap().remote_socket_addr()));
                    }
                }

                Ok(ResponseStatus::Continue)
            }) 
        );

    let result = future_result.await;
    result.map_err(|e| e.to_string())
}

async fn send_request_with_writer<F>(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, addr: &SocketAddr, resource: &str, writer: F) -> Result<(), String> 
    where F: Fn(&mut dyn async_coap::message::MessageWrite) -> Result<(), async_coap::Error> + Send,
{
    let uri = String::new() + "coap://" + &addr.to_string();
    let remote_endpoint = local_endpoint.remote_endpoint_from_uri(Uri::from_str(&uri).unwrap()).unwrap();

    let future_result = remote_endpoint.send_to(
        RelRef::from_str(resource).unwrap(),
        CoapRequest::post()
            .content_format(ContentFormat::APPLICATION_CBOR)
            .payload_writer(move |msg_wrt| {
                msg_wrt.set_msg_code(MsgCode::MethodPost);
                writer(msg_wrt)
            })
            .emit_successful_response()
        );

    let result = future_result.await;
    result.map(|_| ()).map_err(|e| e.to_string())
}

async fn send_request(local_endpoint: &Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>, addr: &SocketAddr, resource: &str, key: &str, value: &str) -> Result<(), String> {
    send_request_with_writer(local_endpoint, addr, resource, |msg_wrt| {
             let mut payload = BTreeMap::new();
             payload.insert(key, value);

             ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
             Ok(())
         }).await
}

#[tokio::main]
async fn main() {
    let udp_socket = Socket::new(Domain::IPV6, Type::DGRAM, None).expect("Socket creating failed");
    let address: SocketAddr = "[::]:0".parse().unwrap();
    let address = address.into();
    udp_socket.set_nonblocking(true).unwrap();
    udp_socket.set_multicast_hops_v6(16).expect("Setting multicast hops failed");
    udp_socket.bind(&address).expect("UDP bind failed");

    let socket = AllowStdUdpSocket::from_std(udp_socket.into());
    let local_endpoint = Arc::new(DatagramLocalEndpoint::new(socket));

    let mut tasks = Vec::new();

    tasks.push(tokio::spawn(local_endpoint
                            .clone()
                            .receive_loop_arc(null_receiver!())
                            .map(|err| panic!("CoAP recv loop terminated: {}", err))
    ));

    let shades_open_local_endpoint = local_endpoint.clone();
    let shades_close_local_endpoint = local_endpoint.clone();
    let fh_enabler_local_endpoint = local_endpoint.clone();
    let fh_disabler_local_endpoint = local_endpoint.clone();

    tasks.push(tokio::spawn(async move {
        open_shades_on_dawn(&shades_open_local_endpoint).await;
    }));
    tasks.push(tokio::spawn(async move {
        close_shades_on_dusk(&shades_close_local_endpoint).await;
    }));
    tasks.push(tokio::spawn(async move {
        enable_floor_heating_in_the_morning(&fh_enabler_local_endpoint).await;
    }));
    tasks.push(tokio::spawn(async move {
        disable_floor_heating_in_the_evening(&fh_disabler_local_endpoint).await;
    }));

    for task in tasks {
        task.await.expect("Failed infinite task");
    }
}
