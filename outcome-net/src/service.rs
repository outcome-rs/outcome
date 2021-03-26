use std::io::Read;
use std::process;
use std::time::{Duration, Instant};

use outcome::model::ServiceModel;
use outcome::Result;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Managed client connected to local or remote server.
///
/// # Managed service
///
/// Service is monitored by it's parent process, who's collecting output and
/// metrics, and checking if the service is alive. If a service process
/// crashes, there will be an attempt to restart it.
pub struct Service {
    /// Project-wide unique name of the service
    pub name: String,
    /// Path to service binary
    pub bin_path: PathBuf,
    args: Vec<String>,

    /// Handle to the child process
    pub handle: std::process::Child,

    /// Spawn time of last service instance
    started_at: Instant,
    /// Address of the service client
    address: Option<SocketAddr>,

    /// Cumulative log for stdout
    pub std_out_log: String,
}

// TODO support compiling rust services from path to src using cargo
impl Service {
    pub fn start_from_model(model: ServiceModel) -> Result<Self> {
        let bin_path = if let Some(executable_path) = &model.executable {
            executable_path
        } else if let Some(src_path) = model.project {
            unimplemented!("compiling rust services from path to src")
        } else {
            panic!("service must provide path to executable or to compilable project")
        };

        let mut cmd = process::Command::new(model.executable.as_ref().unwrap());
        cmd.args(&model.args);
        let started_at = Instant::now();
        let child = cmd.spawn()?;

        let service = Self {
            name: model.name.clone(),
            bin_path: bin_path.to_path_buf(),
            args: model.args.clone(),
            handle: child,
            started_at,
            address: None,
            std_out_log: "".to_string(),
        };

        Ok(service)
    }

    pub fn get_uptime(&self) -> Duration {
        Instant::now() - self.started_at
    }

    pub fn monitor(&mut self) {
        // check if the service is running
        if let Ok(status) = self.handle.try_wait() {
            if let Some(s) = status {
                warn!(
                    "service \"{}\" found dead with exit status: {}, attempting to restart...",
                    self.name, s
                );
                self.restart(false);
            }
            return;
        }

        let mut buf = [0; 1000];
        if let Ok(n) = self.handle.stdout.as_mut().unwrap().read(&mut buf) {
            trace!("read number of bytes from service stdout: {}", n);
            self.std_out_log.push_str(&String::from_utf8_lossy(&buf));
        }
    }

    pub fn restart(&mut self, kill: bool) -> Result<()> {
        if kill {
            self.handle.kill()?;
        }
        self.handle = process::Command::new(&self.bin_path)
            .args(&self.args)
            .spawn()?;
        self.started_at = Instant::now();

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.handle.kill()?;
        Ok(())
    }
}
