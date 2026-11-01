fn bad(err: Error, db: Db) {
    // ruleid: reefadmin-public-internal-error
    let response = (StatusCode::INTERNAL_SERVER_ERROR, err.to_string());
    // ruleid: reefadmin-disable-tls-verification
    client.danger_accept_invalid_certs(true);
    // ruleid: reefadmin-interpolated-sql
    db.execute_unprepared(&format!("SELECT * FROM users WHERE name='{}'", name));
}
fn good(err: Error, db: Db) {
    // ok: reefadmin-public-internal-error
    let response = (StatusCode::INTERNAL_SERVER_ERROR, "Request failed.");
    // ok: reefadmin-disable-tls-verification
    client.danger_accept_invalid_certs(false);
    // ok: reefadmin-interpolated-sql
    db.execute_unprepared("SELECT 1");
}
