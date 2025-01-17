use std::{net::SocketAddr, sync::mpsc::channel, thread::spawn};
use anyhow::Context;
use rustls::{client, ClientConfig};
use time::SystemTime;
use tokio::{io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt}, net::TcpStream, sync::Mutex};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use crate::prelude::*;


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
        client_stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.context("[-] Failed to write http/1.1 200."); 
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
        // 将客户端流升级为 TLS 流
        let  mut tls_stream = match tls_acceptor.accept(&mut *client_stream).await{
            Ok(stream) =>  {stream},
            Err(e) => {
                eprintln!("[-] TLS handshake failed: {:?}", e);
                return Ok(());
            }
        };
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

                    
                    let mut total_request_bytes = 0;
                    let mut total_response_bytes = 0;
                    let mut buffer_req = vec![0u8; 8192];
                    let mut buffer_resp = vec![0u8; 8192];
                    loop {
                        // 从客户端读取数据
                        match tls_stream.read(&mut buffer_req).await {
                            Ok(0) => {
                                // 客户端关闭连接
                                println!("[INFO] Client disconnected after sending all request data.");
                                drop(buffer_req);
                                break;
                            }
                            Ok(n) => {
                                // 更新请求总字节数
                                total_request_bytes += n;
                    
                                // 打印读取的请求数据
                                let raw_data = String::from_utf8((&buffer_req[..n]).to_vec()).context("[-] connect recv data bytes failed to string.")?;
                                let res = Request::from_string(raw_data.as_str()).unwrap();
                                self.request = res;
                    
                                // 将数据写入目标服务器
                                if let Err(e) = target_tls_stream.write_all(&buffer_req[..n]).await {
                                    eprintln!("[-] Failed to write to target server: {:?}", e);
                                    break;
                                }
                                
                                println!("[INFO] Sent {} bytes to target server.", n);
                            }
                            Err(e) => {
                                eprintln!("[-] Error reading from client: {:?}", e);
                                break;
                            }
                        }
                    
                        // 从目标服务器读取响应
                        match target_tls_stream.read(&mut buffer_resp).await {
                            Ok(0) => {
                                // 目标服务器关闭连接
                                println!("[INFO] Target server disconnected after sending {} bytes.", total_response_bytes);
                                drop(buffer_resp);
                                break;
                            }
                            Ok(n) => {
                                // 更新响应总字节数
                                total_response_bytes += n;
                    
                                // 打印接收的响应数据
                                let raw_data = String::from_utf8((&buffer_resp[..n]).to_vec()).context("[-] connect recv data bytes failed to string.")?;
                                let resp = Response::from_string(raw_data.as_str())?;
                                self.response = resp;
                                // 将响应写回客户端
                                if let Err(e) = tls_stream.write_all(&buffer_resp[..n]).await {
                                    eprintln!("[-] Failed to write to client: {:?}", e);
                                    break;
                                }
                                println!("[INFO] Sent {} bytes back to client.", n);
                                //drop(buffer_resp);
                            }
                            Err(e) => {
                                eprintln!("[-] Error reading from target: {:?}", e);
                                break;
                            }
                        }
                    }
                    
                    // 打印请求完成的总信息
                    println!("[INFO] Request completed. Total request bytes: {}, Total response bytes: {}\n", total_request_bytes, total_response_bytes);



                    // match copy_bidirectional(&mut tls_stream, &mut target_tls_stream).await {
                    //     Ok((from_client, from_server)) => {

                    //         println!("  [HANDLE {}] Connection closed: {} bytes from client, {} bytes from server", self.session_id,from_client, from_server);
                    //     }
                    //     Err(e) => {
                    //         eprintln!(" [-] Error during bidirectional copy: {:?}", e);
                    //     }
                    // }
                },
                Err(e) => {
                    eprintln!("[-] Connection error: {:?}", e);
                }
            }
        Ok(())

    }
}
