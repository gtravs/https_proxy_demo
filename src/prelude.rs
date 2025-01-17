pub use std::{io::{Read, Write}, net::TcpListener, sync::Arc, time};
pub use rcgen::{Certificate, CertificateParams, DistinguishedName,CertifiedKey, DnType, KeyPair, SerialNumber, SignatureAlgorithm, PKCS_ECDSA_P256_SHA256, PKCS_RSA_SHA256};
pub use rustls::{pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, ServerName}, Stream};
pub use crate::ca_cert::*;
pub use tokio::{sync::Semaphore, task::JoinSet};
pub use std::result::Result::Ok;
pub const MAX_CONCURRENT_REQUESTS: usize = 100;

#[derive(Debug,Clone,PartialEq,Eq,Hash,Default)]
pub enum Method {
    #[default] GET,
     POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    CONNECT,
    PATCH,
    TRACE
}

impl Method {
    /// 将字符串转换为`Method`枚举
    pub fn from_str(method:&str) -> Option<Self> {
        match method.to_uppercase().as_str() {
            "GET" => Some(Self::GET),
            "POST" => Some(Self::POST),
            "PUT" => Some(Self::PUT),
            "DELETE" => Some(Self::DELETE),
            "HEAD" => Some(Self::HEAD),
            "OPTIONS" => Some(Self::OPTIONS),
            "CONNECT" => Some(Self::CONNECT),
            "PATCH" => Some(Self::PATCH),
            "TRACE" => Some(Self::TRACE),
            _ => None
        }
    }
    /// 将`Method`枚举转换对应字符串
    pub fn as_str(&self) -> &str {
        match self {
            Self::GET => "GET", 
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::DELETE => "DELETE",
            Self::HEAD => "HEAD",
            Self::OPTIONS => "OPTIONS",
            Self::CONNECT => "CONNECT",
            Self::PATCH => "PATCH",
            Self::TRACE => "TRACE",
        }
    }
}

#[derive(Clone,Debug,Default)]
pub struct Request {
    pub method : Method,
    pub url : String,
    pub http_version:String,
    pub headers:Vec<String>,
    pub body: Vec<String>
}

impl Request {

    // 请求数据转换解析
    pub fn from_string(raw_data:&str) -> Option<Self> {
        let mut  lines = raw_data.lines();
        let first_line = lines.next().unwrap_or("NULL");
        if first_line.is_empty() {
           return None; 
        }
        let split_first = first_line.split(" ").collect::<Vec<&str>>();
        let method = split_first[0];
        if method == Method::CONNECT.as_str() {
            //println!("    [DEBUG] Tcp Connect Parse.");
            let headers_vec:Vec<String> = lines.map(|line|line.to_string()).collect();
            let pos = headers_vec.iter().position(|line|line.is_empty()).unwrap();
            let headers = headers_vec[..pos].to_vec();
            Some(
                Request{
                    method :Method::from_str(method).unwrap(),
                    url: split_first[1].to_string(),
                    http_version:split_first[2].to_string(),
                    headers,
                    body:Vec::new()
                }
            )            
        } else {
            //println!("     [DEBUG] HTTP DATA Parse.");
            let next_lines = lines.map(|line|line.to_string()).collect::<Vec<String>>();
            //println!("     [DEBUG] HTTP DATA {next_lines:?}");   
            let pos = next_lines.iter().position(|line|line.is_empty()).unwrap();
            if (pos == next_lines.len()-1) {
                println!("     [DEBUG] None Body.");
                let headers = next_lines[..pos].to_vec();
                let body  = Vec::new();
                Some(
                Request{
                    method: Method::from_str(method).unwrap(),
                    url: split_first[1].to_string(),
                    http_version: split_first[2].to_string(),
                    headers,
                    body
                }
                )

            } else {
                let headers = next_lines[..pos].to_vec();
                let body  = next_lines[pos+1..].to_vec();
                Some(
                Request{
                    method: Method::from_str(method).unwrap(),
                    url: split_first[1].to_string(),
                    http_version: split_first[2].to_string(),
                    headers,
                    body
                }
                )
            }
        } 
    }
}

#[derive(Debug, Clone, Default)]
pub struct Response {
    pub status_code: u16,
    pub message: String,
    pub data: Option<String>,
    pub error: Option<String>,
}

impl Response {
    /// 从HTTP响应字符串解析出Response
    pub fn from_string(raw_response: &str) -> Result<Self, anyhow::Error> {
        let mut response = Response::default();

        // 将原始响应分为行，第一行通常是状态行
        let mut lines = raw_response.lines();
        if let Some(status_line) = lines.next() {
            let parts: Vec<&str> = status_line.split_whitespace().collect();
            if parts.len() >= 2 {
                // 提取状态码
                response.status_code = parts[1].parse::<u16>()
                    .map_err(|_| anyhow::anyhow!("Invalid status code in response"))?;
                // 提取消息
                if parts.len() > 2 {
                    response.message = parts[2..].join(" ");
                }
            } else {
                return Err(anyhow::anyhow!("Invalid status line in response"));
            }
        }

        // 合并剩余行作为数据
        let body: Vec<&str> = lines.collect();
        let body_text = body.join("\r\n");

        // 如果内容非空，填充到 data 字段
        if !body_text.is_empty() {
            response.data = Some(body_text);
        }

        // 如果状态码表示错误，填充到 error 字段
        if response.status_code >= 400 {
            response.error = Some(response.message.clone());
        }

        Ok(response)
    }
}


