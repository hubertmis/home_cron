use async_coap::datagram::{DatagramLocalEndpoint, AllowStdUdpSocket};
use async_coap::prelude::*;
use async_coap::uri::Uri;
use serde::Serialize;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct Basic {
    local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>,
}

impl Basic {
    pub fn new(local_endpoint: Arc<DatagramLocalEndpoint<AllowStdUdpSocket>>) -> Self {
        Basic {
            local_endpoint
        }
    }

    pub async fn send_getter<F, FR>(self, addr: &SocketAddr, resource: &str, handler: F) -> Result<(), String> 
        where F: FnMut(Result<&dyn async_coap::InboundContext<SocketAddr = std::net::SocketAddr>, async_coap::Error>) -> Result<ResponseStatus<FR>, async_coap::Error> + Send,
              FR: Send,
    {
        let uri = String::new() + "coap://" + &addr.to_string();
        let remote_endpoint = self.local_endpoint.remote_endpoint_from_uri(Uri::from_str(&uri).unwrap()).unwrap();

        let future_result = remote_endpoint.send_to(
            RelRef::from_str(resource).unwrap(),
            CoapRequest::get()
                .use_handler(handler)
            );

        let result = future_result.await;
        result.map(|_| ()).map_err(|e| e.to_string())
    }

    pub async fn send_setter_with_writer<F>(&self, addr: &SocketAddr, resource: &str, writer: F) -> Result<(), String> 
        where F: Fn(&mut dyn async_coap::message::MessageWrite) -> Result<(), async_coap::Error> + Send,
    {
        let uri = String::new() + "coap://" + &addr.to_string();
        let remote_endpoint = self.local_endpoint.remote_endpoint_from_uri(Uri::from_str(&uri).unwrap()).unwrap();

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

    pub async fn send_setter<T>(&self, addr: &SocketAddr, resource: &str, key: &str, value: T) -> Result<(), String>
        where T: Serialize + Copy + Send + Sync
    {
        self.send_setter_with_writer(addr, resource, |msg_wrt| {
                 let mut payload = BTreeMap::new();
                 payload.insert(key, value);

                 ciborium::ser::into_writer(&payload, msg_wrt).unwrap();
                 Ok(())
             }).await
    }

}
