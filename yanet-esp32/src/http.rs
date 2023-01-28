use anyhow::Result;
use http::header::HeaderName;
use http::{HeaderValue, Method, Response};
use httparse::Status;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::wifi::WifiService;

pub async fn try_async<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match f() {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                futures_timer::Delay::new(Duration::from_millis(100)).await;
            }
            res => return res,
        }
    }
}

pub struct HttpServe {
    listener: TcpListener,
}

impl HttpServe {
    pub fn new() -> Result<Self> {
        let listener = std::net::TcpListener::bind("0.0.0.0:80")?;
        Ok(Self { listener })
    }
    pub async fn run<'a>(&self, wifi: &WifiService<'a>) {
        self.listener.set_nonblocking(true).unwrap();
        while let Ok((stream, _addr)) = try_async(|| self.listener.accept()).await {
            stream.set_nonblocking(true).ok();
            Self::handle_stream(stream, wifi).await.ok();
        }
    }

    async fn handle_stream<'a>(mut stream: TcpStream, wifi: &WifiService<'a>) -> Result<()> {
        let mut buf = [0u8; 1024];
        let mut start = 0;
        let req = loop {
            let len = try_async(|| stream.read(&mut buf[start..])).await?;
            start += len;
            if let Some((req, size)) = try_parse_request(&buf[..start])? {
                if size < start {
                    buf.copy_within(size..start, 0);
                    start -= size;
                }
                break req;
            }
        };
        #[derive(Serialize, Deserialize)]
        struct Wifi {
            ssid: String,
            pass: String,
        }
        match (req.method().clone(), req.uri().path()) {
            (Method::GET, "/") => {
                let res = Response::new(include_str!("index.html"));
                write_respond(stream, res).await?;
            }
            (Method::POST, "/wifi") => {
                let len = if let Some(value) = req.headers().get("Content-Length") {
                    let len = value.to_str()?.parse()?;
                    if len > start {
                        stream.read_exact(&mut buf[start..][..len - start])?;
                    }
                    len
                } else {
                    start
                };

                let Wifi { ssid, pass } = serde_qs::from_bytes(&buf[..len])?;
                wifi.connect(&ssid, &pass).await?;
                let res = Response::new(b"");
                write_respond(stream, res).await?;
            }
            _ => (),
        }

        Ok(())
    }
}
async fn write_respond<T: AsRef<[u8]>>(
    mut stream: impl Write,
    response: Response<T>,
) -> Result<()> {
    try_async(|| stream.write_all(b"HTTP/1.1 ")).await?;
    try_async(|| stream.write_all(response.status().as_str().as_bytes())).await?;
    try_async(|| stream.write_all(b" ")).await?;
    try_async(|| {
        stream.write_all(
            response
                .status()
                .canonical_reason()
                .unwrap_or("")
                .as_bytes(),
        )
    })
    .await?;
    try_async(|| stream.write_all(b"\r\n")).await?;
    for (key, value) in response.headers() {
        try_async(|| stream.write_all(key.as_str().as_bytes())).await?;
        try_async(|| stream.write_all(b": ")).await?;
        try_async(|| stream.write_all(value.as_bytes())).await?;
        try_async(|| stream.write_all(b"\r\n")).await?;
    }
    try_async(|| stream.write_all(b"\r\n")).await?;
    try_async(|| stream.write_all(response.body().as_ref())).await?;
    Ok(())
}
fn try_parse_request(buf: &[u8]) -> Result<Option<(http::Request<()>, usize)>> {
    let mut headers = [httparse::EMPTY_HEADER; 15];
    let mut req = httparse::Request::new(&mut headers);
    match req.parse(buf)? {
        Status::Partial => Ok(None),
        Status::Complete(len) => {
            let mut headers = http::HeaderMap::new();
            for header in req.headers {
                headers.append(
                    HeaderName::from_bytes(header.name.as_bytes())?,
                    HeaderValue::from_bytes(header.value)?,
                );
            }
            let mut request = http::Request::new(());
            *request.method_mut() = http::Method::from_bytes(req.method.unwrap_or("").as_bytes())?;
            *request.headers_mut() = headers;
            *request.uri_mut() = req.path.unwrap().parse()?;
            *request.version_mut() = http::Version::HTTP_11;

            Ok(Some((request, len)))
        }
    }
}
