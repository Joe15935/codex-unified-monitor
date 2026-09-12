#[test]
fn one_owned_child_serves_multiple_reads_reaps_on_drop_and_sanitizes_errors() {
    use codexmeter_core::account::Client;
    use std::{fs, process::Command};
    let d = tempfile::Builder::new()
        .prefix("provider test 中文 ")
        .tempdir()
        .unwrap();
    let binary = d
        .path()
        .join(format!("fake-app-server{}", std::env::consts::EXE_SUFFIX));
    let pid_file = d.path().join("provider-pids");
    let compilation = Command::new("rustc")
        .arg("--edition=2021")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/fake_app_server.rs"),
        )
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "{}",
        String::from_utf8_lossy(&compilation.stderr)
    );
    let mut client = Client::start_binary(binary).unwrap();
    assert_eq!(
        client
            .read(false)
            .unwrap()
            .weekly
            .unwrap()
            .remaining_percent,
        80.0
    );
    assert_eq!(
        client
            .read(false)
            .unwrap()
            .weekly
            .unwrap()
            .remaining_percent,
        80.0
    );
    let error = client
        .request("account/usage/read", None)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("SYNTHETIC_SECRET_TOKEN"));
    assert!(client.request("account/logout", None).is_err());
    assert!(client
        .request(
            "account/read",
            Some(serde_json::json!({"refreshToken":true}))
        )
        .is_err());
    let pids = fs::read_to_string(pid_file).unwrap();
    assert_eq!(pids.lines().count(), 1);
    let pid = pids.trim().parse::<u32>().unwrap();
    let probe = ProcessProbe::new(pid);
    assert!(probe.alive());
    drop(client);
    assert!(!probe.alive());
}

#[cfg(unix)]
struct ProcessProbe(u32);
#[cfg(unix)]
impl ProcessProbe {
    fn new(pid: u32) -> Self {
        Self(pid)
    }
    fn alive(&self) -> bool {
        use std::process::Command;
        Command::new("/bin/kill")
            .args(["-0", &self.0.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }
}

#[cfg(windows)]
struct ProcessProbe(*mut std::ffi::c_void);
#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
}
#[cfg(windows)]
impl ProcessProbe {
    fn new(pid: u32) -> Self {
        // SAFETY: query only SYNCHRONIZE access to our known fixture PID.
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
        assert!(!handle.is_null());
        Self(handle)
    }
    fn alive(&self) -> bool {
        // SAFETY: this owned process handle remains valid until Drop.
        let state = unsafe { WaitForSingleObject(self.0, 0) };
        assert!(
            state == 0 || state == 258,
            "Unexpected process wait result: {state}"
        );
        state == 258
    }
}
#[cfg(windows)]
impl Drop for ProcessProbe {
    fn drop(&mut self) {
        // SAFETY: close exactly the handle returned by OpenProcess.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
