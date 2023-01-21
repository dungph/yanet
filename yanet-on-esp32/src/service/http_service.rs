//use crate::controller::Controller; use crate::storage::StorageService;
//use crate::wifi::WifiService;
//use anyhow::Result;
//use http::header::HeaderName;
//use http::{HeaderValue, Method, Response};
//use httparse::Status;
//use serde::{Deserialize, Serialize};
//use serde_json::Value;
//use std::collections::BTreeMap;
//use std::io::{self, Read, Write};
//use std::net::{TcpListener, TcpStream};
//use std::time::Duration;
//
//pub async fn try_async<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
//    loop {
//        match f() {
//            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
//                futures_timer::Delay::new(Duration::from_millis(100)).await;
//            }
//            res => return res,
//        }
//    }
//}
//pub struct HttpServe {
//    listener: TcpListener,
//}
//
//impl HttpServe {
//    pub fn new(_: &WifiService) -> Result<Self> {
//        let listener = std::net::TcpListener::bind("0.0.0.0:80")?;
//        Ok(Self { listener })
//    }
//    pub async fn run<'a>(&self, controller: &Controller<'a>, storage: &StorageService) {
//        self.listener.set_nonblocking(true).unwrap();
//        while let Ok((stream, _addr)) = try_async(|| self.listener.accept()).await {
//            stream.set_nonblocking(true).ok();
//            Self::handle_stream(stream, controller, storage).await.ok();
//        }
//    }
//
//    async fn handle_stream<'a>(
//        mut stream: TcpStream,
//        controller: &Controller<'a>,
//        storage: &StorageService,
//    ) -> Result<()> {
//        let mut buf = [0u8; 1024];
//        let mut start = 0;
//        let req = loop {
//            let len = try_async(|| stream.read(&mut buf[start..])).await?;
//            start += len;
//            if let Some((req, size)) = try_parse_request(&buf[..start])? {
//                if size < start {
//                    buf.copy_within(size..start, 0);
//                    start -= size;
//                }
//                break req;
//            }
//        };
//
//        match (req.method().clone(), req.uri().path()) {
//            (Method::GET, "/") => {
//                let res = Response::new(include_str!("index.html"));
//                write_respond(stream, res).await?;
//            }
//            (Method::GET, "/schema") => {
//                let body = dbg!(serde_json::to_string(&controller.get_schema()))?;
//                let mut res = Response::new(&body);
//                res.headers_mut()
//                    .append("Content-Type", HeaderValue::from_str("application/json")?);
//                dbg!(write_respond(stream, res).await)?;
//            }
//            (Method::GET, "/data") => {
//                #[derive(Deserialize)]
//                struct Query {
//                    field: String,
//                }
//
//                #[derive(Serialize)]
//                struct Ret {
//                    value: Value,
//                }
//                if let Some(Query { field }) =
//                    req.uri().query().map(serde_qs::from_str).transpose()?
//                {
//                    let value = storage.get(field.as_str());
//                    let ret = Ret { value };
//
//                    let body = serde_json::to_string(&ret)?;
//                    let mut res = Response::new(&body);
//                    res.headers_mut()
//                        .append("Content-Type", HeaderValue::from_str("application/json")?);
//                    write_respond(stream, res).await?;
//                }
//            }
//            (Method::POST, "/data") => {
//                let len = if let Some(value) = req.headers().get("Content-Length") {
//                    let len = value.to_str()?.parse()?;
//                    if len > start {
//                        stream.read_exact(&mut buf[start..][..len - start])?;
//                    }
//                    len
//                } else {
//                    start
//                };
//                let val: BTreeMap<String, Value> = serde_json::from_slice(&buf[..len])?;
//                val.into_iter().for_each(|(k, v)| storage.set_check(&k, v));
//                let res = Response::new(b"");
//                write_respond(stream, res).await?;
//            }
//            _ => (),
//        }
//
//        Ok(())
//    }
//}
////pub struct HttpServe {
////    handle_value: Arc<Mutex<ThingSchema>>,
////    receiver: Receiver<Value>,
////}
////
////impl HttpServe {
////    pub fn new(_wifi: WifiService<'_>) -> Self {
////        let (sender, receiver) = bounded(2);
////        let handle_value = Arc::new(Mutex::new(ThingSchema::default()));
////        let value = handle_value.clone();
////        std::thread::Builder::new()
////            .name(String::from("http serve"))
////            .stack_size(10_000)
////            .spawn(move || loop {
////                let sender = sender.clone();
////                if let Ok(listener) = std::net::TcpListener::bind("0.0.0.0:80") {
////                    while let Ok((stream, _addr)) = listener.accept() {
////                        let sender = sender.clone();
////                        handle_stream(stream, value.clone(), sender).ok();
////                    }
////                }
////                std::thread::sleep(Duration::from_millis(1000));
////            })
////            .unwrap();
////        Self {
////            receiver,
////            handle_value,
////        }
////    }
////    pub async fn set_schema(&self, data: ThingSchema) {
////        *self.handle_value.lock().await = data
////    }
////}
////
////fn handle_stream(
////    mut stream: TcpStream,
////    value: Arc<Mutex<ThingSchema>>,
////    sender: Sender<Value>,
////) -> Result<()> {
////    let mut buf = [0u8; 1024];
////    let mut start = 0;
////    let req = loop {
////        let len = stream.read(&mut buf[start..])?;
////        start += len;
////        if let Some((req, size)) = try_parse_request(&buf[..start])? {
////            if size < start {
////                buf.copy_within(size..start, 0);
////                start -= size;
////            }
////            break req;
////        }
////    };
////
////    match dbg!(req.method().clone(), req.uri().path()) {
////        (Method::GET, "/") => {
////            let res = Response::new(include_str!("index.html"));
////            write_respond(stream, res)?;
////        }
////        (Method::GET, "/data") => {
////            let value = loop {
////                if let Some(value) = value.try_lock() {
////                    break value;
////                } else {
////                    std::thread::sleep(Duration::from_millis(500));
////                }
////            };
////            let body = serde_json::to_vec(&*value)?;
////            let mut res = Response::new(body);
////            res.headers_mut()
////                .append("Content-Type", HeaderValue::from_str("application/json")?);
////            write_respond(stream, res)?;
////        }
////        (Method::POST, "/data") => {
////            let len = if let Some(value) = req.headers().get("Content-Length") {
////                let len = value.to_str()?.parse()?;
////                if len > start {
////                    stream.read_exact(&mut buf[start..][..len - start])?;
////                }
////                len
////            } else {
////                start
////            };
////            let val: Value = serde_json::from_slice(&buf[..len])?;
////            sender.send_blocking(val)?;
////            let res = Response::new(b"");
////            write_respond(stream, res)?;
////        }
////        _ => (),
////    }
////
////    Ok(())
////}
////fn write_respond<T: AsRef<[u8]>>(mut stream: impl Write, response: Response<T>) -> Result<()> {
////    stream.write_all(b"HTTP/1.1 ")?;
////    stream.write_all(response.status().as_str().as_bytes())?;
////    stream.write_all(b" ")?;
////    stream.write_all(
////        response
////            .status()
////            .canonical_reason()
////            .unwrap_or("")
////            .as_bytes(),
////    )?;
////    stream.write_all(b"\r\n")?;
////    for (key, value) in response.headers() {
////        stream.write_all(key.as_str().as_bytes())?;
////        stream.write_all(b": ")?;
////        stream.write_all(value.as_bytes())?;
////        stream.write_all(b"\r\n")?;
////    }
////    stream.write_all(b"\r\n")?;
////    stream.write_all(response.body().as_ref())?;
////    Ok(())
////}
//async fn write_respond<T: AsRef<[u8]>>(
//    mut stream: impl Write,
//    response: Response<T>,
//) -> Result<()> {
//    try_async(|| stream.write_all(b"HTTP/1.1 ")).await?;
//    try_async(|| stream.write_all(response.status().as_str().as_bytes())).await?;
//    try_async(|| stream.write_all(b" ")).await?;
//    try_async(|| {
//        stream.write_all(
//            response
//                .status()
//                .canonical_reason()
//                .unwrap_or("")
//                .as_bytes(),
//        )
//    })
//    .await?;
//    try_async(|| stream.write_all(b"\r\n")).await?;
//    for (key, value) in response.headers() {
//        try_async(|| stream.write_all(key.as_str().as_bytes())).await?;
//        try_async(|| stream.write_all(b": ")).await?;
//        try_async(|| stream.write_all(value.as_bytes())).await?;
//        try_async(|| stream.write_all(b"\r\n")).await?;
//    }
//    try_async(|| stream.write_all(b"\r\n")).await?;
//    try_async(|| stream.write_all(response.body().as_ref())).await?;
//    Ok(())
//}
//fn try_parse_request(buf: &[u8]) -> Result<Option<(http::Request<()>, usize)>> {
//    let mut headers = [httparse::EMPTY_HEADER; 15];
//    let mut req = httparse::Request::new(&mut headers);
//    match req.parse(buf)? {
//        Status::Partial => Ok(None),
//        Status::Complete(len) => {
//            let mut headers = http::HeaderMap::new();
//            for header in req.headers {
//                headers.append(
//                    HeaderName::from_bytes(header.name.as_bytes())?,
//                    HeaderValue::from_bytes(header.value)?,
//                );
//            }
//            let mut request = http::Request::new(());
//            *request.method_mut() = http::Method::from_bytes(req.method.unwrap_or("").as_bytes())?;
//            *request.headers_mut() = headers;
//            *request.uri_mut() = req.path.unwrap().parse()?;
//            *request.version_mut() = http::Version::HTTP_11;
//
//            Ok(Some((request, len)))
//        }
//    }
//}
