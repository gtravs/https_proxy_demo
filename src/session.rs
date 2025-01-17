use std::{future::ready, net::SocketAddr, sync::mpsc::channel, thread::spawn};
use anyhow::Context as ct;
use rustls::{client, ClientConfig};
use time::SystemTime;
use tokio::{io::{ AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt}, net::TcpStream, sync::Mutex};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use crate::{debug_stream::copy_bidirectional, prelude::*};
use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use crate::copy::{CopyBuffer, Direction};

enum TransferState {
    Running(CopyBuffer),
    ShuttingDown(u64),
    Done(u64),
}

#[derive(Debug,Clone)]
pub struct Session{
    //  请求体
    pub request : Request,
    //  响应体
    pub response: Response,
    // 其他
    pub session_id:u32,
    pub time: Option<SystemTime>,
    pub stream : Option<Arc<Mutex<TcpStream>>>
}

impl Session {
    pub fn new(session_id:u32,stream: TcpStream) ->  Option<Self>{

        Some(
            Session { 
                request: Request::default(), 
                response: Response::default(), 
                session_id, 
                time: Some(SystemTime::now()), 
                stream: Some(Arc::new(Mutex::new(stream)))
            }
        )
    }

    pub fn set_request(&mut self,req:Request) {
        self.request = req;
    }

    pub fn set_response(&mut self,resp:Response) { 
        self.response = resp;
    }

    // 新增方法以获取请求体和响应体的原始数据
    pub fn get_request_data(&self) -> String {
        String::from_utf8_lossy(&self.request.to_bytes()).to_string()
    }

    pub fn get_response_data(&self) -> String {
        String::from_utf8_lossy(&self.response.to_bytes()).to_string()
    }

    pub async  fn session_connect(&mut self,addr:SocketAddr) {
        //println!("[CONNECT SATRT {}]",self.session_id);
        //println!(" -> connection new {addr:?}");
        //let mut stream = self.stream.as_ref().unwrap().lock().await;
        let mut buffer = [0u8;8192]; 
        let n = self.stream.as_ref().unwrap().lock().await.read(&mut buffer[..]).await.unwrap();
        // println!(" -> connect recv data {n:?} bytes.");

        let raw_data = String::from_utf8(buffer[..n].to_vec()).context("[-] connect recv data bytes failed to string.");
        let res = Request::from_string(raw_data.unwrap().as_str()).unwrap();
        self.request = res;
    }



    pub async fn handle_https(&mut self,host:String,port:String,ca_cert: Arc<CertifiedKey>) -> Result<(), anyhow::Error> {
        let mut client_stream = self.stream.as_ref().unwrap().lock().await;
        client_stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.context("[-] Failed to write http/1.1 200.")?;
        // let mut client_stream = self.stream.as_ref().unwrap().lock().await;
        // client_stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.context("[-] Failed to write http/1.1 200."); 
        drop(client_stream);
        let server_cert = generate_signed_cert(&ca_cert.cert, &ca_cert.key_pair, host.clone()).await?;
        let  server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![server_cert.cert.into()],rustls::pki_types::PrivateKeyDer::Pkcs8(server_cert.key_pair.serialize_der().into()))?;
        let mut  root_store = rustls::RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS
                .iter()
                .cloned(),
        );
        let mut server  = rustls::ServerConnection::new(Arc::new(server_config.clone()))?;
        let tls_acceptor = TlsAcceptor::from(Arc::new(server_config.clone()));



        let cert_der = CertificateDer::from_pem_file("ca.crt").expect("[-] Failed to RootCertStore read ca.crt");
        let cert_der_iter = vec![cert_der];
        root_store.add_parsable_certificates(cert_der_iter);
            // 连接目标服务器
            match TcpStream::connect(format!("{}:{}", host, port)).await {
                Ok(target_stream) => {
                    // 配置 Rustls 客户端配置
                    let client_config = ClientConfig::builder()
                        .with_root_certificates(root_store)
                        .with_no_client_auth();
                    let tls_connector = TlsConnector::from(Arc::new(client_config));
                    // 构建服务器名称
                    let server_name = ServerName::try_from(host).context("Invalid server name")?;
                    // 将目标服务器流升级为 TLS 流
                    let mut target_tls_stream = match tls_connector.connect(server_name, target_stream).await {
                        Ok(stream) => stream,
                        Err(e) => {
                            eprintln!("[-] TLS handshake with target server failed: {:?}", e);
                            return Ok(());
                        }
                    };

                    let mut client_stream = self.stream.as_ref().unwrap().lock().await;
                    // 传递引用而非移动
                    let  mut tls_stream = match tls_acceptor.accept(&mut *client_stream).await {
                        Ok(stream) => stream,
                        Err(e) => {
                            eprintln!("[-] TLS handshake failed: {:?}", e);
                            return Ok(());
                        }
                    }; 


                    match copy_bidirectional(&mut tls_stream, &mut target_tls_stream).await {
                        Ok(((from_client_byte,from_client_data), (from_server_byte,from_server_data))) => {
                            let req = Request::from_string(&from_client_data).unwrap();
                            self.request = req;

                            let resp = Response::from_string(&from_server_data).unwrap();
                            self.response = resp;
                            //println!("  [HANDLE {}] Connection closed: {} \n{} bytes from client, {}\n{} bytes from server", self.session_id,from_client_data,from_client_byte, from_server_data,from_server_byte);
                        }
                        Err(e) => {
                            //eprintln!(" [-] Error during bidirectional copy: {:?}", e);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("[-] Connection error: {:?}", e);
                }
            }
        Ok(())

    }

    
}

