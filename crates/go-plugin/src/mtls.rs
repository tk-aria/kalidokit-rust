//! Automatic mutual TLS for plugin communication.
//!
//! When AutoMTLS is enabled, the host generates an ephemeral ECDSA CA and
//! client certificate. The CA cert is passed to the plugin via the
//! `PLUGIN_CLIENT_CERT` environment variable. The plugin generates its own
//! server certificate and sends it back in the negotiation line.
//!
//! This ensures only the original host process can connect to the plugin.

use crate::error::PluginError;
use base64::Engine;
use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P256_SHA256};
use std::sync::Arc;

/// Generated mTLS certificates for a plugin session.
#[derive(Clone)]
pub struct MtlsConfig {
    /// DER-encoded CA certificate.
    pub ca_cert_der: Vec<u8>,
    /// PEM-encoded CA certificate.
    pub ca_cert_pem: String,
    /// DER-encoded client certificate.
    pub client_cert_der: Vec<u8>,
    /// PEM-encoded client certificate.
    pub client_cert_pem: String,
    /// PEM-encoded client private key.
    pub client_key_pem: String,
}

impl MtlsConfig {
    /// Generate a new ephemeral CA + client certificate pair.
    ///
    /// The CA is self-signed with ECDSA P-256. The client certificate is
    /// signed by the CA with both ClientAuth and ServerAuth extended key usage.
    pub fn generate() -> Result<Self, PluginError> {
        // Generate CA key pair
        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| PluginError::Tls(format!("CA key generation failed: {e}")))?;

        let mut ca_params = CertificateParams::new(vec!["localhost".to_string()])
            .map_err(|e| PluginError::Tls(format!("CA params failed: {e}")))?;
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];

        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| PluginError::Tls(format!("CA cert self-sign failed: {e}")))?;

        // Generate client key pair
        let client_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| PluginError::Tls(format!("client key generation failed: {e}")))?;

        let mut client_params = CertificateParams::new(vec!["localhost".to_string()])
            .map_err(|e| PluginError::Tls(format!("client params failed: {e}")))?;
        client_params.is_ca = rcgen::IsCa::NoCa;
        client_params.extended_key_usages = vec![
            rcgen::ExtendedKeyUsagePurpose::ClientAuth,
            rcgen::ExtendedKeyUsagePurpose::ServerAuth,
        ];

        let client_cert = client_params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .map_err(|e| PluginError::Tls(format!("client cert signing failed: {e}")))?;

        Ok(Self {
            ca_cert_der: ca_cert.der().to_vec(),
            ca_cert_pem: ca_cert.pem(),
            client_cert_der: client_cert.der().to_vec(),
            client_cert_pem: client_cert.pem(),
            client_key_pem: client_key.serialize_pem(),
        })
    }

    /// Base64-encode the CA certificate DER for the `PLUGIN_CLIENT_CERT` env var.
    pub fn ca_cert_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.ca_cert_der)
    }

    /// Build a rustls `ClientConfig` for the host to connect to the plugin.
    ///
    /// The host presents the client cert and trusts only the server cert
    /// from the negotiation line.
    pub fn client_tls_config(
        &self,
        server_cert_der: &[u8],
    ) -> Result<rustls::ClientConfig, PluginError> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store
            .add(rustls::pki_types::CertificateDer::from(
                server_cert_der.to_vec(),
            ))
            .map_err(|e| PluginError::Tls(format!("add server cert to root store: {e}")))?;

        let client_cert = rustls::pki_types::CertificateDer::from(self.client_cert_der.clone());
        let client_key = rustls::pki_types::PrivateKeyDer::try_from(
            self.client_key_pem.as_bytes().to_vec(),
        )
        .or_else(|_| {
            // Try parsing as PEM
            let mut reader = std::io::BufReader::new(self.client_key_pem.as_bytes());
            let keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| PluginError::Tls(format!("parse client key PEM: {e}")))?;
            keys.into_iter()
                .next()
                .map(|k| rustls::pki_types::PrivateKeyDer::Pkcs8(k))
                .ok_or_else(|| PluginError::Tls("no private key found in PEM".into()))
        })?;

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(vec![client_cert], client_key)
            .map_err(|e| PluginError::Tls(format!("build client TLS config: {e}")))?;

        Ok(config)
    }

    /// Build a rustls `ServerConfig` for the plugin server.
    ///
    /// The plugin trusts only the CA cert from the host and requires client auth.
    pub fn server_tls_config(
        ca_cert_der: &[u8],
        server_cert_pem: &str,
        server_key_pem: &str,
    ) -> Result<rustls::ServerConfig, PluginError> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store
            .add(rustls::pki_types::CertificateDer::from(
                ca_cert_der.to_vec(),
            ))
            .map_err(|e| PluginError::Tls(format!("add CA cert to root store: {e}")))?;

        let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
            .build()
            .map_err(|e| PluginError::Tls(format!("build client verifier: {e}")))?;

        let server_cert = {
            let mut reader = std::io::BufReader::new(server_cert_pem.as_bytes());
            rustls_pemfile::certs(&mut reader)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| PluginError::Tls(format!("parse server cert PEM: {e}")))?
        };

        let server_key = {
            let mut reader = std::io::BufReader::new(server_key_pem.as_bytes());
            let keys: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut reader)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| PluginError::Tls(format!("parse server key PEM: {e}")))?;
            keys.into_iter()
                .next()
                .map(|k| rustls::pki_types::PrivateKeyDer::Pkcs8(k))
                .ok_or_else(|| PluginError::Tls("no server key found in PEM".into()))?
        };

        let config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(server_cert, server_key)
            .map_err(|e| PluginError::Tls(format!("build server TLS config: {e}")))?;

        Ok(config)
    }
}

/// Generate an ephemeral server certificate for the plugin side.
///
/// The plugin reads the CA cert from `PLUGIN_CLIENT_CERT` env var,
/// generates a server cert, and returns both the cert and key PEM
/// along with the DER for the negotiation line.
pub fn generate_server_cert() -> Result<ServerCert, PluginError> {
    let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .map_err(|e| PluginError::Tls(format!("server key generation failed: {e}")))?;

    let mut params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| PluginError::Tls(format!("server cert params failed: {e}")))?;
    params.is_ca = rcgen::IsCa::NoCa;
    params.extended_key_usages = vec![
        rcgen::ExtendedKeyUsagePurpose::ServerAuth,
        rcgen::ExtendedKeyUsagePurpose::ClientAuth,
    ];

    // Self-signed for now -- the plugin creates its own trust relationship
    let cert = params
        .self_signed(&key)
        .map_err(|e| PluginError::Tls(format!("server cert self-sign failed: {e}")))?;

    Ok(ServerCert {
        cert_der: cert.der().to_vec(),
        cert_pem: cert.pem(),
        key_pem: key.serialize_pem(),
    })
}

/// Server certificate + key for the plugin process.
pub struct ServerCert {
    /// DER-encoded certificate (for the negotiation line).
    pub cert_der: Vec<u8>,
    /// PEM-encoded certificate.
    pub cert_pem: String,
    /// PEM-encoded private key.
    pub key_pem: String,
}

impl ServerCert {
    /// Base64-encode the certificate DER for the negotiation line.
    pub fn cert_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.cert_der)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_mtls_config() {
        let config = MtlsConfig::generate().unwrap();
        assert!(!config.ca_cert_der.is_empty());
        assert!(!config.client_cert_der.is_empty());
        assert!(config.ca_cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(config.client_cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(config.client_key_pem.contains("BEGIN PRIVATE KEY"));
    }

    #[test]
    fn ca_cert_base64_roundtrip() {
        let config = MtlsConfig::generate().unwrap();
        let b64 = config.ca_cert_base64();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .unwrap();
        assert_eq!(decoded, config.ca_cert_der);
    }

    #[test]
    fn generate_server_cert_works() {
        let cert = generate_server_cert().unwrap();
        assert!(!cert.cert_der.is_empty());
        assert!(cert.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(cert.key_pem.contains("BEGIN PRIVATE KEY"));
        assert!(!cert.cert_base64().is_empty());
    }
}
