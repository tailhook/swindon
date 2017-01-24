use std::str::from_utf8;
use std::sync::Arc;
use std::io::BufWriter;
use std::mem;

use futures::Async;
use futures::future::{FutureResult, ok};
use minihttp::{Status, Version};
use minihttp::client as http;
use tokio_core::io::Io;
use rustc_serialize::Encodable;
use rustc_serialize::json::{as_json, Json};

use proxy::{Response};
use chat::{Cid, ConnectionSender};
use chat::cid::{serialize_cid};
use chat::message::{AuthData, Auth, Meta, Args, Kwargs};
use chat::processor::{ProcessorPool, Action};
use chat::authorize::{parse_userinfo, good_status};
use chat::ConnectionMessage::{Hello, StopSocket};
use chat::CloseReason::{AuthHttp};

enum AuthState {
    Init(String, AuthData),
    Wait,
    Headers(Status),
    Done(Response),
    Void,
}

enum CallState {
    Init {
        cid: Cid, path: String, meta: Meta,
        args: Args, kw: Kwargs
    },
}

pub struct AuthCodec {
    state: AuthState,
    chat: ProcessorPool,
    conn_id: Cid,
    sender: ConnectionSender,
}

pub struct CallCodec {
    state: CallState,
    sender: ConnectionSender,
}

impl AuthCodec {
    pub fn new(path: String, cid: Cid, req: AuthData, chat: ProcessorPool,
        tx: ConnectionSender)
        -> AuthCodec
    {
        AuthCodec {
            state: AuthState::Init(path, req),
            chat: chat,
            conn_id: cid,
            sender: tx,
        }
    }
}

impl CallCodec {
    pub fn new(cid: Cid, path: String, meta: Meta, args: Args, kw: Kwargs,
        sender: ConnectionSender)
        -> CallCodec
    {
        CallCodec {
            state: CallState::Init {
                cid: cid,
                path: path,
                meta: meta,
                args: args,
                kw: kw,
            },
            sender: sender,
        }
    }
}

fn write_json_request<S: Io, E>(mut e: http::Encoder<S>, data: &E)
    -> http::EncoderDone<S>
    where E: Encodable,
{
    use std::io::Write;
    e.add_header("Content-Type", "application/json").unwrap();
    e.add_chunked().unwrap();
    e.done_headers().unwrap();
    let mut buf = BufWriter::new(e);
    write!(&mut buf, "{}", as_json(data)).unwrap();
    match buf.into_inner() {
        Ok(x) => x.done(),
        Err(_) => unreachable!(),
    }
}


impl<S: Io> http::Codec<S> for AuthCodec {
    type Future = FutureResult<http::EncoderDone<S>, http::Error>;

    fn start_write(&mut self, mut e: http::Encoder<S>) -> Self::Future {
        use self::AuthState::*;
        if let Init(p, i) = mem::replace(&mut self.state, Void)
        {
            self.state = Wait;
            e.request_line("POST", &p, Version::Http11);
            ok(write_json_request(e,
                &Auth(&serialize_cid(&self.conn_id), &i)))
        } else {
            panic!("wrong state");
        }
    }
    fn headers_received(&mut self, headers: &http::Head)
        -> Result<http::RecvMode, http::Error>
    {
        use self::AuthState::*;
        if let Wait = mem::replace(&mut self.state, Void) {
            self.state = Headers(
                headers.status().unwrap_or(Status::InternalServerError));
            // TODO(tailhook) limit and streaming
            Ok(http::RecvMode::Buffered(10_485_760))
        } else {
            panic!("wrong state");
        }
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, http::Error>
    {
        use self::AuthState::*;
        // TODO(tailhook) streaming
        assert!(end);
        match mem::replace(&mut self.state, Void) {
            Headers(Status::Ok) => {
                let result = from_utf8(data)
                    .map_err(|e| debug!("Invalid utf-8 in auth data: {}", e))
                .and_then(|s| Json::from_str(s)
                    .map_err(|e| debug!("Invalid json in auth data: {}", e)))
                .and_then(|j| parse_userinfo(j)
                    .map_err(|e| debug!("Bad user info in auth data: {}", e)));
                match result {
                    Ok((sess_id, userinfo)) => {
                        let userinfo = Arc::new(userinfo);
                        debug!("Auth data received {:?}: {:?}",
                            sess_id, userinfo);
                        self.chat.send(Action::Associate {
                            conn_id: self.conn_id,
                            session_id: sess_id,
                            metadata: userinfo.clone(),
                        });
                        self.sender.send(Hello(userinfo));
                    }
                    Err(()) => {
                        debug!("Auth error");
                        self.sender.send(StopSocket(
                            AuthHttp(Status::InternalServerError)));
                    }
                };
            }
            Headers(status) => {
                if good_status(status) {
                    self.sender.send(StopSocket(AuthHttp(status)));
                } else {
                    self.sender.send(StopSocket(
                        AuthHttp(Status::InternalServerError)));
                }
            }
            _ => unreachable!(),
        }
        Ok((Async::Ready(data.len())))
    }
}

impl<S: Io> http::Codec<S> for CallCodec {
    type Future = FutureResult<http::EncoderDone<S>, http::Error>;

    fn start_write(&mut self, mut e: http::Encoder<S>) -> Self::Future {
        use self::AuthState::*;
        unimplemented!();
    }
    fn headers_received(&mut self, headers: &http::Head)
        -> Result<http::RecvMode, http::Error>
    {
        use self::AuthState::*;
        unimplemented!();
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, http::Error>
    {
        use self::AuthState::*;
        unimplemented!();
    }
}
