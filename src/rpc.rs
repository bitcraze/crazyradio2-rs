use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, RwLock},
    thread::spawn,
};

use ciborium::{cbor, de::from_reader, ser::into_writer, value::Value};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("RPC Error response: {0}")]
    RpcError(String),

    #[error("RPC Error value")]
    RpcErrorValue(Value),

    #[error("Serialize error {0}")]
    SerializeError(#[from] ciborium::ser::Error<std::io::Error>),

    #[error("CBOR Value error {0}")]
    CborValueError(#[from] ciborium::value::Error),

    #[error("Deserialize error {0}, received: {1:?}")]
    DeserializeError(#[source] ciborium::value::Error, Value),

    #[error("Protocol Error {0}")]
    ProtocolError(#[from] crate::Error),

    #[error("Channel Error {0}")]
    ChannelError(#[from] flume::RecvError),

    #[error("RPC TX thread error. Thread likely panicked.")]
    RpcTxThreadError,
}

pub struct Rpc {
    device: Arc<crate::usb_protocol::UsbProtocol>,

    current_calls: Arc<RwLock<HashMap<u64, flume::Sender<Result<Value, Value>>>>>,

    sequence: AtomicU64,

    methods: HashMap<String, u32>,
}

impl Rpc {
    pub fn new(device: crate::usb_protocol::UsbProtocol) -> Self {
        let current_calls: Arc<RwLock<HashMap<u64, flume::Sender<Result<Value, Value>>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let sequence = AtomicU64::new(0);

        let device = Arc::new(device);

        {
            let device = device.clone();
            let current_calls = current_calls.clone();
            spawn(move || loop {
                let data = device.recv().unwrap();
                let (_, sequence, error, result): (u32, u64, Value, Value) =
                    from_reader(data.as_slice()).unwrap();

                if let Some(call) = current_calls.write().unwrap().remove(&sequence) {
                    if error.is_null() {
                        call.send(Ok(result)).unwrap();
                    } else {
                        call.send(Err(error)).unwrap();
                    }
                } else {
                    println!("RPC: Received response for unknown call: {}", sequence);
                }
            });
        }

        let methods = HashMap::new();

        let mut rpc = Self {
            device,
            current_calls,
            sequence,
            methods,
        };

        if let Ok(methods) = rpc.call("well-known.methods", Value::Null) {
            rpc.methods = methods;
        }

        rpc
    }

    pub fn call<P, R>(&self, method: &str, params: P) -> Result<R, RpcError>
    where
        P: serde::ser::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let next_sequence = self
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (tx, rx) = flume::bounded(1);
        self.current_calls
            .write()
            .map_err(|_| RpcError::RpcTxThreadError)?
            .insert(next_sequence, tx);

        let request = if let Some(method_id) = self.methods.get(method) {
            cbor!([0, next_sequence, *method_id, params])?
        } else {
            cbor!([0, next_sequence, method, params])?
        };
        let mut request_bytes = vec![];
        into_writer(&request, &mut request_bytes)?;

        self.device.send(&request_bytes)?;
        let response = match rx.recv()? {
            Ok(v) => Ok(v
                .deserialized()
                .map_err(|e| RpcError::DeserializeError(e, v))?),
            Err(v) => {
                if let Some(text) = v.as_text() {
                    Err(RpcError::RpcError(text.to_string()))
                } else {
                    Err(RpcError::RpcErrorValue(v))
                }
            }
        }?;

        Ok(response)
    }
}
