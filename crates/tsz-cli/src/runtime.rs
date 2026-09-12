use std::{
    fmt::{self, Display},
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    str::FromStr,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const APP_IMAGE: &str = "ghcr.io/zakura-core/thus-spoke-zakura-app:0.1.0";
const ZAKURA_IMAGE: &str = "zakuracore/zakura:1.2.0";
const LIGHTWALLETD_IMAGE: &str = "ghcr.io/zakura-core/thus-spoke-zakura-lightwalletd:0.1.0";

#[derive(Clone, Debug)]
pub struct InstanceName(String);

impl Display for InstanceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for InstanceName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let valid = !value.is_empty()
            && value.len() <= 40
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid || value.starts_with('-') || value.ends_with('-') {
            bail!("instance names use 1-40 lowercase letters, digits, or internal hyphens");
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Endpoints {
    pub dashboard: String,
    pub rpc: String,
    pub lightwalletd: String,
    pub p2p: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Instance {
    name: String,
    version: u32,
    endpoints: Endpoints,
}

pub struct Runtime {
    root: PathBuf,
}

impl Runtime {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "zakura", "thus-spoke-zakura")
            .ok_or_else(|| anyhow!("could not determine the platform configuration directory"))?;
        Ok(Self {
            root: dirs.config_dir().to_owned(),
        })
    }

    pub fn doctor(&self, json: bool) -> Result<()> {
        let docker = docker_output(["version", "--format", "{{.Server.Version}}"]);
        let result = serde_json::json!({
            "docker": docker.as_ref().ok(),
            "config_dir": self.root,
            "ok": docker.is_ok(),
        });
        if json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else if let Ok(version) = docker {
            println!("✓ Docker {version}\n✓ Config: {}", self.root.display());
        } else {
            bail!("Docker is not reachable; start Docker Desktop or the Docker daemon");
        }
        Ok(())
    }

    pub fn start(
        &self,
        name: &InstanceName,
        no_open: bool,
        build: bool,
        build_dev: bool,
        json: bool,
    ) -> Result<()> {
        self.doctor(false)?;
        if build || build_dev {
            build_project_images(build_dev)?;
            recreate_project_containers(&prefix(name))?;
        }
        for image in [APP_IMAGE, ZAKURA_IMAGE, LIGHTWALLETD_IMAGE] {
            ensure_image(image)?;
        }
        fs::create_dir_all(self.instance_dir(name))?;
        let prefix = prefix(name);
        ensure_network(&prefix)?;
        for suffix in ["chain", "wallet", "lightwalletd", "config"] {
            ensure_volume(&format!("{prefix}-{suffix}"), name)?;
        }

        println!("Starting {name}…");
        if !container_exists(&format!("{prefix}-init"))? {
            docker([
                "create",
                "--name",
                &format!("{prefix}-init"),
                "--label",
                &label(name),
                "-v",
                &format!("{prefix}-wallet:/data"),
                "-v",
                &format!("{prefix}-config:/config"),
                APP_IMAGE,
                "init",
                "--data-dir",
                "/data",
                "--config-dir",
                "/config",
            ])?;
            docker(["start", "-a", &format!("{prefix}-init")])?;
        }

        ensure_zakura(&prefix, name)?;
        ensure_lightwalletd(&prefix, name)?;
        ensure_app(&prefix, name)?;
        for service in ["zakura", "lightwalletd", "app"] {
            docker(["start", &format!("{prefix}-{service}")])?;
        }
        let endpoints = inspect_endpoints(&prefix)?;
        self.write_instance(name, &endpoints)?;
        wait_ready(
            &endpoints.dashboard,
            &format!("{prefix}-app"),
            Duration::from_secs(120),
        )?;
        if json {
            println!("{}", serde_json::to_string_pretty(&endpoints)?);
        } else {
            print_endpoints(name, &endpoints);
        }
        if !no_open {
            open_url(&endpoints.dashboard)?;
        }
        Ok(())
    }

    pub fn status(&self, name: &InstanceName, json: bool) -> Result<()> {
        let endpoints = inspect_endpoints(&prefix(name))
            .or_else(|_| self.read_instance(name).map(|i| i.endpoints))?;
        let running = container_running(&format!("{}-app", prefix(name))).unwrap_or(false);
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"name": name.to_string(), "running": running, "endpoints": endpoints})
                )?
            );
        } else {
            println!("{}: {}", name, if running { "running" } else { "stopped" });
            print_endpoints(name, &endpoints);
        }
        Ok(())
    }

    pub fn endpoints(&self, name: &InstanceName, json: bool) -> Result<()> {
        let endpoints = self.read_instance(name)?.endpoints;
        if json {
            println!("{}", serde_json::to_string_pretty(&endpoints)?);
        } else {
            print_endpoints(name, &endpoints);
        }
        Ok(())
    }

    pub fn open(&self, name: &InstanceName) -> Result<()> {
        open_url(&self.read_instance(name)?.endpoints.dashboard)
    }

    pub fn logs(&self, name: &InstanceName, service: Option<&str>, follow: bool) -> Result<()> {
        let service = service.unwrap_or("app");
        let mut args = vec!["logs"];
        if follow {
            args.push("--follow");
        }
        let container = format!("{}-{service}", prefix(name));
        args.push(&container);
        docker_inherit(&args)
    }

    pub fn stop(&self, name: &InstanceName) -> Result<()> {
        let prefix = prefix(name);
        for service in ["app", "lightwalletd", "zakura"] {
            if container_exists(&format!("{prefix}-{service}"))? {
                docker(["stop", &format!("{prefix}-{service}")])?;
            }
        }
        println!("Stopped {name}; its data is preserved.");
        Ok(())
    }

    pub fn reset(&self, name: &InstanceName, force: bool) -> Result<()> {
        if !force {
            bail!("reset deletes chain, wallet, and seed data; repeat with --force");
        }
        let prefix = prefix(name);
        for service in ["app", "lightwalletd", "zakura", "init"] {
            let target = format!("{prefix}-{service}");
            if container_exists(&target)? {
                docker(["rm", "-f", &target])?;
            }
        }
        for suffix in ["chain", "wallet", "lightwalletd", "config"] {
            let _ = docker(["volume", "rm", &format!("{prefix}-{suffix}")]);
        }
        let _ = docker(["network", "rm", &prefix]);
        let dir = self.instance_dir(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
        }
        println!("Deleted {name}; its Docker volumes cannot be recovered.");
        Ok(())
    }

    pub fn list(&self, json: bool) -> Result<()> {
        let mut instances = Vec::new();
        if self.root.exists() {
            for entry in fs::read_dir(&self.root)? {
                let path = entry?.path().join("instance.json");
                if path.exists() {
                    instances.push(serde_json::from_slice::<Instance>(&fs::read(path)?)?);
                }
            }
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&instances)?);
        } else if instances.is_empty() {
            println!("No environments yet.");
        } else {
            for i in instances {
                println!("{:<20} {}", i.name, i.endpoints.dashboard);
            }
        }
        Ok(())
    }

    fn instance_dir(&self, name: &InstanceName) -> PathBuf {
        self.root.join(name.to_string())
    }
    fn write_instance(&self, name: &InstanceName, endpoints: &Endpoints) -> Result<()> {
        let instance = Instance {
            name: name.to_string(),
            version: 1,
            endpoints: endpoints.clone(),
        };
        fs::write(
            self.instance_dir(name).join("instance.json"),
            serde_json::to_vec_pretty(&instance)?,
        )?;
        Ok(())
    }
    fn read_instance(&self, name: &InstanceName) -> Result<Instance> {
        let path = self.instance_dir(name).join("instance.json");
        serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("instance {name} does not exist"))?,
        )
        .context("invalid instance metadata")
    }
}

fn prefix(name: &InstanceName) -> String {
    format!("tsz-{name}")
}
fn label(name: &InstanceName) -> String {
    format!("com.zakura.tsz.instance={name}")
}

fn ensure_network(prefix: &str) -> Result<()> {
    if docker_output(["network", "inspect", prefix]).is_err() {
        docker(["network", "create", prefix])?;
    }
    Ok(())
}
fn ensure_volume(volume: &str, name: &InstanceName) -> Result<()> {
    if docker_output(["volume", "inspect", volume]).is_err() {
        docker(["volume", "create", "--label", &label(name), volume])?;
    }
    Ok(())
}
fn ensure_zakura(prefix: &str, name: &InstanceName) -> Result<()> {
    let target = format!("{prefix}-zakura");
    if !container_exists(&target)? {
        docker([
            "create",
            "--name",
            &target,
            "--network",
            prefix,
            "--network-alias",
            "zakura",
            "--label",
            &label(name),
            "-p",
            "127.0.0.1::18232",
            "-p",
            "127.0.0.1::18233",
            "-v",
            &format!("{prefix}-chain:/data"),
            "-v",
            &format!("{prefix}-config:/config:ro"),
            "-e",
            "CONFIG_FILE_PATH=/config/zakurad.toml",
            ZAKURA_IMAGE,
            "zakurad",
            "start",
        ])?;
    }
    Ok(())
}
fn ensure_lightwalletd(prefix: &str, name: &InstanceName) -> Result<()> {
    let target = format!("{prefix}-lightwalletd");
    if !container_exists(&target)? {
        docker([
            "create",
            "--name",
            &target,
            "--network",
            prefix,
            "--network-alias",
            "lightwalletd",
            "--label",
            &label(name),
            "--user",
            "0:0",
            "-p",
            "127.0.0.1::9067",
            "-v",
            &format!("{prefix}-lightwalletd:/var/lib/lightwalletd"),
            LIGHTWALLETD_IMAGE,
            "--no-tls-very-insecure",
            "--grpc-bind-addr",
            "0.0.0.0:9067",
            "--rpchost",
            "zakura",
            "--rpcport",
            "18232",
            "--rpcuser",
            "unused",
            "--rpcpassword",
            "unused",
            "--data-dir",
            "/var/lib/lightwalletd",
            "--log-file",
            "/dev/stdout",
        ])?;
    }
    Ok(())
}
fn ensure_app(prefix: &str, name: &InstanceName) -> Result<()> {
    let target = format!("{prefix}-app");
    if !container_exists(&target)? {
        docker([
            "create",
            "--name",
            &target,
            "--network",
            prefix,
            "--label",
            &label(name),
            "-p",
            "127.0.0.1::8080",
            "-e",
            "TSZ_LISTEN=0.0.0.0:8080",
            "-e",
            "TSZ_ZAKURA_RPC=http://zakura:18232",
            "-e",
            "TSZ_LIGHTWALLETD=http://lightwalletd:9067",
            "-e",
            &format!("TSZ_INSTANCE={name}"),
            "-v",
            &format!("{prefix}-wallet:/data"),
            APP_IMAGE,
            "serve",
            "--data-dir",
            "/data",
        ])?;
    }
    Ok(())
}

fn inspect_endpoints(prefix: &str) -> Result<Endpoints> {
    Ok(Endpoints {
        dashboard: format!(
            "http://127.0.0.1:{}",
            published_port(&format!("{prefix}-app"), "8080/tcp")?
        ),
        rpc: format!(
            "http://127.0.0.1:{}",
            published_port(&format!("{prefix}-zakura"), "18232/tcp")?
        ),
        lightwalletd: format!(
            "http://127.0.0.1:{}",
            published_port(&format!("{prefix}-lightwalletd"), "9067/tcp")?
        ),
        p2p: format!(
            "127.0.0.1:{}",
            published_port(&format!("{prefix}-zakura"), "18233/tcp")?
        ),
    })
}
fn published_port(container: &str, port: &str) -> Result<u16> {
    docker_output([
        "inspect",
        "--format",
        &format!("{{{{(index (index .NetworkSettings.Ports \"{port}\") 0).HostPort}}}}"),
        container,
    ])?
    .parse()
    .context("Docker returned an invalid published port")
}
fn container_exists(name: &str) -> Result<bool> {
    Ok(docker_output(["container", "inspect", name]).is_ok())
}
fn ensure_image(image: &str) -> Result<()> {
    if docker_output(["image", "inspect", image]).is_err() {
        println!("Pulling {image}…");
        docker(["pull", image])?;
    }
    Ok(())
}
fn build_project_images(dev: bool) -> Result<()> {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    if !project_root.join("Dockerfile").is_file()
        || !project_root
            .join("docker/lightwalletd.Dockerfile")
            .is_file()
    {
        bail!(
            "cannot build images: project source is unavailable at {}",
            project_root.display()
        );
    }

    if dev {
        println!("Building {APP_IMAGE} with the Rust development profile…");
        docker_inherit_in(
            &[
                "build",
                "--build-arg",
                "RUST_PROFILE=dev-runtime",
                "-t",
                APP_IMAGE,
                ".",
            ],
            &project_root,
        )?;
    } else {
        println!("Building {APP_IMAGE}…");
        docker_inherit_in(&["build", "-t", APP_IMAGE, "."], &project_root)?;
    }
    println!("Building {LIGHTWALLETD_IMAGE}…");
    docker_inherit_in(
        &[
            "build",
            "-f",
            "docker/lightwalletd.Dockerfile",
            "-t",
            LIGHTWALLETD_IMAGE,
            ".",
        ],
        &project_root,
    )
}
fn recreate_project_containers(prefix: &str) -> Result<()> {
    for service in ["app", "lightwalletd"] {
        let target = format!("{prefix}-{service}");
        if container_exists(&target)? {
            println!("Recreating {target} with the new image…");
            docker(["rm", "-f", &target])?;
        }
    }
    Ok(())
}
fn container_running(name: &str) -> Result<bool> {
    Ok(docker_output([
        "container",
        "inspect",
        "--format",
        "{{.State.Running}}",
        name,
    ])? == "true")
}
fn wait_ready(base: &str, app_container: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if Command::new("curl")
            .args(["-fsS", &format!("{base}/api/v1/health")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
        {
            return Ok(());
        }
        if !container_running(app_container).unwrap_or(false) {
            let logs = docker_logs(app_container)
                .unwrap_or_else(|error| format!("could not read app logs: {error}"));
            bail!("app exited before becoming healthy:\n{logs}");
        }
        thread::sleep(Duration::from_millis(750));
    }
    bail!(
        "dashboard did not become healthy within {} seconds",
        timeout.as_secs()
    )
}
fn print_endpoints(name: &InstanceName, e: &Endpoints) {
    println!(
        "\n{name} is ready 🌸\n  Dashboard    {}\n  Zakura RPC   {}\n  lightwalletd {}\n  P2P          {}",
        e.dashboard, e.rpc, e.lightwalletd, e.p2p
    );
}
fn open_url(url: &str) -> Result<()> {
    let (program, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else {
        ("xdg-open", vec![url])
    };
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("opening {url}"))?;
    Ok(())
}
fn docker<const N: usize>(args: [&str; N]) -> Result<()> {
    docker_inherit(&args)
}
fn docker_inherit(args: &[&str]) -> Result<()> {
    docker_command(args, None)
}
fn docker_inherit_in(args: &[&str], current_dir: &std::path::Path) -> Result<()> {
    docker_command(args, Some(current_dir))
}
fn docker_command(args: &[&str], current_dir: Option<&std::path::Path>) -> Result<()> {
    let mut command = Command::new("docker");
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let status = command.status().context("running Docker")?;
    if !status.success() {
        bail!("docker {} failed", args.join(" "));
    }
    Ok(())
}
fn docker_output<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .context("running Docker")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
fn docker_logs(container: &str) -> Result<String> {
    let output = Command::new("docker")
        .args(["logs", "--tail", "50", container])
        .output()
        .context("running Docker")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    let mut logs = output.stdout;
    logs.extend_from_slice(&output.stderr);
    Ok(String::from_utf8_lossy(&logs).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_instance_names() {
        for valid in ["default", "project-2", "a"] {
            assert!(valid.parse::<InstanceName>().is_ok());
        }
        for invalid in ["", "UPPER", "with space", "-start", "end-"] {
            assert!(invalid.parse::<InstanceName>().is_err());
        }
    }
}
