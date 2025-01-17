use tracing::info;

use crate::prelude::*;

// 生成自定义CA证书
pub async  fn generate_ca_certificate() -> Result<CertifiedKey, anyhow::Error> {
    // // 证书已存在并加载
    if std::path::Path::new("ca.crt").exists() && std::path::Path::new("ca.key").exists() {
        info!("[+] Load existing CA certificate and key");
        let cert_pem = std::fs::read_to_string("ca.crt").expect("[-] Failed read ca.crt");
        let key_pem = std::fs::read_to_string("ca.key").expect("[-] Failed read ca.key");
        let params = CertificateParams::from_ca_cert_pem(&cert_pem)?;
        
        let key_pair = KeyPair::from_pem(&key_pem).expect("[-] Failed to parse ca.key");
        let cert = params.self_signed(&key_pair)?;
        return Ok(CertifiedKey { cert, key_pair });
    }
    let ca_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let mut params = CertificateParams::new(vec!["GT TRAV CA".to_string()])?;
    // 设置 CA 证书的关键属性
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    // 设置证书有效期（例如：10年）
    params.not_before = std::time::SystemTime::now().into();
    params.not_after = (std::time::SystemTime::now() + std::time::Duration::from_secs(3650 * 24 * 60 * 60)).into();
    
    // 设置证书信息
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::OrganizationName, "GT TRAV CA");
    params.distinguished_name.push(DnType::CommonName, "GT TRAV CA");
    params.distinguished_name.push(DnType::CountryName, "CN");  // 可选：添加国家代码
    
    let ca_cert = params.self_signed(&ca_key_pair)?;
    // 保存证书和私钥
    std::fs::write("ca.crt", ca_cert.pem())?;
    std::fs::write("ca.key", ca_key_pair.serialize_pem())?;
    info!("[+] Generated new CA certificate and saved to ca.crt");
    info!("[+] Please install ca.crt in your browser/system");
    Ok(CertifiedKey { cert: ca_cert, key_pair: ca_key_pair })
}

// 生成自定义服务器证书
pub  async fn generate_signed_cert(ca_cert: &Certificate, ca_key: &KeyPair, host: String) -> Result<CertifiedKey, anyhow::Error> {

    let server_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let mut params = CertificateParams::new(vec![host.clone()])?;
    params.is_ca = rcgen::IsCa::ExplicitNoCa;
    // 设置证书有效期（例如：10年）
    params.not_before = std::time::SystemTime::now().into();
    params.not_after = (std::time::SystemTime::now() + std::time::Duration::from_secs(3650 * 24 * 60 * 60)).into();
    // 设置证书用途
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::DigitalSignature,
        rcgen::KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![
        rcgen::ExtendedKeyUsagePurpose::ServerAuth,
        rcgen::ExtendedKeyUsagePurpose::ClientAuth,
    ];
    // 设置证书主体信息
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, host.clone());
    params.distinguished_name.push(DnType::OrganizationName, "GT TRAV Server");
    params.distinguished_name.push(DnType::CountryName, "CN");
    
    params.use_authority_key_identifier_extension = true; 
    let cert = params.signed_by(&server_key_pair, ca_cert, ca_key)?;
    Ok(CertifiedKey { cert, key_pair: server_key_pair })
}