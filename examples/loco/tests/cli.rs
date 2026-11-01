use reefadmin_loco_example::models::{products, users};
use sea_orm::{Database, EntityTrait};
use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Directory(std::path::PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn executable_requires_explicit_provisioning_and_serves_on_loopback() {
    let directory =
        Directory(std::env::temp_dir().join(format!("reefadmin-cli-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir(&directory.0).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.0.join("inventory.sqlite").display()
    );
    let secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let command = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_reefadmin-loco-example"));
        command
            .env("INVENTORY_DATABASE_URL", &url)
            .env("INVENTORY_SESSION_SECRET", &secret)
            .env("INVENTORY_PORT", port.to_string());
        command
    };
    let missing_secret = command()
        .env_remove("INVENTORY_SESSION_SECRET")
        .output()
        .unwrap();
    assert!(!missing_secret.status.success());
    let invalid_secret = command()
        .env("INVENTORY_SESSION_SECRET", "invalid")
        .output()
        .unwrap();
    assert!(!invalid_secret.status.success());
    assert!(!command().arg("unknown").output().unwrap().status.success());
    assert!(
        !command()
            .args(["create-user", "cli@example.test", "staff"])
            .env_remove("INVENTORY_INITIAL_PASSWORD")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !command()
            .args(["create-user", "cli@example.test", "staff"])
            .env("INVENTORY_INITIAL_PASSWORD", "short")
            .output()
            .unwrap()
            .status
            .success()
    );
    let created = command()
        .args(["create-user", "cli@example.test", "staff"])
        .env("INVENTORY_INITIAL_PASSWORD", "test-only-cli-password")
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(!String::from_utf8_lossy(&created.stdout).contains("test-only-cli-password"));
    let db = Database::connect(&url).await.unwrap();
    let accounts = users::Entity::find().all(&db).await.unwrap();
    assert_eq!(accounts.len(), 1);
    assert_ne!(accounts[0].password_hash, "test-only-cli-password");
    assert_eq!(products::Entity::find().all(&db).await.unwrap().len(), 1);
    let mut server = Server(
        command()
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut connected = None;
    for _ in 0..100 {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "server exited before readiness"
        );
        if let Ok(stream) = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)) {
            connected = Some(stream);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut stream = connected.expect("server did not start within ten seconds");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 200"));
    drop(server);
    db.close().await.unwrap();
}
