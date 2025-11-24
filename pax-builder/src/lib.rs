use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaxPackageSpec {
    pub name: Option<String>,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub source_url: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub dependencies: PackageDependencies,
    pub build: BuildConfig,
    pub install: InstallConfig,
    pub files: FileConfig,
    pub scripts: ScriptConfig,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDependencies {
    #[serde(default)]
    pub build_dependencies: Vec<Dependency>,
    #[serde(default)]
    pub runtime_dependencies: Vec<Dependency>,
    #[serde(default)]
    pub optional_dependencies: Vec<Dependency>,
    #[serde(default)]
    pub conflicts: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version_constraint: String,
    pub optional: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub build_system: BuildSystem,
    pub build_commands: Vec<String>,
    #[serde(default)]
    pub build_dependencies: Vec<String>,
    pub build_flags: Vec<String>,
    pub environment: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub target_architectures: Vec<TargetArch>,
    pub cross_compiler_prefix: Option<String>,
    pub target_sysroot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TargetArch {
    X86_64,
    X86_64v1,
    X86_64v2,
    X86_64v3,
    Aarch64,
    Armv7l,
    Armv8l,
    Riscv64,
    Powerpc64le,
    S390x,
}

impl TargetArch {
    pub fn to_triple(&self) -> &'static str {
        match self {
            TargetArch::X86_64 => "x86_64-unknown-linux-gnu",
            TargetArch::X86_64v1 => "x86_64-unknown-linux-gnu",
            TargetArch::X86_64v2 => "x86_64-unknown-linux-gnu",
            TargetArch::X86_64v3 => "x86_64-unknown-linux-gnu",
            TargetArch::Aarch64 => "aarch64-unknown-linux-gnu",
            TargetArch::Armv7l => "armv7-unknown-linux-gnueabihf",
            TargetArch::Armv8l => "aarch64-unknown-linux-gnu",
            TargetArch::Riscv64 => "riscv64gc-unknown-linux-gnu",
            TargetArch::Powerpc64le => "powerpc64le-unknown-linux-gnu",
            TargetArch::S390x => "s390x-unknown-linux-gnu",
        }
    }

    pub fn as_label(&self) -> &'static str {
        match self {
            TargetArch::X86_64 => "x86_64",
            TargetArch::X86_64v1 => "x86_64_v1",
            TargetArch::X86_64v2 => "x86_64_v2",
            TargetArch::X86_64v3 => "x86_64_v3",
            TargetArch::Aarch64 => "aarch64",
            TargetArch::Armv7l => "armv7l",
            TargetArch::Armv8l => "armv8l",
            TargetArch::Riscv64 => "riscv64",
            TargetArch::Powerpc64le => "powerpc64le",
            TargetArch::S390x => "s390x",
        }
    }

    pub fn cross_compiler_prefix(&self) -> &'static str {
        match self {
            TargetArch::X86_64 => "x86_64-linux-gnu-",
            TargetArch::X86_64v1 => "x86_64-linux-gnu-",
            TargetArch::X86_64v2 => "x86_64-linux-gnu-",
            TargetArch::X86_64v3 => "x86_64-linux-gnu-",
            TargetArch::Aarch64 => "aarch64-linux-gnu-",
            TargetArch::Armv7l => "arm-linux-gnueabihf-",
            TargetArch::Armv8l => "aarch64-linux-gnu-",
            TargetArch::Riscv64 => "riscv64-linux-gnu-",
            TargetArch::Powerpc64le => "powerpc64le-linux-gnu-",
            TargetArch::S390x => "s390x-linux-gnu-",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "x86_64" | "amd64" => Some(TargetArch::X86_64),
            "x86_64v1" | "x86_64_v1" => Some(TargetArch::X86_64v1),
            "x86_64v2" | "x86_64_v2" => Some(TargetArch::X86_64v2),
            "x86_64v3" | "x86_64_v3" => Some(TargetArch::X86_64v3),
            "aarch64" | "arm64" => Some(TargetArch::Aarch64),
            "armv7l" | "armv7" => Some(TargetArch::Armv7l),
            "armv8l" => Some(TargetArch::Armv8l),
            "riscv64" => Some(TargetArch::Riscv64),
            "powerpc64le" | "ppc64le" => Some(TargetArch::Powerpc64le),
            "s390x" => Some(TargetArch::S390x),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossCompileConfig {
    pub target_arch: TargetArch,
    pub compiler_prefix: String,
    pub sysroot: Option<String>,
    pub environment: HashMap<String, String>,
    pub build_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildSystem {
    Make,
    CMake,
    Meson,
    Cargo,
    Npm,
    Pip,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    pub install_method: InstallMethod,
    pub install_commands: Vec<String>,
    pub install_directories: Vec<String>,
    pub install_files: Vec<FileMapping>,
    pub post_install_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InstallMethod {
    CopyFiles,
    RunCommands,
    ExtractArchive,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMapping {
    pub source: String,
    pub destination: String,
    pub permissions: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub binary_files: Vec<String>,
    pub config_files: Vec<String>,
    pub documentation_files: Vec<String>,
    pub license_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptConfig {
    pub pre_install: Option<String>,
    pub post_install: Option<String>,
    pub pre_uninstall: Option<String>,
    pub post_uninstall: Option<String>,
    pub pre_upgrade: Option<String>,
    pub post_upgrade: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltPackage {
    pub spec: PaxPackageSpec,
    pub package_path: PathBuf,
    pub build_log: String,
    pub checksum: String,
    pub size: u64,
    pub build_time: u64,
    pub build_duration: u64,
}

#[derive(Debug)]
pub struct PaxPackageBuilder {
    build_directory: PathBuf,
    output_directory: PathBuf,
    temp_directory: PathBuf,
    verbose: bool,
    target_arch: Option<TargetArch>,
    use_bubblewrap: bool,
    buildroot_directory: PathBuf,
    host_arch: String,
    allow_dependency_builds: bool,
}

#[derive(Debug, Clone)]
struct SourcePreparation {
    source_dir: PathBuf,
    archive_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct PackagedArtifacts {
    binary_artifact: PathBuf,
    source_artifact: PathBuf,
}

impl PaxPackageBuilder {
    pub fn new() -> Result<Self, String> {
        // Detect host architecture
        let host_arch = Self::detect_host_architecture()?;

        // Use user-specific directories to avoid permission issues
        let _user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let base_dir = PathBuf::from(&home_dir).join(".local/share/pax-builder");

        let build_dir = base_dir.join("build");
        let output_dir = std::env::var("PAX_RESULTS_ROOT")
            .map(PathBuf::from)
            .or_else(|_| {
                std::env::var("PAX_BUILDER_PROJECT_ROOT")
                    .map(PathBuf::from)
                    .map(|root| root.join("results"))
            })
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("results")
            });
        let buildroot_dir = base_dir.join("buildroot");
        let temp_dir = base_dir.join("temp");

        // Create directories with proper permissions
        Self::create_directory_with_permissions(&build_dir)?;
        Self::create_directory_with_permissions(&output_dir)?;
        Self::create_directory_with_permissions(&buildroot_dir)?;
        Self::create_directory_with_permissions(&temp_dir)?;

        Ok(Self {
            build_directory: build_dir,
            output_directory: output_dir,
            temp_directory: temp_dir,
            verbose: false,
            target_arch: None,
            use_bubblewrap: true,
            buildroot_directory: buildroot_dir,
            host_arch,
            allow_dependency_builds: true,
        })
    }

    fn detect_host_architecture() -> Result<String, String> {
        let arch = std::env::consts::ARCH;
        match arch {
            "x86_64" => Ok("x86_64".to_string()),
            "aarch64" => Ok("aarch64".to_string()),
            "arm" => Ok("armv7l".to_string()),
            "riscv64" => Ok("riscv64".to_string()),
            "powerpc64le" => Ok("powerpc64le".to_string()),
            "s390x" => Ok("s390x".to_string()),
            _ => Err(format!("Unsupported host architecture: {}", arch)),
        }
    }

    fn create_directory_with_permissions(path: &Path) -> Result<(), String> {
        fs::create_dir_all(path)
            .map_err(|_| format!("Failed to create directory: {}", path.display()))?;

        // Set permissions to 755 (rwxr-xr-x)
        let mut perms = fs::metadata(path)
            .map_err(|_| format!("Failed to get metadata for: {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|_| format!("Failed to set permissions for: {}", path.display()))?;

        Ok(())
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_target_arch(mut self, target_arch: TargetArch) -> Result<Self, String> {
        // Validate that the target architecture matches the host architecture
        let target_arch_str = match target_arch {
            TargetArch::X86_64
            | TargetArch::X86_64v1
            | TargetArch::X86_64v2
            | TargetArch::X86_64v3 => "x86_64",
            TargetArch::Aarch64 => "aarch64",
            TargetArch::Armv7l => "armv7l",
            TargetArch::Armv8l => "aarch64",
            TargetArch::Riscv64 => "riscv64",
            TargetArch::Powerpc64le => "powerpc64le",
            TargetArch::S390x => "s390x",
        };

        // Allow cross-compilation for aarch64 on x86_64 hosts
        if target_arch_str != self.host_arch
            && !(target_arch_str == "aarch64" && self.host_arch == "x86_64")
        {
            return Err(format!(
                "Target architecture {} is not supported on host architecture {}. \
                PAX builder only supports native builds. Please build on a {} machine or \
                remove {} from target_architectures in your pax.yaml",
                target_arch_str, self.host_arch, target_arch_str, target_arch_str
            ));
        }

        self.target_arch = Some(target_arch);
        Ok(self)
    }

    pub fn with_bubblewrap(mut self, use_bwrap: bool) -> Self {
        self.use_bubblewrap = use_bwrap;
        self
    }

    pub fn with_dependency_builds(mut self, allow: bool) -> Self {
        self.allow_dependency_builds = allow;
        self
    }

    pub fn with_output_directory(mut self, output_dir: PathBuf) -> Self {
        self.output_directory = output_dir;
        self
    }

    pub fn validate_spec(&self, spec_path: &Path) -> Result<Vec<String>, String> {
        let spec = self.load_spec(spec_path)?;
        let mut errors = Vec::new();

        // Validate required fields
        if spec.name.is_none() || spec.name.as_ref().unwrap().is_empty() {
            errors.push("Package name is required".to_string());
        }

        if spec.version.is_empty() {
            errors.push("Package version is required".to_string());
        }

        if spec.description.is_empty() {
            errors.push("Package description is required".to_string());
        }

        if spec.author.is_empty() {
            errors.push("Package author is required".to_string());
        }

        // Validate version format - basic check
        if spec.version.is_empty() {
            errors.push("Version cannot be empty".to_string());
        }

        // Validate build configuration
        if spec.build.build_commands.is_empty() {
            errors.push("At least one build command is required".to_string());
        }

        // Validate install configuration
        match spec.install.install_method {
            InstallMethod::CopyFiles => {
                if spec.install.install_files.is_empty() {
                    errors.push("Install files are required for CopyFiles method".to_string());
                }
            }
            InstallMethod::RunCommands => {
                if spec.install.install_commands.is_empty() {
                    errors.push("Install commands are required for RunCommands method".to_string());
                }
            }
            _ => {}
        }

        Ok(errors)
    }

    pub fn clean_build_directory(&self) -> Result<(), String> {
        if self.build_directory.exists() {
            fs::remove_dir_all(&self.build_directory)
                .map_err(|_| "Failed to clean build directory")?;
        }
        Ok(())
    }

    pub fn get_build_stats(&self) -> BuildStats {
        BuildStats {
            build_directory: self.build_directory.clone(),
            output_directory: self.output_directory.clone(),
            temp_directory: self.temp_directory.clone(),
        }
    }

    pub fn build_package(&mut self, spec_path: &Path) -> Result<Vec<BuiltPackage>, String> {
        let start_time = SystemTime::now();
        let spec = self.load_spec(spec_path)?;

        let package_name = spec
            .name
            .clone()
            .unwrap_or_else(|| "unnamed-package".to_string());

        let build_id = format!(
            "{}-{}-{}",
            package_name.replace('/', "_"),
            spec.version.replace('/', "_"),
            start_time
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "System clock drift detected".to_string())?
                .as_micros()
        );

        let workspace = self.build_directory.join(&build_id);
        fs::create_dir_all(&workspace)
            .map_err(|_| format!("Failed to create workspace {}", workspace.display()))?;

        let keep_workspace = std::env::var("PAX_BUILDER_KEEP_WORKSPACE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "True"))
            .unwrap_or(false);

        let mut build_log = String::new();

        let source_info = self
            .prepare_sources(&spec, &workspace, &mut build_log)
            .map_err(|err| {
                if !keep_workspace {
                    let _ = fs::remove_dir_all(&workspace);
                }
                err
            })?;

        let dependency_env = self
            .prepare_dependencies(spec_path, &spec, &workspace, &mut build_log)
            .map_err(|err| {
                if !keep_workspace {
                    let _ = fs::remove_dir_all(&workspace);
                }
                err
            })?;

        if let Err(err) = self.execute_build_steps(
            &spec,
            &source_info.source_dir,
            &dependency_env,
            &mut build_log,
        ) {
            if !keep_workspace {
                let _ = fs::remove_dir_all(&workspace);
            }
            return Err(err);
        }

        let destdir = workspace.join("destdir");
        fs::create_dir_all(&destdir)
            .map_err(|_| format!("Failed to create DESTDIR {}", destdir.display()))?;

        if let Err(err) = self.execute_install_steps(
            &spec,
            &source_info.source_dir,
            &destdir,
            &dependency_env,
            &mut build_log,
        ) {
            if !keep_workspace {
                let _ = fs::remove_dir_all(&workspace);
            }
            return Err(err);
        }

        let effective_package_name =
            std::env::var("PAX_PACKAGE_NAME").unwrap_or_else(|_| package_name.clone());
        let effective_version =
            std::env::var("PAX_PACKAGE_VERSION").unwrap_or_else(|_| spec.version.clone());
        let package_release =
            std::env::var("PAX_PACKAGE_RELEASE").unwrap_or_else(|_| "1".to_string());
        let target_release =
            std::env::var("PAX_TARGET_RELEASE").unwrap_or_else(|_| "oreon11".to_string());
        let branch = std::env::var("PAX_BRANCH").unwrap_or_else(|_| "mainstream".to_string());
        let arch_label = self
            .target_arch
            .as_ref()
            .map(|arch| arch.as_label().to_string())
            .unwrap_or_else(|| self.host_arch.clone());
        let mut effective_release = package_release.clone();
        if !target_release.is_empty() && !effective_release.contains(&target_release) {
            effective_release = format!("{}.{}", effective_release, target_release);
        }

        let packaged = match self.package_artifacts(
            &spec,
            &destdir,
            spec_path,
            &mut build_log,
            &source_info,
            &effective_package_name,
            &effective_version,
            &effective_release,
            &target_release,
            &branch,
            &arch_label,
        ) {
            Ok(paths) => paths,
            Err(err) => {
                if !keep_workspace {
                    let _ = fs::remove_dir_all(&workspace);
                }
                return Err(err);
            }
        };

        let binary_size = fs::metadata(&packaged.binary_artifact)
            .map_err(|_| {
                format!(
                    "Failed to stat artifact {}",
                    packaged.binary_artifact.display()
                )
            })?
            .len();
        let binary_checksum = self.calculate_checksum(&packaged.binary_artifact)?;
        let source_size = fs::metadata(&packaged.source_artifact)
            .map_err(|_| {
                format!(
                    "Failed to stat artifact {}",
                    packaged.source_artifact.display()
                )
            })?
            .len();
        let source_checksum = self.calculate_checksum(&packaged.source_artifact)?;

        let build_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock drift detected".to_string())?
            .as_secs();
        let build_duration = SystemTime::now()
            .duration_since(start_time)
            .unwrap_or_default()
            .as_secs();

        let source_build_log = build_log.clone();
        let mut results = Vec::new();
        results.push(BuiltPackage {
            spec: spec.clone(),
            package_path: packaged.binary_artifact.clone(),
            build_log,
            checksum: binary_checksum,
            size: binary_size,
            build_time,
            build_duration,
        });
        results.push(BuiltPackage {
            spec,
            package_path: packaged.source_artifact.clone(),
            build_log: source_build_log,
            checksum: source_checksum,
            size: source_size,
            build_time,
            build_duration,
        });

        // Clean workspace after success
        if !keep_workspace {
            let _ = fs::remove_dir_all(&workspace);
        }

        Ok(results)
    }

    fn load_spec(&self, spec_path: &Path) -> Result<PaxPackageSpec, String> {
        let mut file = File::open(spec_path)
            .map_err(|_| format!("Failed to open spec file: {}", spec_path.display()))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|_| format!("Failed to read spec file: {}", spec_path.display()))?;

        serde_yaml::from_str(&contents)
            .map_err(|e| format!("Failed to parse spec file: {} - {}", spec_path.display(), e))
    }

    fn calculate_checksum(&self, path: &Path) -> Result<String, String> {
        use sha2::{Digest, Sha256};

        let mut file =
            File::open(path).map_err(|_| format!("Failed to open file: {}", path.display()))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|_| format!("Failed to read file: {}", path.display()))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    fn prepare_sources(
        &self,
        spec: &PaxPackageSpec,
        workspace: &Path,
        build_log: &mut String,
    ) -> Result<SourcePreparation, String> {
        if let Some(url) = &spec.source_url {
            if url.trim().is_empty() {
                build_log.push_str("No source URL defined, skipping download step\n");
                return Ok(SourcePreparation {
                    source_dir: workspace.to_path_buf(),
                    archive_path: None,
                });
            }
            build_log.push_str(&format!("Downloading source from {}\n", url));
            let archive_name = Path::new(url)
                .file_name()
                .ok_or_else(|| "Unable to determine source archive name".to_string())?;
            let archive_path = workspace.join(archive_name);
            self.download_source(url, &archive_path)?;
            let extracted_dir = self.extract_archive(&archive_path, workspace, build_log)?;
            Ok(SourcePreparation {
                source_dir: extracted_dir,
                archive_path: Some(archive_path),
            })
        } else {
            Ok(SourcePreparation {
                source_dir: workspace.to_path_buf(),
                archive_path: None,
            })
        }
    }

    fn download_source(&self, url: &str, destination: &Path) -> Result<(), String> {
        let mut last_error: Option<String> = None;
        for candidate in Self::candidate_source_urls(url) {
            match self.fetch_source(&candidate, destination) {
                Ok(()) => return Ok(()),
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or_else(|| format!("Failed to download {}", url)))
    }

    fn candidate_source_urls(original: &str) -> Vec<String> {
        let mut urls = vec![original.to_string()];
        if let Some(path_idx) = original.find("://ftp.gnu.org/gnu/") {
            let path = &original[(path_idx + "://ftp.gnu.org/".len())..];
            urls.push(format!("https://ftpmirror.gnu.org/{}", path));
            urls.push(format!("https://mirrors.kernel.org/gnu/{}", path));
        }

        if original.contains("://github.com/") && original.contains("/archive/refs/tags/") {
            // Convert to codeload URL which is more CDN friendly
            if let Some(stripped) = original.strip_prefix("https://github.com/") {
                if let Some((repo, suffix)) = stripped.split_once("/archive/refs/tags/") {
                    urls.push(format!(
                        "https://codeload.github.com/{}/tar.gz/refs/tags/{}",
                        repo, suffix
                    ));
                }
            }
        }

        urls.dedup();
        urls
    }

    fn fetch_source(&self, url: &str, destination: &Path) -> Result<(), String> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|err| format!("Failed to create HTTP client: {}", err))?;
        let response = client
            .get(url)
            .send()
            .map_err(|err| format!("Failed to download {}: {}", url, err))?;
        if !response.status().is_success() {
            return Err(format!(
                "Failed to download {}: HTTP {}",
                url,
                response.status()
            ));
        }
        let bytes = response
            .bytes()
            .map_err(|err| format!("Failed to read response body: {}", err))?;
        fs::write(destination, &bytes)
            .map_err(|err| format!("Failed to write archive {}: {}", destination.display(), err))?;
        Ok(())
    }

    fn extract_archive(
        &self,
        archive: &Path,
        workspace: &Path,
        build_log: &mut String,
    ) -> Result<PathBuf, String> {
        build_log.push_str(&format!(
            "Extracting archive {} into {}\n",
            archive.display(),
            workspace.display()
        ));

        let status = Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(workspace)
            .status()
            .map_err(|err| format!("Failed to spawn tar: {}", err))?;
        if !status.success() {
            return Err(format!(
                "Failed to extract archive {} (exit code {:?})",
                archive.display(),
                status.code()
            ));
        }

        let mut entries = fs::read_dir(workspace)
            .map_err(|err| format!("Failed to read workspace {}: {}", workspace.display(), err))?;
        let first_dir = entries
            .find_map(|entry| {
                entry.ok().and_then(|e| {
                    e.file_type()
                        .ok()
                        .filter(|ft| ft.is_dir())
                        .map(|_| e.path())
                })
            })
            .ok_or_else(|| "Unable to determine extracted source directory".to_string())?;

        Ok(first_dir)
    }

    fn prepare_dependencies(
        &self,
        spec_path: &Path,
        spec: &PaxPackageSpec,
        workspace: &Path,
        build_log: &mut String,
    ) -> Result<HashMap<String, String>, String> {
        if !self.allow_dependency_builds {
            build_log.push_str("Dependency auto-build disabled; skipping dependency build step\n");
            return Ok(HashMap::new());
        }

        if spec.dependencies.build_dependencies.is_empty()
            && spec.build.build_dependencies.is_empty()
        {
            return Ok(HashMap::new());
        }

        let deps_sysroot = workspace.join("deps-sysroot");
        fs::create_dir_all(&deps_sysroot).map_err(|err| {
            format!(
                "Failed to create dependency sysroot {}: {}",
                deps_sysroot.display(),
                err
            )
        })?;

        let mut visited = HashSet::new();
        for dependency in &spec.dependencies.build_dependencies {
            if !Self::should_auto_build_dependency(dependency.name.as_str()) {
                build_log.push_str(&format!(
                    "Skipping auto-build for dependency {} (not marked as headers)\n",
                    dependency.name
                ));
                continue;
            }
            self.build_dependency(
                dependency.name.as_str(),
                spec_path,
                &deps_sysroot,
                &mut visited,
                build_log,
            )?;
        }

        for dependency_name in &spec.build.build_dependencies {
            if !Self::should_auto_build_dependency(dependency_name) {
                build_log.push_str(&format!(
                    "Skipping auto-build for build dependency {} (not marked as headers)\n",
                    dependency_name
                ));
                continue;
            }
            self.build_dependency(
                dependency_name,
                spec_path,
                &deps_sysroot,
                &mut visited,
                build_log,
            )?;
        }

        Ok(Self::dependency_environment(&deps_sysroot))
    }

    fn build_dependency(
        &self,
        dep_name: &str,
        spec_path: &Path,
        deps_sysroot: &Path,
        visited: &mut HashSet<String>,
        build_log: &mut String,
    ) -> Result<(), String> {
        let recipe_dir = match Self::find_dependency_recipe(dep_name, spec_path) {
            Some(path) => path,
            None => {
                build_log.push_str(&format!(
                    "Skipping dependency {}: recipe not found\n",
                    dep_name
                ));
                return Ok(());
            }
        };

        let current_package = spec_path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| Self::normalize_name(&n.to_string_lossy()))
            .unwrap_or_default();
        let recipe_name = recipe_dir
            .file_name()
            .map(|n| Self::normalize_name(&n.to_string_lossy()))
            .unwrap_or_default();
        if recipe_name == current_package {
            build_log.push_str(&format!(
                "Skipping dependency {} to avoid recursive build loop\n",
                dep_name
            ));
            return Ok(());
        }

        if !visited.insert(recipe_name.clone()) {
            build_log.push_str(&format!(
                "Dependency {} already built, skipping duplicate\n",
                dep_name
            ));
            return Ok(());
        }

        let dep_spec_path = Self::find_recipe_spec(&recipe_dir).ok_or_else(|| {
            format!(
                "Recipe {} does not contain a .yaml specification",
                recipe_dir.display()
            )
        })?;

        let dep_spec = self.load_spec(&dep_spec_path)?;

        let package_name = dep_spec
            .name
            .clone()
            .unwrap_or_else(|| recipe_name.replace('_', "-"));
        let target_label = self
            .target_arch
            .as_ref()
            .map(|arch| arch.to_triple())
            .unwrap_or_else(|| self.host_arch.as_str())
            .replace("unknown-linux-gnu", "");

        let cache_dir = if self.output_directory.is_absolute() {
            self.output_directory.clone()
        } else {
            std::env::current_dir()
                .map_err(|_| "Failed to determine current working directory".to_string())?
                .join(&self.output_directory)
        };

        let expected_artifact = cache_dir.join(format!(
            "{}-{}-{}.pax",
            package_name, dep_spec.version, target_label
        ));
        if expected_artifact.exists() {
            build_log.push_str(&format!(
                "Using cached dependency artifact {}\n",
                expected_artifact.display()
            ));
            self.extract_dependency_artifact(&expected_artifact, deps_sysroot)?;
            return Ok(());
        }

        build_log.push_str(&format!(
            "Building dependency {} using {}\n",
            dep_name,
            dep_spec_path.display()
        ));

        let mut dep_builder = PaxPackageBuilder::new()?
            .with_output_directory(self.output_directory.clone())
            .with_bubblewrap(self.use_bubblewrap)
            .with_dependency_builds(false);

        if let Some(target) = self.target_arch.clone() {
            dep_builder = dep_builder.with_target_arch(target)?;
        }

        let artifacts = dep_builder.build_package(&dep_spec_path)?;
        for artifact in artifacts {
            self.extract_dependency_artifact(&artifact.package_path, deps_sysroot)?;
        }

        Ok(())
    }

    fn extract_dependency_artifact(
        &self,
        artifact_path: &Path,
        deps_sysroot: &Path,
    ) -> Result<(), String> {
        fs::create_dir_all(deps_sysroot).map_err(|err| {
            format!(
                "Failed to create dependency extract dir {}: {}",
                deps_sysroot.display(),
                err
            )
        })?;

        let status = Command::new("tar")
            .arg("-xzf")
            .arg(artifact_path)
            .arg("-C")
            .arg(deps_sysroot)
            .status()
            .map_err(|err| format!("Failed to extract dependency artifact: {}", err))?;

        if !status.success() {
            return Err(format!(
                "Failed to extract dependency artifact {} (exit code {:?})",
                artifact_path.display(),
                status.code()
            ));
        }

        Ok(())
    }

    fn dependency_environment(deps_sysroot: &Path) -> HashMap<String, String> {
        let mut env = HashMap::new();

        let include_dirs = [
            deps_sysroot.join("usr/include"),
            deps_sysroot.join("usr/local/include"),
        ];
        let include_flags = include_dirs
            .iter()
            .filter(|dir| dir.exists())
            .map(|dir| format!("-I{}", dir.display()))
            .collect::<Vec<_>>()
            .join(" ");
        if !include_flags.is_empty() {
            env.insert("CPPFLAGS".to_string(), include_flags.clone());
            env.insert("CFLAGS".to_string(), include_flags);
        }

        let library_dirs = [
            deps_sysroot.join("usr/lib"),
            deps_sysroot.join("usr/lib64"),
            deps_sysroot.join("usr/local/lib"),
            deps_sysroot.join("usr/local/lib64"),
        ];
        let lib_flags = library_dirs
            .iter()
            .filter(|dir| dir.exists())
            .map(|dir| format!("-L{}", dir.display()))
            .collect::<Vec<_>>()
            .join(" ");
        if !lib_flags.is_empty() {
            env.insert("LDFLAGS".to_string(), lib_flags.clone());
            env.insert(
                "LIBRARY_PATH".to_string(),
                library_dirs
                    .iter()
                    .filter(|dir| dir.exists())
                    .map(|dir| dir.display().to_string())
                    .collect::<Vec<_>>()
                    .join(":"),
            );
            env.insert(
                "LD_LIBRARY_PATH".to_string(),
                library_dirs
                    .iter()
                    .filter(|dir| dir.exists())
                    .map(|dir| dir.display().to_string())
                    .collect::<Vec<_>>()
                    .join(":"),
            );
        }

        let pkg_config_dirs = [
            deps_sysroot.join("usr/lib/pkgconfig"),
            deps_sysroot.join("usr/lib64/pkgconfig"),
            deps_sysroot.join("usr/local/lib/pkgconfig"),
            deps_sysroot.join("usr/local/lib64/pkgconfig"),
        ];
        let pkg_config_path = pkg_config_dirs
            .iter()
            .filter(|dir| dir.exists())
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(":");
        if !pkg_config_path.is_empty() {
            env.insert("PKG_CONFIG_PATH".to_string(), pkg_config_path);
        }

        let bin_dirs = [
            deps_sysroot.join("usr/bin"),
            deps_sysroot.join("usr/sbin"),
            deps_sysroot.join("usr/local/bin"),
            deps_sysroot.join("usr/local/sbin"),
        ];
        let path_additions = bin_dirs
            .iter()
            .filter(|dir| dir.exists())
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(":");
        if !path_additions.is_empty() {
            env.insert("PATH".to_string(), path_additions);
        }

        let cmake_prefix = [deps_sysroot.join("usr"), deps_sysroot.join("usr/local")]
            .iter()
            .filter(|dir| dir.exists())
            .map(|dir| dir.display().to_string())
            .collect::<Vec<_>>()
            .join(":");
        if !cmake_prefix.is_empty() {
            env.insert("CMAKE_PREFIX_PATH".to_string(), cmake_prefix);
        }

        env
    }

    fn normalize_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    }

    fn find_dependency_recipe(dep_name: &str, spec_path: &Path) -> Option<PathBuf> {
        let package_dir = spec_path.parent()?;
        let release_dir = package_dir.parent()?;

        let mut candidates = HashSet::new();
        candidates.insert(Self::normalize_name(dep_name));
        if let Some(stripped) = dep_name.strip_suffix("-devel") {
            candidates.insert(Self::normalize_name(stripped));
        }
        if let Some(stripped) = dep_name.strip_suffix("-dev") {
            candidates.insert(Self::normalize_name(stripped));
        }
        if let Some(stripped) = dep_name.strip_suffix("-headers") {
            candidates.insert(Self::normalize_name(stripped));
        }

        let entries = fs::read_dir(release_dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                continue;
            }
            let dir_name = entry.file_name();
            let dir_str = dir_name.to_string_lossy();
            let normalized = Self::normalize_name(&dir_str);
            if candidates.contains(&normalized) {
                return Some(entry.path());
            }
        }

        None
    }

    fn find_recipe_spec(recipe_dir: &Path) -> Option<PathBuf> {
        let entries = fs::read_dir(recipe_dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("yaml")
                || path.extension().and_then(|ext| ext.to_str()) == Some("yml")
            {
                return Some(path);
            }
        }
        None
    }

    fn merge_env(target: &mut HashMap<String, String>, additions: &HashMap<String, String>) {
        for (key, value) in additions {
            if value.is_empty() {
                continue;
            }
            target
                .entry(key.clone())
                .and_modify(|existing| {
                    if existing.is_empty() {
                        *existing = value.clone();
                    } else {
                        let separator = if key.contains("PATH") && !key.contains("FLAGS") {
                            ":"
                        } else {
                            " "
                        };
                        existing.insert_str(0, separator);
                        existing.insert_str(0, value);
                    }
                })
                .or_insert(value.clone());
        }
    }

    fn sanitize_component(value: &str) -> String {
        let mut result = String::with_capacity(value.len());
        for ch in value.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                result.push(ch);
            } else {
                result.push('_');
            }
        }
        if result.is_empty() {
            "_".to_string()
        } else {
            result
        }
    }

    fn copy_directory_recursive(src: &Path, dest: &Path) -> Result<(), String> {
        for entry in WalkDir::new(src) {
            let entry = entry.map_err(|err| format!("WalkDir error: {}", err))?;
            let relative = entry
                .path()
                .strip_prefix(src)
                .map_err(|err| format!("Failed to determine relative path: {}", err))?;
            let target_path = dest.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target_path).map_err(|err| {
                    format!(
                        "Failed to create directory {}: {}",
                        target_path.display(),
                        err
                    )
                })?;
            } else {
                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent).map_err(|err| {
                        format!(
                            "Failed to create parent directory {}: {}",
                            parent.display(),
                            err
                        )
                    })?;
                }
                fs::copy(entry.path(), &target_path).map_err(|err| {
                    format!(
                        "Failed to copy {} to {}: {}",
                        entry.path().display(),
                        target_path.display(),
                        err
                    )
                })?;
            }
        }
        Ok(())
    }

    fn should_auto_build_dependency(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        lower.ends_with("-devel")
            || lower.ends_with("-dev")
            || lower.ends_with("-headers")
            || lower.ends_with("-sdk")
    }

    fn execute_build_steps(
        &self,
        spec: &PaxPackageSpec,
        source_dir: &Path,
        dependency_env: &HashMap<String, String>,
        build_log: &mut String,
    ) -> Result<(), String> {
        let mut build_env = spec.build.environment.clone();
        // Propagate host environment
        for (key, value) in std::env::vars() {
            build_env.entry(key).or_insert(value);
        }
        Self::merge_env(&mut build_env, dependency_env);

        let working_dir = if let Some(custom_dir) = &spec.build.working_directory {
            source_dir.join(custom_dir)
        } else {
            source_dir.to_path_buf()
        };

        for command in &spec.build.build_commands {
            build_log.push_str(&format!("Running build command: {}\n", command));
            let (stdout, stderr) = self.run_shell_command(command, &working_dir, &build_env)?;
            if !stdout.trim().is_empty() {
                build_log.push_str(&format!("stdout:\n{}\n", stdout));
            }
            if !stderr.trim().is_empty() {
                build_log.push_str(&format!("stderr:\n{}\n", stderr));
            }
        }

        Ok(())
    }

    fn execute_install_steps(
        &self,
        spec: &PaxPackageSpec,
        source_dir: &Path,
        destdir: &Path,
        dependency_env: &HashMap<String, String>,
        build_log: &mut String,
    ) -> Result<(), String> {
        let mut env = spec.build.environment.clone();
        env.insert("DESTDIR".to_string(), destdir.display().to_string());
        for (key, value) in std::env::vars() {
            env.entry(key).or_insert(value);
        }
        Self::merge_env(&mut env, dependency_env);

        if let Some(pre_install) = spec.scripts.pre_install.as_ref() {
            self.run_script_if_present("pre_install", pre_install, destdir, &env, build_log)?;
        }

        let working_dir = if let Some(custom_dir) = &spec.build.working_directory {
            source_dir.join(custom_dir)
        } else {
            source_dir.to_path_buf()
        };

        match spec.install.install_method {
            InstallMethod::RunCommands | InstallMethod::Custom | InstallMethod::ExtractArchive => {
                for dir in &spec.install.install_directories {
                    let path = destdir.join(dir.trim_start_matches('/'));
                    build_log.push_str(&format!("Ensuring directory exists: {}\n", path.display()));
                    fs::create_dir_all(&path).map_err(|err| {
                        format!(
                            "Failed to create install directory {}: {}",
                            path.display(),
                            err
                        )
                    })?;
                }

                for command in &spec.install.install_commands {
                    build_log.push_str(&format!("Running install command: {}\n", command));
                    let (stdout, stderr) = self.run_shell_command(command, &working_dir, &env)?;
                    if !stdout.trim().is_empty() {
                        build_log.push_str(&format!("stdout:\n{}\n", stdout));
                    }
                    if !stderr.trim().is_empty() {
                        build_log.push_str(&format!("stderr:\n{}\n", stderr));
                    }
                }
            }
            InstallMethod::CopyFiles => {
                for mapping in &spec.install.install_files {
                    let source = working_dir.join(&mapping.source);
                    let destination = destdir.join(&mapping.destination.trim_start_matches('/'));
                    build_log.push_str(&format!(
                        "Copying {} -> {}\n",
                        source.display(),
                        destination.display()
                    ));
                    if source.is_dir() {
                        fs::create_dir_all(&destination).map_err(|err| {
                            format!(
                                "Failed to create destination directory {}: {}",
                                destination.display(),
                                err
                            )
                        })?;
                        for entry in WalkDir::new(&source) {
                            let entry = entry.map_err(|err| format!("WalkDir error: {}", err))?;
                            let relative = entry.path().strip_prefix(&source).map_err(|err| {
                                format!("Failed to determine relative path: {}", err)
                            })?;
                            let dest_path = destination.join(relative);
                            if entry.file_type().is_dir() {
                                fs::create_dir_all(&dest_path).map_err(|err| {
                                    format!(
                                        "Failed to create directory {}: {}",
                                        dest_path.display(),
                                        err
                                    )
                                })?;
                            } else {
                                if let Some(parent) = dest_path.parent() {
                                    fs::create_dir_all(parent).map_err(|err| {
                                        format!(
                                            "Failed to create directory {}: {}",
                                            parent.display(),
                                            err
                                        )
                                    })?;
                                }
                                fs::copy(entry.path(), &dest_path).map_err(|err| {
                                    format!(
                                        "Failed to copy {} to {}: {}",
                                        entry.path().display(),
                                        dest_path.display(),
                                        err
                                    )
                                })?;
                            }
                        }
                    } else {
                        if let Some(parent) = destination.parent() {
                            fs::create_dir_all(parent).map_err(|err| {
                                format!("Failed to create directory {}: {}", parent.display(), err)
                            })?;
                        }
                        fs::copy(&source, &destination).map_err(|err| {
                            format!(
                                "Failed to copy {} to {}: {}",
                                source.display(),
                                destination.display(),
                                err
                            )
                        })?;
                    }
                    if let Some(permissions) = mapping.permissions {
                        fs::set_permissions(&destination, fs::Permissions::from_mode(permissions))
                            .map_err(|err| {
                                format!(
                                    "Failed to set permissions on {}: {}",
                                    destination.display(),
                                    err
                                )
                            })?;
                    }
                }
            }
        }

        for command in &spec.install.post_install_commands {
            build_log.push_str(&format!("Running post-install command: {}\n", command));
            let (stdout, stderr) = self.run_shell_command(command, destdir, &env)?;
            if !stdout.trim().is_empty() {
                build_log.push_str(&format!("stdout:\n{}\n", stdout));
            }
            if !stderr.trim().is_empty() {
                build_log.push_str(&format!("stderr:\n{}\n", stderr));
            }
        }

        if let Some(post_install) = spec.scripts.post_install.as_ref() {
            self.run_script_if_present("post_install", post_install, destdir, &env, build_log)?;
        }

        Ok(())
    }

    fn package_artifacts(
        &self,
        spec: &PaxPackageSpec,
        destdir: &Path,
        spec_path: &Path,
        build_log: &mut String,
        source_info: &SourcePreparation,
        package_name: &str,
        version: &str,
        release: &str,
        target_release: &str,
        branch: &str,
        arch_label: &str,
    ) -> Result<PackagedArtifacts, String> {
        let workspace = destdir
            .parent()
            .ok_or_else(|| "Failed to determine workspace directory".to_string())?;

        let safe_package = Self::sanitize_component(package_name);
        let safe_version = Self::sanitize_component(version);
        let safe_release = Self::sanitize_component(release);
        let safe_target_release = Self::sanitize_component(target_release);
        let safe_branch = Self::sanitize_component(branch);
        let safe_arch = Self::sanitize_component(arch_label);

        let base_output_dir = self
            .output_directory
            .join(&safe_target_release)
            .join(&safe_branch);
        let arch_output_dir = base_output_dir.join(&safe_arch);

        fs::create_dir_all(&arch_output_dir).map_err(|err| {
            format!(
                "Failed to create output directory {}: {}",
                arch_output_dir.display(),
                err
            )
        })?;

        let binary_filename = format!(
            "{}-{}-{}-{}.pax",
            safe_package, safe_version, safe_release, safe_arch
        );
        let binary_artifact_path = arch_output_dir.join(&binary_filename);

        build_log.push_str(&format!(
            "Packaging binary artifact {} from {}\n",
            binary_artifact_path.display(),
            destdir.display()
        ));

        let source_filename = format!("{}-{}-{}.src.pax", safe_package, safe_version, safe_release);

        let metadata_doc = json!({
            "package": {
                "name": package_name,
                "version": version,
                "release": release,
                "branch": branch,
                "target_release": target_release,
                "architecture": arch_label,
                "source_url": spec.source_url,
            },
            "artifacts": {
                "binary": binary_filename,
                "source": source_filename,
            },
        });
        let metadata_yaml = serde_yaml::to_string(&metadata_doc)
            .map_err(|err| format!("Failed to serialise metadata: {}", err))?;
        let metadata_json = serde_json::to_string_pretty(&metadata_doc)
            .map_err(|err| format!("Failed to serialise metadata JSON: {}", err))?;

        let metadata_yaml_path = workspace.join("metadata.yaml");
        let metadata_json_path = workspace.join("metadata.json");

        fs::write(&metadata_yaml_path, &metadata_yaml)
            .map_err(|err| format!("Failed to write metadata.yaml file: {}", err))?;
        fs::write(&metadata_json_path, &metadata_json)
            .map_err(|err| format!("Failed to write metadata.json file: {}", err))?;

        let metadata_bundle_dir = workspace.join("pax-metadata");
        if metadata_bundle_dir.exists() {
            fs::remove_dir_all(&metadata_bundle_dir).map_err(|err| {
                format!(
                    "Failed to reset metadata bundle directory {}: {}",
                    metadata_bundle_dir.display(),
                    err
                )
            })?;
        }
        fs::create_dir_all(&metadata_bundle_dir).map_err(|err| {
            format!(
                "Failed to create metadata bundle directory {}: {}",
                metadata_bundle_dir.display(),
                err
            )
        })?;
        fs::copy(
            &metadata_yaml_path,
            metadata_bundle_dir.join("metadata.yaml"),
        )
        .map_err(|err| {
            format!(
                "Failed to copy metadata.yaml into bundle {}: {}",
                metadata_bundle_dir.display(),
                err
            )
        })?;
        fs::copy(
            &metadata_json_path,
            metadata_bundle_dir.join("metadata.json"),
        )
        .map_err(|err| {
            format!(
                "Failed to copy metadata.json into bundle {}: {}",
                metadata_bundle_dir.display(),
                err
            )
        })?;

        let mut tar_command = Command::new("tar");
        tar_command
            .arg("-czf")
            .arg(&binary_artifact_path)
            .arg("-C")
            .arg(destdir)
            .arg(".");
        if metadata_bundle_dir.exists() {
            tar_command.arg("-C").arg(workspace).arg("pax-metadata");
        }

        let status = tar_command
            .status()
            .map_err(|err| format!("Failed to run tar: {}", err))?;
        if !status.success() {
            return Err(format!(
                "Failed to create binary artifact (exit code {:?})",
                status.code()
            ));
        }

        let source_artifact_path = arch_output_dir.join(&source_filename);

        let source_staging = workspace.join("src-package");
        if source_staging.exists() {
            fs::remove_dir_all(&source_staging).map_err(|err| {
                format!(
                    "Failed to reset source staging directory {}: {}",
                    source_staging.display(),
                    err
                )
            })?;
        }
        fs::create_dir_all(&source_staging).map_err(|err| {
            format!(
                "Failed to create source staging directory {}: {}",
                source_staging.display(),
                err
            )
        })?;

        fs::copy(&metadata_yaml_path, source_staging.join("metadata.yaml")).map_err(|err| {
            format!(
                "Failed to copy metadata into source package {}: {}",
                source_staging.display(),
                err
            )
        })?;
        fs::copy(&metadata_json_path, source_staging.join("metadata.json")).map_err(|err| {
            format!(
                "Failed to copy metadata JSON into source package {}: {}",
                source_staging.display(),
                err
            )
        })?;

        let spec_filename = spec_path
            .file_name()
            .map(|name| name.to_owned())
            .unwrap_or_else(|| std::ffi::OsStr::new("recipe.yaml").to_owned());
        fs::copy(spec_path, source_staging.join(spec_filename)).map_err(|err| {
            format!(
                "Failed to copy specification into source package {}: {}",
                source_staging.display(),
                err
            )
        })?;

        if let Some(archive) = &source_info.archive_path {
            let archive_name = archive
                .file_name()
                .ok_or_else(|| "Unable to determine source archive filename".to_string())?;
            fs::copy(archive, source_staging.join(archive_name)).map_err(|err| {
                format!(
                    "Failed to copy source archive into source package {}: {}",
                    source_staging.display(),
                    err
                )
            })?;
        } else {
            let source_tree = source_staging.join("sources");
            Self::copy_directory_recursive(&source_info.source_dir, &source_tree)?;
        }

        build_log.push_str(&format!(
            "Packaging source artifact {} from {}\n",
            source_artifact_path.display(),
            source_staging.display()
        ));

        let status = Command::new("tar")
            .arg("-czf")
            .arg(&source_artifact_path)
            .arg("-C")
            .arg(&source_staging)
            .arg(".")
            .status()
            .map_err(|err| format!("Failed to package source artifact: {}", err))?;
        if !status.success() {
            return Err(format!(
                "Failed to create source artifact (exit code {:?})",
                status.code()
            ));
        }

        fs::remove_dir_all(&source_staging).map_err(|err| {
            format!(
                "Failed to clean source staging directory {}: {}",
                source_staging.display(),
                err
            )
        })?;

        if let Ok(job_results_dir) = std::env::var("PAX_JOB_RESULTS_DIR") {
            let job_base = PathBuf::from(job_results_dir)
                .join(&safe_target_release)
                .join(&safe_branch);
            let job_arch_dir = job_base.join(&safe_arch);
            fs::create_dir_all(&job_arch_dir).map_err(|err| {
                format!(
                    "Failed to create job artifact directory {}: {}",
                    job_arch_dir.display(),
                    err
                )
            })?;

            let binary_dest = job_arch_dir.join(&binary_filename);
            if binary_artifact_path != binary_dest {
                if let Err(err) = fs::copy(&binary_artifact_path, &binary_dest) {
                    eprintln!(
                        "WARNING: Failed to copy binary artifact into job results {}: {}",
                        binary_dest.display(),
                        err
                    );
                }
            }
            let source_dest = job_arch_dir.join(&source_artifact_path);
            if source_artifact_path != source_dest {
                if let Err(err) = fs::copy(&source_artifact_path, &source_dest) {
                    eprintln!(
                        "WARNING: Failed to copy source artifact into job results {}: {}",
                        source_dest.display(),
                        err
                    );
                }
            }

            let _ = fs::copy(&metadata_yaml_path, job_arch_dir.join("metadata.yaml"));
            let _ = fs::copy(&metadata_json_path, job_arch_dir.join("metadata.json"));
            let job_metadata_dir = job_arch_dir.join("pax-metadata");
            if let Err(err) = fs::create_dir_all(&job_metadata_dir) {
                eprintln!(
                    "WARNING: Failed to create pax-metadata directory in job results {}: {}",
                    job_metadata_dir.display(),
                    err
                );
            } else {
                let _ = fs::copy(
                    metadata_bundle_dir.join("metadata.yaml"),
                    job_metadata_dir.join("metadata.yaml"),
                );
                let _ = fs::copy(
                    metadata_bundle_dir.join("metadata.json"),
                    job_metadata_dir.join("metadata.json"),
                );
            }
        }

        if let Ok(mirror_root) = std::env::var("PAX_RESULTS_MIRROR") {
            let mirror_base = PathBuf::from(&mirror_root)
                .join(&safe_target_release)
                .join(&safe_branch);
            let mirror_arch_dir = mirror_base.join(&safe_arch);
            if let Err(err) = fs::create_dir_all(&mirror_arch_dir) {
                eprintln!(
                    "WARNING: Failed to create mirror artifact directory {}: {}",
                    mirror_arch_dir.display(),
                    err
                );
            } else {
                let mirror_binary = mirror_arch_dir.join(&binary_filename);
                if mirror_binary != binary_artifact_path {
                    if let Err(err) = fs::copy(&binary_artifact_path, &mirror_binary) {
                        eprintln!(
                            "WARNING: Failed to mirror binary artifact into {}: {}",
                            mirror_binary.display(),
                            err
                        );
                    }
                }
                let mirror_source = mirror_arch_dir.join(&source_filename);
                if mirror_source != source_artifact_path {
                    if let Err(err) = fs::copy(&source_artifact_path, &mirror_source) {
                        eprintln!(
                            "WARNING: Failed to mirror source artifact into {}: {}",
                            mirror_source.display(),
                            err
                        );
                    }
                }
                let _ = fs::copy(&metadata_yaml_path, mirror_arch_dir.join("metadata.yaml"));
                let _ = fs::copy(&metadata_json_path, mirror_arch_dir.join("metadata.json"));
            }
        }

        let _ = fs::remove_file(&metadata_yaml_path);
        let _ = fs::remove_file(&metadata_json_path);
        let _ = fs::remove_dir_all(&metadata_bundle_dir);

        build_log.push_str(&format!(
            "Binary artifact written to {}\nSource artifact written to {}\n",
            binary_artifact_path.display(),
            source_artifact_path.display()
        ));

        Ok(PackagedArtifacts {
            binary_artifact: binary_artifact_path,
            source_artifact: source_artifact_path,
        })
    }

    fn run_shell_command(
        &self,
        command: &str,
        cwd: &Path,
        env: &HashMap<String, String>,
    ) -> Result<(String, String), String> {
        let child = Command::new("bash")
            .arg("-lc")
            .arg(command)
            .current_dir(cwd)
            .envs(env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("Failed to spawn command '{}': {}", command, err))?;

        let output = child
            .wait_with_output()
            .map_err(|err| format!("Failed to wait for command '{}': {}", command, err))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(format!(
                "Command '{}' failed with exit code {:?}\nstdout:\n{}\nstderr:\n{}",
                command,
                output.status.code(),
                stdout,
                stderr
            ));
        }

        Ok((stdout, stderr))
    }

    fn run_script_if_present(
        &self,
        label: &str,
        script: &str,
        cwd: &Path,
        env: &HashMap<String, String>,
        build_log: &mut String,
    ) -> Result<(), String> {
        build_log.push_str(&format!("Running script {}: {}\n", label, script));
        let (stdout, stderr) = self.run_shell_command(script, cwd, env)?;
        if !stdout.trim().is_empty() {
            build_log.push_str(&format!("stdout:\n{}\n", stdout));
        }
        if !stderr.trim().is_empty() {
            build_log.push_str(&format!("stderr:\n{}\n", stderr));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct BuildStats {
    pub build_directory: PathBuf,
    pub output_directory: PathBuf,
    pub temp_directory: PathBuf,
}

impl Default for PaxPackageBuilder {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to a basic configuration
            Self {
                build_directory: PathBuf::from("/tmp/pax-build"),
                output_directory: PathBuf::from("/tmp/pax-output"),
                temp_directory: PathBuf::from("/tmp/pax-temp"),
                verbose: false,
                target_arch: None,
                use_bubblewrap: true,
                buildroot_directory: PathBuf::from("/tmp/pax-buildroot"),
                host_arch: "x86_64".to_string(),
                allow_dependency_builds: true,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_target_arch_from_str() {
        assert_eq!(TargetArch::from_str("x86_64"), Some(TargetArch::X86_64));
        assert_eq!(TargetArch::from_str("x86_64v1"), Some(TargetArch::X86_64v1));
        assert_eq!(
            TargetArch::from_str("x86_64_v1"),
            Some(TargetArch::X86_64v1)
        );
        assert_eq!(TargetArch::from_str("aarch64"), Some(TargetArch::Aarch64));
        assert_eq!(TargetArch::from_str("arm64"), Some(TargetArch::Aarch64));
        assert_eq!(TargetArch::from_str("armv7l"), Some(TargetArch::Armv7l));
        assert_eq!(TargetArch::from_str("riscv64"), Some(TargetArch::Riscv64));
        assert_eq!(TargetArch::from_str("invalid"), None);
    }

    #[test]
    fn test_target_arch_to_triple() {
        assert_eq!(TargetArch::X86_64.to_triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(TargetArch::X86_64v1.to_triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(TargetArch::Aarch64.to_triple(), "aarch64-unknown-linux-gnu");
        assert_eq!(
            TargetArch::Armv7l.to_triple(),
            "armv7-unknown-linux-gnueabihf"
        );
        assert_eq!(
            TargetArch::Riscv64.to_triple(),
            "riscv64gc-unknown-linux-gnu"
        );
    }

    #[test]
    fn test_target_arch_cross_compiler_prefix() {
        assert_eq!(
            TargetArch::X86_64.cross_compiler_prefix(),
            "x86_64-linux-gnu-"
        );
        assert_eq!(
            TargetArch::Aarch64.cross_compiler_prefix(),
            "aarch64-linux-gnu-"
        );
        assert_eq!(
            TargetArch::Armv7l.cross_compiler_prefix(),
            "arm-linux-gnueabihf-"
        );
        assert_eq!(
            TargetArch::Riscv64.cross_compiler_prefix(),
            "riscv64-linux-gnu-"
        );
    }

    #[test]
    fn test_package_spec_validation() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("test.pax.yaml");

        // Test valid spec
        let valid_spec = r#"
name: test-package
version: "1.0.0"
description: "Test package"
author: "Test Author"
license: "MIT"
keywords: []
categories: []
dependencies:
  build_dependencies: []
  runtime_dependencies: []
  optional_dependencies: []
  conflicts: []
build:
  build_system: Make
  build_commands:
    - "make"
  build_dependencies: []
  build_flags: []
  environment: {}
  working_directory: null
  target_architectures:
    - X86_64v1
  cross_compiler_prefix: null
  target_sysroot: null
install:
  install_method: RunCommands
  install_commands:
    - "make install"
  install_directories: []
  install_files: []
  post_install_commands: []
files:
  include_patterns: []
  exclude_patterns: []
  binary_files: []
  config_files: []
  documentation_files: []
  license_files: []
scripts:
  pre_install: null
  post_install: null
  pre_uninstall: null
  post_uninstall: null
  pre_upgrade: null
  post_upgrade: null
metadata: {}
"#;

        fs::write(&spec_path, valid_spec).unwrap();
        let builder = PaxPackageBuilder::default();
        let errors = builder.validate_spec(&spec_path).unwrap();
        assert!(errors.is_empty());

        // Test invalid spec - missing name
        let invalid_spec = r#"
version: "1.0.0"
description: "Test package"
author: "Test Author"
license: "MIT"
keywords: []
categories: []
dependencies:
  build_dependencies: []
  runtime_dependencies: []
  optional_dependencies: []
  conflicts: []
build:
  build_system: Make
  build_commands:
    - "make"
  build_dependencies: []
  build_flags: []
  environment: {}
  working_directory: null
  target_architectures:
    - X86_64v1
  cross_compiler_prefix: null
  target_sysroot: null
install:
  install_method: RunCommands
  install_commands:
    - "make install"
  install_directories: []
  install_files: []
  post_install_commands: []
files:
  include_patterns: []
  exclude_patterns: []
  binary_files: []
  config_files: []
  documentation_files: []
  license_files: []
scripts:
  pre_install: null
  post_install: null
  pre_uninstall: null
  post_uninstall: null
  pre_upgrade: null
  post_upgrade: null
metadata: {}
"#;

        fs::write(&spec_path, invalid_spec).unwrap();
        let errors = builder.validate_spec(&spec_path).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.contains(&"Package name is required".to_string()));
    }

    #[test]
    fn test_spec_validation_missing_build_commands() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("test.pax.yaml");

        let invalid_spec = r#"
name: test-package
version: "1.0.0"
description: "Test package"
author: "Test Author"
license: "MIT"
keywords: []
categories: []
dependencies:
  build_dependencies: []
  runtime_dependencies: []
  optional_dependencies: []
  conflicts: []
build:
  build_system: Make
  build_commands: []
  build_dependencies: []
  build_flags: []
  environment: {}
  working_directory: null
  target_architectures:
    - X86_64v1
  cross_compiler_prefix: null
  target_sysroot: null
install:
  install_method: RunCommands
  install_commands:
    - "make install"
  install_directories: []
  install_files: []
  post_install_commands: []
files:
  include_patterns: []
  exclude_patterns: []
  binary_files: []
  config_files: []
  documentation_files: []
  license_files: []
scripts:
  pre_install: null
  post_install: null
  pre_uninstall: null
  post_uninstall: null
  pre_upgrade: null
  post_upgrade: null
metadata: {}
"#;

        fs::write(&spec_path, invalid_spec).unwrap();
        let builder = PaxPackageBuilder::default();
        let errors = builder.validate_spec(&spec_path).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.contains(&"At least one build command is required".to_string()));
    }

    #[test]
    fn test_spec_validation_copy_files_missing_files() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("test.pax.yaml");

        let invalid_spec = r#"
name: test-package
version: "1.0.0"
description: "Test package"
author: "Test Author"
license: "MIT"
keywords: []
categories: []
dependencies:
  build_dependencies: []
  runtime_dependencies: []
  optional_dependencies: []
  conflicts: []
build:
  build_system: Make
  build_commands:
    - "make"
  build_dependencies: []
  build_flags: []
  environment: {}
  working_directory: null
  target_architectures:
    - X86_64v1
  cross_compiler_prefix: null
  target_sysroot: null
install:
  install_method: CopyFiles
  install_commands: []
  install_directories: []
  install_files: []
  post_install_commands: []
files:
  include_patterns: []
  exclude_patterns: []
  binary_files: []
  config_files: []
  documentation_files: []
  license_files: []
scripts:
  pre_install: null
  post_install: null
  pre_uninstall: null
  post_uninstall: null
  pre_upgrade: null
  post_upgrade: null
metadata: {}
"#;

        fs::write(&spec_path, invalid_spec).unwrap();
        let builder = PaxPackageBuilder::default();
        let errors = builder.validate_spec(&spec_path).unwrap();
        assert!(!errors.is_empty());
        assert!(errors.contains(&"Install files are required for CopyFiles method".to_string()));
    }

    #[test]
    fn test_spec_validation_run_commands_missing_commands() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("test.pax.yaml");

        let invalid_spec = r#"
name: test-package
version: "1.0.0"
description: "Test package"
author: "Test Author"
license: "MIT"
keywords: []
categories: []
dependencies:
  build_dependencies: []
  runtime_dependencies: []
  optional_dependencies: []
  conflicts: []
build:
  build_system: Make
  build_commands:
    - "make"
  build_dependencies: []
  build_flags: []
  environment: {}
  working_directory: null
  target_architectures:
    - X86_64v1
  cross_compiler_prefix: null
  target_sysroot: null
install:
  install_method: RunCommands
  install_commands: []
  install_directories: []
  install_files: []
  post_install_commands: []
files:
  include_patterns: []
  exclude_patterns: []
  binary_files: []
  config_files: []
  documentation_files: []
  license_files: []
scripts:
  pre_install: null
  post_install: null
  pre_uninstall: null
  post_uninstall: null
  pre_upgrade: null
  post_upgrade: null
metadata: {}
"#;

        fs::write(&spec_path, invalid_spec).unwrap();
        let builder = PaxPackageBuilder::default();
        let errors = builder.validate_spec(&spec_path).unwrap();
        assert!(!errors.is_empty());
        assert!(
            errors.contains(&"Install commands are required for RunCommands method".to_string())
        );
    }

    #[test]
    fn test_calculate_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let test_content = b"Hello, World!";
        fs::write(&test_file, test_content).unwrap();

        let builder = PaxPackageBuilder::default();
        let checksum = builder.calculate_checksum(&test_file).unwrap();

        // SHA256 of "Hello, World!"
        let expected = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
        assert_eq!(checksum, expected);
    }

    #[test]
    fn test_clean_build_directory() {
        let temp_dir = TempDir::new().unwrap();
        let build_dir = temp_dir.path().join("build");
        fs::create_dir_all(&build_dir).unwrap();
        let test_file = build_dir.join("test.txt");
        fs::write(&test_file, "test").unwrap();

        // Create a builder with the temp directory as build directory
        let mut builder = PaxPackageBuilder {
            build_directory: build_dir.clone(),
            output_directory: temp_dir.path().join("output"),
            temp_directory: temp_dir.path().join("temp"),
            verbose: false,
            target_arch: None,
            use_bubblewrap: true,
            buildroot_directory: temp_dir.path().join("buildroot"),
            host_arch: "x86_64".to_string(),
        };

        assert!(build_dir.exists());
        assert!(test_file.exists());

        builder.clean_build_directory().unwrap();

        assert!(!build_dir.exists());
        assert!(!test_file.exists());
    }

    #[test]
    fn test_with_target_arch_valid() {
        let builder = PaxPackageBuilder::default();
        let result = builder.with_target_arch(TargetArch::X86_64v1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_with_target_arch_invalid() {
        let builder = PaxPackageBuilder {
            build_directory: PathBuf::from("/tmp/build"),
            output_directory: PathBuf::from("/tmp/output"),
            temp_directory: PathBuf::from("/tmp/temp"),
            verbose: false,
            target_arch: None,
            use_bubblewrap: true,
            buildroot_directory: PathBuf::from("/tmp/buildroot"),
            host_arch: "armv7l".to_string(), // Use armv7l host which doesn't support x86_64
        };

        // Try to set x86_64 target on armv7l host (should fail)
        let result = builder.with_target_arch(TargetArch::X86_64v1);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("not supported on host architecture"));
    }

    #[test]
    fn test_with_bubblewrap() {
        let builder = PaxPackageBuilder::default();
        let builder_with_bwrap = builder.with_bubblewrap(false);
        // This is a configuration test - we can't easily test the internal state
        // but we can verify the method exists and returns a builder
        assert_eq!(builder_with_bwrap.use_bubblewrap, false);
    }

    #[test]
    fn test_with_verbose() {
        let builder = PaxPackageBuilder::default();
        let builder_verbose = builder.with_verbose(true);
        assert_eq!(builder_verbose.verbose, true);
    }

    #[test]
    fn test_get_build_stats() {
        let builder = PaxPackageBuilder::default();
        let stats = builder.get_build_stats();

        assert_eq!(stats.build_directory, builder.build_directory);
        assert_eq!(stats.output_directory, builder.output_directory);
        assert_eq!(stats.temp_directory, builder.temp_directory);
    }
}
