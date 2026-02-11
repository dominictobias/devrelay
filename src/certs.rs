use anyhow::{Context, Result};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair,
};
use std::fs;
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

pub struct CertManager {
    cert_dir: PathBuf,
    ca_name: String,
}

impl CertManager {
    pub fn new(cert_dir: impl AsRef<Path>, ca_name: String) -> Self {
        Self {
            cert_dir: cert_dir.as_ref().to_path_buf(),
            ca_name,
        }
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.cert_dir)
            .with_context(|| format!("Failed to create cert directory: {}", self.cert_dir.display()))?;

        // Generate CA if it doesn't exist
        if !self.ca_cert_path().exists() {
            println!("Generating new Certificate Authority...");
            self.generate_ca()?;
            println!("✓ CA certificate generated at: {}", self.ca_cert_path().display());
            println!("\nTo trust this CA on macOS, run:");
            println!("  sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {}", self.ca_cert_path().display());
        }

        Ok(())
    }

    fn generate_ca(&self) -> Result<()> {
        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, &self.ca_name);
        dn.push(DnType::OrganizationName, "DevRelay");
        params.distinguished_name = dn;

        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc() + Duration::days(365 * 10); // 10 years

        let ca_key_pair = KeyPair::generate()?;
        let ca_cert = params.self_signed(&ca_key_pair)
            .context("Failed to generate CA certificate")?;

        let ca_cert_pem = ca_cert.pem();
        let ca_key_pem = ca_key_pair.serialize_pem();

        fs::write(self.ca_cert_path(), ca_cert_pem)
            .context("Failed to write CA certificate")?;
        fs::write(self.ca_key_path(), ca_key_pem)
            .context("Failed to write CA key")?;

        Ok(())
    }

    pub fn generate_server_cert(&self, domain: &str) -> Result<()> {
        let cert_path = self.server_cert_path(domain);
        let key_path = self.server_key_path(domain);

        if cert_path.exists() && key_path.exists() {
            return Ok(()); // Already exists
        }

        println!("Generating server certificate for: {}", domain);

        // Load CA key
        let ca_key_pem = fs::read_to_string(self.ca_key_path())
            .context("Failed to read CA key")?;

        let ca_key_pair = KeyPair::from_pem(&ca_key_pem)
            .context("Failed to parse CA key")?;

        // Reconstruct CA params (they need to match what was used to create the CA)
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, &self.ca_name);
        dn.push(DnType::OrganizationName, "DevRelay");
        ca_params.distinguished_name = dn;

        // Create CA certificate object for signing
        let ca_cert = ca_params.self_signed(&ca_key_pair)
            .context("Failed to reconstruct CA certificate")?;

        // Create server cert
        let mut params = CertificateParams::default();
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName(rcgen::Ia5String::try_from(domain.to_string())
                .context("Invalid domain name")?),
        ];

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, domain);
        params.distinguished_name = dn;

        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc() + Duration::days(365); // 1 year

        let server_key_pair = KeyPair::generate()?;
        let server_cert = params.signed_by(&server_key_pair, &ca_cert, &ca_key_pair)
            .context("Failed to generate server certificate")?;

        let server_cert_pem = server_cert.pem();
        let server_key_pem = server_key_pair.serialize_pem();

        fs::write(&cert_path, server_cert_pem)
            .context("Failed to write server certificate")?;
        fs::write(&key_path, server_key_pem)
            .context("Failed to write server key")?;

        println!("✓ Certificate generated for: {}", domain);

        Ok(())
    }

    /// Generate a single server certificate covering all given domains as SANs.
    /// Used for TLS listeners that may serve multiple domains on the same port.
    /// Always regenerated on startup to pick up config changes.
    pub fn generate_combined_server_cert(&self, domains: &[String]) -> Result<()> {
        let cert_path = self.combined_cert_path();
        let key_path = self.combined_key_path();

        println!("Generating combined server certificate for TLS listeners...");

        // Load CA key
        let ca_key_pem = fs::read_to_string(self.ca_key_path())
            .context("Failed to read CA key")?;
        let ca_key_pair = KeyPair::from_pem(&ca_key_pem)
            .context("Failed to parse CA key")?;

        // Reconstruct CA for signing
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, &self.ca_name);
        dn.push(DnType::OrganizationName, "DevRelay");
        ca_params.distinguished_name = dn;
        let ca_cert = ca_params.self_signed(&ca_key_pair)
            .context("Failed to reconstruct CA certificate")?;

        // Create server cert with all domains as SANs
        let mut params = CertificateParams::default();
        params.subject_alt_names = domains
            .iter()
            .map(|d| {
                Ok(rcgen::SanType::DnsName(
                    rcgen::Ia5String::try_from(d.clone())
                        .map_err(|_| anyhow::anyhow!("Invalid domain name: {}", d))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "DevRelay Server");
        params.distinguished_name = dn;

        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc() + Duration::days(365);

        let server_key_pair = KeyPair::generate()?;
        let server_cert = params.signed_by(&server_key_pair, &ca_cert, &ca_key_pair)
            .context("Failed to generate combined server certificate")?;

        fs::write(&cert_path, server_cert.pem())
            .context("Failed to write combined server certificate")?;
        fs::write(&key_path, server_key_pair.serialize_pem())
            .context("Failed to write combined server key")?;

        for domain in domains {
            println!("  ✓ {}", domain);
        }

        Ok(())
    }

    pub fn combined_cert_path(&self) -> PathBuf {
        self.cert_dir.join("server.crt")
    }

    pub fn combined_key_path(&self) -> PathBuf {
        self.cert_dir.join("server.key")
    }

    pub fn ca_cert_path(&self) -> PathBuf {
        self.cert_dir.join("ca.crt")
    }

    fn ca_key_path(&self) -> PathBuf {
        self.cert_dir.join("ca.key")
    }

    pub fn server_cert_path(&self, domain: &str) -> PathBuf {
        self.cert_dir.join(format!("{}.crt", domain))
    }

    pub fn server_key_path(&self, domain: &str) -> PathBuf {
        self.cert_dir.join(format!("{}.key", domain))
    }
}
