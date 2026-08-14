use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use bridge::modal_action::ProgressTracker;
use schema::java_manager::{JavaProvider, JavaVariant};

#[derive(serde::Deserialize)]
struct AdoptiumRelease {
    binaries: Vec<AdoptiumBinary>,
    version_data: AdoptiumVersionData,
}

#[derive(serde::Deserialize)]
struct AdoptiumBinary {
    architecture: String,
    os: String,
    package: AdoptiumPackage,
}

#[derive(serde::Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
}

#[derive(serde::Deserialize)]
struct AdoptiumVersionData {
    major: u32,
}

#[derive(serde::Deserialize)]
struct ZuluPackage {
    java_version: Vec<u32>,
    name: String,
    download_url: String,
}

pub async fn fetch_versions(
    provider: JavaProvider,
    http_client: &reqwest::Client,
    runtime_base_dir: &Path,
) -> Result<Vec<JavaVariant>> {
    let mut variants = Vec::new();

    let target_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };

    let target_arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86" // fallback
    };

    match provider {
        JavaProvider::Adoptium => {
            // Fetch Java 8, 17, 21 didnt add subversions cuz its pain to do gpui
            for version in [8, 17, 21] {
                let url = format!(
                    "https://api.adoptium.net/v3/assets/feature_releases/{}/ga?architecture={}&heap_size=normal&image_type=jre&jvm_impl=hotspot&os={}",
                    version, target_arch, target_os
                );
                
                if let Ok(res) = http_client.get(&url).send().await {
                    if let Ok(releases) = res.json::<Vec<AdoptiumRelease>>().await {
                        // Only take the first (latest) release for this major version
                        if let Some(release) = releases.first() {
                            // And take the first matching binary
                            if let Some(binary) = release.binaries.first() {
                                let install_dir = runtime_base_dir.join("adoptium").join(format!("jre-{}", release.version_data.major));
                                
                                variants.push(JavaVariant {
                                    provider: JavaProvider::Adoptium,
                                    major_version: release.version_data.major,
                                    architecture: binary.architecture.clone(),
                                    os: binary.os.clone(),
                                    download_url: binary.package.link.clone(),
                                    is_installed: install_dir.exists(),
                                });
                            }
                        }
                    }
                }
            }
        }
        JavaProvider::Zulu => {
            let zulu_arch = if target_arch == "x64" { "x86" } else { target_arch }; // Zulu uses x86 with hw_bitness=64
            let zulu_bitness = if target_arch == "x64" { "64" } else { "32" };
            let ext = if target_os == "windows" { "zip" } else { "tar.gz" };

            for version in [8, 17, 21] {
                let url = format!(
                    "https://api.azul.com/metadata/v1/zulu/packages/?os={}&arch={}&hw_bitness={}&ext={}&java_package_type=jre&release_status=ga&java_version={}",
                    target_os, zulu_arch, zulu_bitness, ext, version
                );
                
                if let Ok(res) = http_client.get(&url).send().await {
                    if let Ok(packages) = res.json::<Vec<ZuluPackage>>().await {
                        // Only take the first latest package to avoid duplicates , will do releases in versions soon
                        if let Some(pkg) = packages.first() {
                            let major_version = pkg.java_version.first().copied().unwrap_or(version);
                            let install_dir = runtime_base_dir.join("zulu").join(format!("jre-{}", major_version));
                            
                            variants.push(JavaVariant {
                                provider: JavaProvider::Zulu,
                                major_version,
                                architecture: target_arch.to_string(),
                                os: target_os.to_string(),
                                download_url: pkg.download_url.clone(),
                                is_installed: install_dir.exists(),
                            });
                        }
                    }
                }
            }
        }
        JavaProvider::Mojang => {
            // For Mojang we need the metadata manager. This can be complex to parse manually here. because it fucking has 512 ad more these typo fucking versions of it 
            // Let's stub it for now or implement a quick fallback, as Mojang java is managed by the launch process natively.
            variants.push(JavaVariant {
                provider: JavaProvider::Mojang,
                major_version: 17,
                architecture: target_arch.to_string(),
                os: target_os.to_string(),
                download_url: "".to_string(),
                is_installed: true, // Native launcher logic handles it W Moul :)
            });
        }
    }

    Ok(variants)
}

pub async fn install_java(
    variant: JavaVariant,
    http_client: &reqwest::Client,
    runtime_base_dir: &Path,
    tracker: &ProgressTracker,
) -> Result<()> {
    let provider_str = match variant.provider {
        JavaProvider::Adoptium => "adoptium",
        JavaProvider::Zulu => "zulu",
        JavaProvider::Mojang => "mojang",
    };
    
    let install_dir = runtime_base_dir.join(provider_str).join(format!("jre-{}", variant.major_version));
    
    // Download hahahhhah
    tracker.set_title(format!("Downloading Java {} from {}", variant.major_version, provider_str).into());
    tracker.notify();
    
    let mut res = http_client.get(&variant.download_url).send().await?.error_for_status()?;
    let total_size = res.content_length().unwrap_or(0);
    tracker.set_total(total_size as usize);
    tracker.notify();
    
    let temp_dir = std::env::temp_dir().join(format!("pandora_java_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    std::fs::create_dir_all(&temp_dir)?;
    
    let file_name = variant.download_url.split('/').last().unwrap_or("java.zip");
    let archive_path = temp_dir.join(file_name);
    
    let mut file = std::fs::File::create(&archive_path)?;
    let mut downloaded: u64 = 0;
    
    let mut last_notify = std::time::Instant::now();
    while let Some(chunk) = res.chunk().await? {
        std::io::Write::write_all(&mut file, &chunk)?;
        downloaded += chunk.len() as u64;
        
        tracker.set_count(downloaded as usize);
        if last_notify.elapsed().as_millis() > 100 {
            tracker.notify();
            last_notify = std::time::Instant::now();
        }
    }
    
    tracker.set_count(total_size as usize);
    tracker.notify();
    
    // Ensure file is flushed to disk and drop the file lock to prevent falseclaiming logs 
    file.sync_all()?;
    drop(file);
    
    // Extract
    tracker.set_title("Extracting files...".into());
    tracker.notify();
    
    let install_dir_clone = install_dir.clone();
    let archive_path_clone = archive_path.clone();
    
    let install_dir_clone_for_cleanup = install_dir.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&install_dir_clone)?;
        
        let extract_result = (|| -> Result<()> {
            if archive_path_clone.extension().and_then(|s| s.to_str()) == Some("zip") {
                let file = std::fs::File::open(&archive_path_clone)?;
                let mut archive = zip::ZipArchive::new(file)?;
                
                for i in 0..archive.len() {
                    let mut file = archive.by_index(i)?;
                    let outpath = match file.enclosed_name() {
                        Some(path) => install_dir_clone.join(path),
                        None => continue,
                    };
                    
                    if (*file.name()).ends_with('/') {
                        std::fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            if !p.exists() {
                                std::fs::create_dir_all(&p)?;
                            }
                        }
                        let mut outfile = std::fs::File::create(&outpath)?;
                        std::io::copy(&mut file, &mut outfile)?;
                    }
                }
            } else {
                // assume tar.gz
                let tar_gz = std::fs::File::open(&archive_path_clone)?;
                let tar = flate2::read::GzDecoder::new(tar_gz);
                let mut archive = tar::Archive::new(tar);
                archive.unpack(&install_dir_clone)?;
            }
            Ok(())
        })();

        if extract_result.is_err() {
            let _ = std::fs::remove_dir_all(&install_dir_clone_for_cleanup);
        }
        
        extract_result
    }).await??;
    
    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
    
    Ok(())
}

pub async fn uninstall_java(
    variant: JavaVariant,
    runtime_base_dir: &Path,
) -> Result<()> {
    let provider_str = match variant.provider {
        JavaProvider::Adoptium => "adoptium",
        JavaProvider::Zulu => "zulu",
        JavaProvider::Mojang => "mojang",
    };
    
    let install_dir = runtime_base_dir.join(provider_str).join(format!("jre-{}", variant.major_version));
    
    if install_dir.exists() {
        std::fs::remove_dir_all(install_dir)?;
    }
    
    Ok(())
}
