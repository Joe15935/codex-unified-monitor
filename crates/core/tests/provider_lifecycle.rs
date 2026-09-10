#[cfg(unix)]
#[test]
fn one_owned_child_serves_multiple_reads_reaps_on_drop_and_sanitizes_errors() {
    use codexmeter_core::account::Client;
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let d = tempfile::tempdir().unwrap();
    let binary = d.path().join("fake-app-server");
    let pid_file = d.path().join("pids");
    let script = format!(
        r#"#!/bin/sh
printf '%s\n' "$$" >> '{}'
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  [ -n "$id" ] || continue
  case "$line" in
    *account/rateLimits/read*) result='{{"rateLimits":{{"primary":{{"usedPercent":20,"windowDurationMins":10080,"resetsAt":1900000000}}}}}}' ;;
    *account/usage/read*) printf '{{"id":%s,"error":{{"code":500,"message":"SYNTHETIC_SECRET_TOKEN"}}}}\n' "$id"; continue ;;
    *account/read*) result='{{"account":{{"email":"a@example.invalid","planType":"pro"}}}}' ;;
    *) result='{{}}' ;;
  esac
  printf '{{"id":%s,"result":%s}}\n' "$id" "$result"
done
"#,
        pid_file.display()
    );
    fs::write(&binary, script).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
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
    let pid = pids.trim();
    assert!(Command::new("/bin/kill")
        .args(["-0", pid])
        .status()
        .unwrap()
        .success());
    drop(client);
    assert!(!Command::new("/bin/kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success());
}
