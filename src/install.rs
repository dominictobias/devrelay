use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

fn run_with_sudo(shell_command: &str) -> Result<String> {
    let output = Command::new("sudo")
        .args(["sh", "-c", shell_command])
        .output()
        .context("Failed to execute sudo command")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.is_empty() {
            return Err(anyhow::anyhow!("Command execution failed"));
        }
        return Err(anyhow::anyhow!("Command failed: {}", error.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub struct Installer;

impl Installer {
    /// Install CA certificate to macOS System Keychain
    pub fn install_ca_cert(cert_path: &Path) -> Result<bool> {
        if !cert_path.exists() {
            return Ok(false);
        }

        println!("🔐 Installing CA certificate to macOS Keychain...");
        println!("   Waiting for authentication...");

        let cert_path_str = cert_path.to_str().context("Invalid cert path")?;
        let command = format!(
            "security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain '{}'",
            cert_path_str.replace("'", "'\\''")
        );

        match run_with_sudo(&command) {
            Ok(_) => {
                println!("✅ CA certificate installed successfully!");
                Ok(true)
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Check if cert is already installed
                if error_msg.contains("The specified item already exists in the keychain") {
                    println!("✅ CA certificate already installed");
                    Ok(true)
                } else if error_msg.contains("user cancelled") {
                    println!("⚠️  Installation cancelled by user");
                    Ok(false)
                } else {
                    eprintln!("❌ Failed to install CA certificate: {}", error_msg);
                    Ok(false)
                }
            }
        }
    }

    /// Add domain entries to /etc/hosts
    pub fn install_hosts_entries(domains: &[String]) -> Result<bool> {
        if domains.is_empty() {
            return Ok(true);
        }

        println!("\n🌐 Updating /etc/hosts with domain entries...");

        // Read current /etc/hosts
        let hosts_content =
            fs::read_to_string("/etc/hosts").context("Failed to read /etc/hosts")?;

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

        println!("   Waiting for authentication...");

        let entries_str = new_entries.replace("'", "'\\''");
        let command = format!("echo '{}' >> /etc/hosts", entries_str.trim());

        match run_with_sudo(&command) {
            Ok(_) => {
                println!(
                    "✅ Added {} domain(s) to /etc/hosts:",
                    missing_domains.len()
                );
                for domain in &missing_domains {
                    println!("   • {}", domain);
                }
                Ok(true)
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("user cancelled") {
                    println!("⚠️  Installation cancelled by user");
                    Ok(false)
                } else {
                    eprintln!("❌ Failed to update /etc/hosts: {}", error_msg);
                    Ok(false)
                }
            }
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
    pub fn uninstall_ca_cert(ca_name: &str) -> Result<bool> {
        println!("🔐 Removing CA certificate from macOS Keychain...");

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

        println!("   Waiting for authentication...");

        let mut success = true;
        for hash in &hashes {
            let command = format!(
                "security delete-certificate -Z {} /Library/Keychains/System.keychain",
                hash
            );

            if let Err(e) = run_with_sudo(&command) {
                let error_msg = e.to_string();
                if error_msg.contains("user cancelled") {
                    println!("⚠️  Uninstallation cancelled by user");
                    return Ok(false);
                }
                eprintln!(
                    "❌ Failed to remove certificate (hash {}): {}",
                    hash, error_msg
                );
                success = false;
            }
        }

        if success {
            println!("✅ CA certificate removed from keychain");
        }
        Ok(success)
    }

    /// Remove DevRelay domain entries from /etc/hosts
    pub fn uninstall_hosts_entries(domains: &[String]) -> Result<bool> {
        if domains.is_empty() {
            return Ok(true);
        }

        println!("\n🌐 Removing DevRelay entries from /etc/hosts...");

        let hosts_content =
            fs::read_to_string("/etc/hosts").context("Failed to read /etc/hosts")?;

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
                        if parts.len() >= 2
                            && domains.iter().any(|d| parts[1..].contains(&d.as_str()))
                        {
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

        println!("   Waiting for authentication...");

        let new_content = filtered.join("\n") + "\n";
        let content_escaped = new_content.replace("'", "'\\''").replace("\"", "\\\"");

        // Use printf instead of echo to better handle special characters
        let command = format!(
            "printf '%s' '{}' | tee /etc/hosts > /dev/null",
            content_escaped
        );

        match run_with_sudo(&command) {
            Ok(_) => {
                println!(
                    "✅ Removed {} domain entry/entries from /etc/hosts:",
                    removed.len()
                );
                for entry in &removed {
                    println!("   • {}", entry);
                }
                Ok(true)
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("user cancelled") {
                    println!("⚠️  Uninstallation cancelled by user");
                    Ok(false)
                } else {
                    eprintln!("❌ Failed to update /etc/hosts: {}", error_msg);
                    Ok(false)
                }
            }
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
