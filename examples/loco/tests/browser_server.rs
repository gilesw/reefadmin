mod common;
#[tokio::test]
#[ignore = "loopback browser fixture"]
async fn browser_server() {
    let fixture = common::fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:39128")
        .await
        .unwrap();
    axum::serve(listener, fixture.router).await.unwrap();
}
