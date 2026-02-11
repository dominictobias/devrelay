use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct Installer;

impl Installer {
    /// Install CA certificate to macOS System Keychain
    /// This will prompt the user for their password via sudo
    pub fn install_ca_cert(cert_path: &Path) -> Result<bool> {
        if !cert_path.exists() {
            return Ok(false);
        }

        println!("🔐 Installing CA certificate to macOS Keychain...");
        println!("   This requires administrator privileges.");

        let output = Command::new("sudo")
            .arg("security")
            .arg("add-trusted-cert")
            .arg("-d")
            .arg("-r")
            .arg("trustRoot")
            .arg("-k")
            .arg("/Library/Keychains/System.keychain")
            .arg(cert_path)
            .output()
            .context("Failed to execute security command")?;

        if output.status.success() {
            println!("✅ CA certificate installed successfully!");
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check if cert is already installed
            if stderr.contains("The specified item already exists in the keychain") {
                println!("✅ CA certificate already installed");
                Ok(true)
            } else {
                eprintln!("❌ Failed to install CA certificate: {}", stderr);
                Ok(false)
            }
        }
    }

    /// Add domain entries to /etc/hosts
    /// This will prompt the user for their password via sudo
    pub fn install_hosts_entries(domains: &[String]) -> Result<bool> {
        if domains.is_empty() {
            return Ok(true);
        }

        println!("\n🌐 Updating /etc/hosts with domain entries...");
        println!("   This requires administrator privileges.");

        // Read current /etc/hosts
        let hosts_content = fs::read_to_string("/etc/hosts")
            .context("Failed to read /etc/hosts")?;

        // Check which domains are missing
        let mut missing_domains = Vec::new();
        for domain in domains {
            if !Self::is_domain_in_hosts(&hosts_content, domain) {
                missing_domains.push(domain.clone());
            }
        }

        if missing_domains.is_empty() {
            println!("✅ All domains already in /etc/hosts");
            return Ok(true);
        }

        // Create new entries
        let mut new_entries = String::from("\n# DevRelay entries\n");
        for domain in &missing_domains {
            new_entries.push_str(&format!("127.0.0.1 {}\n", domain));
        }

        // Append to /etc/hosts using sudo
        let output = Command::new("sudo")
            .arg("sh")
            .arg("-c")
            .arg(format!("echo '{}' >> /etc/hosts", new_entries.trim()))
            .output()
            .context("Failed to update /etc/hosts")?;

        if output.status.success() {
            println!("✅ Added {} domain(s) to /etc/hosts:", missing_domains.len());
            for domain in &missing_domains {
                println!("   • {}", domain);
            }
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("❌ Failed to update /etc/hosts: {}", stderr);
            Ok(false)
        }
    }

    /// Check if a domain is already in /etc/hosts pointing to 127.0.0.1
    fn is_domain_in_hosts(hosts_content: &str, domain: &str) -> bool {
        for line in hosts_content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Check if line contains "127.0.0.1" and the domain
            if line.starts_with("127.0.0.1") && line.contains(domain) {
                // Verify it's the exact domain, not a substring
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1..].contains(&domain) {
                    return true;
                }
            }
        }
        false
    }

    /// Remove CA certificate from macOS System Keychain
    /// This will prompt the user for their password via sudo
    pub fn uninstall_ca_cert(ca_name: &str) -> Result<bool> {
        println!("🔐 Removing CA certificate from macOS Keychain...");
        println!("   This requires administrator privileges.");

        // Find the certificate hash in the System Keychain
        let find_output = Command::new("security")
            .arg("find-certificate")
            .arg("-a")
            .arg("-c")
            .arg(ca_name)
            .arg("-Z")
            .arg("/Library/Keychains/System.keychain")
            .output()
            .context("Failed to search keychain")?;

        if !find_output.status.success() || find_output.stdout.is_empty() {
            println!("✅ CA certificate not found in keychain (already removed)");
            return Ok(true);
        }

        // Extract SHA-1 hashes from output
        let stdout = String::from_utf8_lossy(&find_output.stdout);
        let hashes: Vec<&str> = stdout
            .lines()
            .filter(|line| line.starts_with("SHA-1 hash:"))
            .filter_map(|line| line.split(':').nth(1).map(|h| h.trim()))
            .collect();

        if hashes.is_empty() {
            println!("✅ CA certificate not found in keychain (already removed)");
            return Ok(true);
        }

        let mut success = true;
        for hash in &hashes {
            let output = Command::new("sudo")
                .arg("security")
                .arg("delete-certificate")
                .arg("-Z")
                .arg(hash)
                .arg("/Library/Keychains/System.keychain")
                .output()
                .context("Failed to execute security command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("❌ Failed to remove certificate (hash {}): {}", hash, stderr);
                success = false;
            }
        }

        if success {
            println!("✅ CA certificate removed from keychain");
        }
        Ok(success)
    }

    /// Remove DevRelay domain entries from /etc/hosts
    /// This will prompt the user for their password via sudo
    pub fn uninstall_hosts_entries(domains: &[String]) -> Result<bool> {
        if domains.is_empty() {
            return Ok(true);
        }

        println!("\n🌐 Removing DevRelay entries from /etc/hosts...");
        println!("   This requires administrator privileges.");

        let hosts_content = fs::read_to_string("/etc/hosts")
            .context("Failed to read /etc/hosts")?;

        let mut removed = Vec::new();
        let mut in_devrelay_block = false;
        let filtered: Vec<&str> = hosts_content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();

                // Track the "# DevRelay entries" comment block
                if trimmed == "# DevRelay entries" {
                    in_devrelay_block = true;
                    return false;
                }

                if in_devrelay_block {
                    if trimmed.starts_with("127.0.0.1") {
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 2 && domains.iter().any(|d| parts[1..].contains(&d.as_str())) {
                            removed.push(parts[1..].join(" "));
                            return false;
                        }
                    }
                    // Non-matching line ends the block
                    if !trimmed.is_empty() {
                        in_devrelay_block = false;
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect();

        if removed.is_empty() {
            println!("✅ No DevRelay entries found in /etc/hosts");
            return Ok(true);
        }

        let new_content = filtered.join("\n") + "\n";

        // Write back using sudo
        let mut child = Command::new("sudo")
            .arg("tee")
            .arg("/etc/hosts")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn sudo tee")?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(new_content.as_bytes())
                .context("Failed to write to /etc/hosts")?;
        }

        let status = child.wait().context("Failed to wait for sudo tee")?;

        if status.success() {
            println!("✅ Removed {} domain entry/entries from /etc/hosts:", removed.len());
            for entry in &removed {
                println!("   • {}", entry);
            }
            Ok(true)
        } else {
            eprintln!("❌ Failed to update /etc/hosts");
            Ok(false)
        }
    }

    /// Run the full uninstallation process
    pub fn run_uninstall(ca_name: &str, domains: &[String]) -> Result<()> {
        println!("\n╔════════════════════════════════════════╗");
        println!("║     DevRelay Uninstallation           ║");
        println!("╚════════════════════════════════════════╝\n");

        let mut success = true;

        if !Self::uninstall_ca_cert(ca_name)? {
            success = false;
        }

        if !Self::uninstall_hosts_entries(domains)? {
            success = false;
        }

        if success {
            println!("\n🎉 Uninstallation complete! You may need to restart your browser.");
        } else {
            println!("\n⚠️  Uninstallation completed with some errors. Check the messages above.");
        }

        Ok(())
    }

    /// Check if CA certificate is installed in System Keychain
    pub fn is_ca_installed(cert_path: &Path) -> Result<bool> {
        if !cert_path.exists() {
            return Ok(false);
        }

        // Try to find the cert in the keychain by checking the subject
        let output = Command::new("security")
            .arg("find-certificate")
            .arg("-a")
            .arg("-c")
            .arg("DevRelay CA")
            .arg("/Library/Keychains/System.keychain")
            .output()
            .context("Failed to check keychain")?;

        Ok(output.status.success() && !output.stdout.is_empty())
    }

    /// Run the full installation process
    pub fn run_install(cert_path: &Path, domains: &[String]) -> Result<()> {
        println!("\n╔════════════════════════════════════════╗");
        println!("║     DevRelay Installation Setup       ║");
        println!("╚════════════════════════════════════════╝\n");

        let mut success = true;

        // Install CA certificate
        if !Self::is_ca_installed(cert_path)? {
            if !Self::install_ca_cert(cert_path)? {
                success = false;
            }
        } else {
            println!("✅ CA certificate already installed");
        }

        // Install hosts entries
        if !Self::install_hosts_entries(domains)? {
            success = false;
        }

        if success {
            println!("\n🎉 Installation complete! You may need to restart your browser.");
        } else {
            println!("\n⚠️  Installation completed with some errors. Check the messages above.");
        }

        Ok(())
    }
}
