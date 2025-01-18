#![allow(unused)]
pub const DEFAULT_BUF_SIZE: usize = 8192;

use std::thread::JoinHandle;
use anyhow::Context;
use prelude::*;
use session::{Session};
use time::Duration;
use tokio::{io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt}, net::TcpStream, sync::Mutex, time::sleep};
use tokio_rustls::{TlsAcceptor, TlsConnector};

mod prelude;
mod ca_cert;
mod session;
mod debug_stream;
mod copy;
// set_proxy_port
async fn set_proxy_port(host: String, port: u32) -> Result<tokio::net::TcpListener, anyhow::Error> {
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("[+] Listening on {}\n", &*addr);
    Ok(listener)
}

async fn entry() -> Result<(), anyhow::Error> {
    // test code
    let listener = set_proxy_port("127.0.0.1".to_string(), 9990).await.context("[-] Failed to set_proxy_port func error: bad listener.")?;
    let cacert = Arc::new(generate_ca_certificate().await.context("[-] Failed to generate ca certificate")?);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                    let task = tokio::spawn({
                        let uuid = uuid::Uuid::new_v4();
                        let session_id = u32::from_le_bytes(uuid.as_bytes()[0..4].try_into().unwrap());
                        //println!("[Session {}] => [", session_id);
                        let session = Arc::new(Mutex::new(Session::new(session_id, stream).unwrap()));
                        let ca_cert = Arc::clone(&cacert);
                        let session_clone = Arc::clone(&session);
                        async move {
                            let mut session_lock = session_clone.lock().await;
                            session_lock.session_connect(addr).await;
                            let method = session_lock.request.method.clone();
                            let url = session_lock.request.url.clone();
                            let initial_data = session_lock.initial_data.clone();
                            let header_host = session_lock.request.host.clone();
                            println!("[Session {}] Request Method: {:?}, URL: {}", session_id, method, url);
                            match method {
                                Method::CONNECT => {
                                    let url_split: Vec<&str> = url.split(":").collect();
                                    let host = url_split[0].to_string();
                                    let port = url_split[1].to_string();
                                    session_lock.handle_https(host, port, ca_cert).await;
                                }
                                Method::GET => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;

                                }
                                Method::POST => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                }
                                Method::PUT => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                },
                                Method::DELETE => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                },
                                Method::HEAD => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                },
                                Method::OPTIONS => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                },
                                Method::PATCH => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                },
                                Method::TRACE => {
                                    let (host, port) = match header_host.split(':').collect::<Vec<&str>>().as_slice() {
                                        // 如果只有主机名，没有端口号
                                        [host] => (host.to_string(), "80".to_string()),
                                        // 如果有主机名和端口号
                                        [host, port] => (host.to_string(), port.to_string()),
                                        // 处理其他异常情况
                                        _ => ("".to_string(), "80".to_string())
                                    };
                                    session_lock.handle_http(host, port, initial_data).await;
                                },
                            }
                            // After task completion, log session data
                            println!("[Session {}] => Session completed. Session data: {:?}", session_id, session_lock);
                    }});
                // Await the spawned task to ensure it completes before proceeding
                //task.await.context("[-] Failed to await session task.")?;
                
            }
            Err(e) => {
                eprintln!("[-] Failed to listener accept: {:?}", e);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use session::Session;

    use super::*;   
    // test模块测试
    #[tokio::test]
    async fn task_test_run() {
        let _ = entry().await.context("[-] Failed to entry.");
    }
}
