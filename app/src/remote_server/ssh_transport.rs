//! SSH-specific implementation of [`RemoteTransport`].
//!
//! [`SshTransport`] uses an existing SSH ControlMaster socket to check/install
//! the remote server binary and to launch the `remote-server-proxy` process
//! whose stdin/stdout become the protocol channel.
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use warpui::r#async::executor;

use remote_server::auth::RemoteServerAuthContext;
use remote_server::client::RemoteServerClient;
use remote_server::manager::RemoteServerExitStatus;
use remote_server::setup::{
    parse_uname_output, remote_server_daemon_dir, PreinstallCheckResult, RemotePlatform,
};
use remote_server::ssh::ssh_args;
use remote_server::transport::{Connection, RemoteTransport};

use std::sync::Mutex;

/// SSH transport: connects via a ControlMaster socket.
///
/// `socket_path` is the local Unix socket created by the ControlMaster
/// process (`ssh -N -o ControlMaster=yes -o ControlPath=<path>`). All SSH
/// commands (binary check, install, proxy launch) are multiplexed through
/// this socket without re-authenticating.
#[derive(Clone)]
pub struct SshTransport {
    socket_path: PathBuf,
    auth_context: Arc<RemoteServerAuthContext>,
    /// Detected remote platform, set after `detect_platform` succeeds.
    /// Used by the SCP install fallback to construct the download URL.
    platform: Arc<Mutex<Option<RemotePlatform>>>,
}

impl fmt::Debug for SshTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SshTransport")
            .field("socket_path", &self.socket_path)
            .finish_non_exhaustive()
    }
}

impl SshTransport {
    pub fn new(socket_path: PathBuf, auth_context: Arc<RemoteServerAuthContext>) -> Self {
        Self {
            socket_path,
            auth_context,
            platform: Arc::new(Mutex::new(None)),
        }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub fn remote_daemon_socket_path(&self) -> String {
        format!(
            "{}/server.sock",
            remote_server_daemon_dir(&self.auth_context.remote_server_identity_key())
        )
    }

    pub fn remote_daemon_pid_path(&self) -> String {
        format!(
            "{}/server.pid",
            remote_server_daemon_dir(&self.auth_context.remote_server_identity_key())
        )
    }

    fn remote_proxy_command(&self) -> String {
        let binary = remote_server::setup::remote_server_binary();
        let identity_key = self.auth_context.remote_server_identity_key();
        let quoted_identity_key = shell_words::quote(&identity_key);
        format!("{binary} remote-server-proxy --identity-key {quoted_identity_key}")
    }
}

impl RemoteTransport for SshTransport {
    fn detect_platform(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<RemotePlatform, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        let platform_slot = self.platform.clone();
        Box::pin(async move {
            let result = match remote_server::ssh::run_ssh_command(
                &socket_path,
                "uname -sm",
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await
            {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    parse_uname_output(&stdout).map_err(|e| format!("{e:#}"))
                }
                Ok(output) => {
                    let code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("uname -sm exited with code {code}: {stderr}"))
                }
                Err(e) => Err(format!("{e:#}")),
            };
            // Stash the detected platform for the SCP fallback.
            if let Ok(ref p) = result {
                *platform_slot.lock().unwrap() = Some(p.clone());
            }
            result
        })
    }

    fn run_preinstall_check(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<PreinstallCheckResult, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            match remote_server::ssh::run_ssh_script(
                &socket_path,
                remote_server::setup::PREINSTALL_CHECK_SCRIPT,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await
            {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    Ok(PreinstallCheckResult::parse(&stdout))
                }
                Ok(output) => {
                    let code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!(
                        "Preinstall check exited with code {code}: {stderr}"
                    ))
                }
                Err(e) => Err(format!("{e:#}")),
            }
        })
    }

    fn check_binary(&self) -> Pin<Box<dyn Future<Output = Result<bool, String>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let bin_path = remote_server::setup::remote_server_binary();
            log::info!("Checking for remote server binary at {bin_path}");
            match remote_server::ssh::run_ssh_command(
                &socket_path,
                &remote_server::setup::binary_check_command(),
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await
            {
                // `test -x` exits 0 when present, 1 when missing.
                // Any other exit code (or None / signal) is treated as a check failure.
                Ok(output) => match output.status.code() {
                    Some(0) => Ok(true),
                    Some(1) => Ok(false),
                    Some(code) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err(format!("binary check exited with code {code}: {stderr}"))
                    }
                    None => Err("binary check terminated by signal".into()),
                },
                Err(e) => Err(format!("{e:#}")),
            }
        })
    }

    fn check_has_old_binary(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            // Treat the existence of the remote-server install directory
            // itself as evidence of a prior install. If `~/.warp-XX/remote-server`
            // exists, something was installed there before, so any mismatch
            // with the client's expected binary path should be auto-updated
            // rather than surfaced as a first-time install prompt.
            let cmd = format!("test -d {}", remote_server::setup::remote_server_dir());
            let output = remote_server::ssh::run_ssh_command(
                &socket_path,
                &cmd,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await?;
            // `test -d` exits 0 when present, 1 when missing.
            // Anything else is treated as a check failure.
            match output.status.code() {
                Some(0) => Ok(true),
                Some(1) => Ok(false),
                Some(code) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(anyhow::anyhow!(
                        "remote-server dir check exited with code {code}: {stderr}"
                    ))
                }
                None => Err(anyhow::anyhow!(
                    "remote-server dir check terminated by signal"
                )),
            }
        })
    }

    fn install_binary(&self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let socket_path = self.socket_path.clone();
        let platform_slot = self.platform.clone();
        Box::pin(async move {
            let script = remote_server::setup::install_script();
            log::info!(
                "Installing remote server binary to {}",
                remote_server::setup::remote_server_binary()
            );
            match remote_server::ssh::run_ssh_script(
                &socket_path,
                &script,
                remote_server::setup::INSTALL_TIMEOUT,
            )
            .await
            {
                Ok(output) if output.status.success() => Ok(()),
                Ok(output)
                    if output.status.code()
                        == Some(remote_server::setup::NO_HTTP_CLIENT_EXIT_CODE) =>
                {
                    log::info!("Remote has no curl/wget, falling back to SCP upload");
                    let platform = platform_slot.lock().unwrap().clone();
                    let Some(platform) = platform else {
                        return Err(
                            "SCP fallback requires platform detection to have succeeded".into()
                        );
                    };
                    scp_install_fallback(&socket_path, &platform, &script)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
                Ok(output) => {
                    let code = output.status.code().unwrap_or(-1);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("install script failed (exit {code}): {stderr}"))
                }
                Err(e) => Err(format!("{e:#}")),
            }
        })
    }

    fn connect(
        &self,
        executor: Arc<executor::Background>,
    ) -> Pin<Box<dyn Future<Output = Result<Connection>> + Send>> {
        let socket_path = self.socket_path.clone();
        let remote_proxy_command = self.remote_proxy_command();
        Box::pin(async move {
            let mut args = ssh_args(&socket_path);
            args.push(remote_proxy_command);

            // `kill_on_drop(true)` pairs with ownership of the `Child` being
            // returned in the [`Connection`] below: the
            // [`RemoteServerManager`] holds the `Child` on its per-session
            // state, and dropping that state (on explicit teardown or
            // spontaneous disconnect) sends SIGKILL to this ssh process.
            let mut child = command::r#async::Command::new("ssh")
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()?;

            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdout"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stderr"))?;

            let (client, event_rx) =
                RemoteServerClient::from_child_streams(stdin, stdout, stderr, &executor);
            Ok(Connection {
                client,
                event_rx,
                child,
                control_path: Some(socket_path),
            })
        })
    }

    fn remove_remote_server_binary(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        let socket_path = self.socket_path.clone();
        Box::pin(async move {
            let cmd = format!("rm -f {}", remote_server::setup::remote_server_binary());
            log::info!("Removing stale remote server binary: {cmd}");
            let output = remote_server::ssh::run_ssh_command(
                &socket_path,
                &cmd,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!("Failed to remove binary: {stderr}"))
            }
        })
    }

    /// SSH exit code 255 indicates a connection-level error (broken pipe,
    /// connection reset, host unreachable) — the ControlMaster's TCP
    /// connection is dead. A signal kill also suggests the transport was
    /// torn down. In either case, reconnecting through the same
    /// ControlMaster is futile.
    fn is_reconnectable(&self, exit_status: Option<&RemoteServerExitStatus>) -> bool {
        match exit_status {
            Some(s) => s.code != Some(255) && !s.signal_killed,
            // No exit status available — optimistically allow reconnect.
            None => true,
        }
    }
}

/// SCP install fallback: downloads the tarball locally, uploads it to
/// the remote via SCP, then re-invokes the install script with the
/// staging path as $1 so the shared extraction tail runs.
async fn scp_install_fallback(
    socket_path: &Path,
    platform: &RemotePlatform,
    install_script: &str,
) -> anyhow::Result<()> {
    use std::process::Stdio;

    let url = remote_server::setup::download_tarball_url(platform);
    let staging_path = remote_server::setup::remote_tarball_staging_path();
    let timeout = remote_server::setup::SCP_INSTALL_TIMEOUT;

    // 1. Download the tarball locally into a temp directory.
    let tmp_dir =
        tempfile::tempdir().map_err(|e| anyhow::anyhow!("Failed to create local temp dir: {e}"))?;
    let local_tarball = tmp_dir.path().join("oz.tar.gz");

    log::info!("Downloading tarball locally from {url}");
    let output = command::r#async::Command::new("curl")
        .arg("-fSL")
        .arg(&url)
        .arg("-o")
        .arg(&local_tarball)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to spawn local curl: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Local curl failed (exit {:?}): {stderr}",
            output.status.code()
        ));
    }

    // 2. Upload to the remote via SCP.
    log::info!("Uploading tarball to remote at {staging_path}");
    remote_server::ssh::scp_upload(socket_path, &local_tarball, &staging_path, timeout).await?;

    // 3. Re-invoke the install script with the staging path as $1.
    //    The script's `[ -n "$1" ]` branch will mv the tarball and
    //    run the shared extraction tail.
    let staging_path_expanded = staging_path.replace("~", "$HOME");
    log::info!("Running extraction via install script with tarball at {staging_path_expanded}");
    let output = remote_server::ssh::run_ssh_script_with_args(
        socket_path,
        install_script,
        &[&staging_path_expanded],
        timeout,
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "Extraction script failed (exit {code}): {stderr}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warpui::r#async::BoxFuture;
    fn static_auth_context() -> Arc<RemoteServerAuthContext> {
        Arc::new(RemoteServerAuthContext::new(
            || -> BoxFuture<'static, Option<String>> { Box::pin(async { None }) },
            || "user id/with spaces".to_string(),
        ))
    }

    #[test]
    fn remote_proxy_command_quotes_identity_key() {
        let transport = SshTransport::new(
            PathBuf::from("/tmp/control-master.sock"),
            static_auth_context(),
        );

        let command = transport.remote_proxy_command();

        assert!(command.contains("remote-server-proxy --identity-key"));
        assert!(command.contains("'user id/with spaces'"));
    }
}
