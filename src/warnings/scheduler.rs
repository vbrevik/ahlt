use std::time::Duration;
use crate::db::DbPool;
use crate::handlers::warning_handlers::ws::ConnectionMap;

pub fn spawn_scheduler(pool: DbPool, conn_map: ConnectionMap, data_dir: String) {
    actix_web::rt::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            log::info!("Running warning scheduler");
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Scheduler: failed to get DB connection: {}", e);
                    continue;
                }
            };
            // Run generators
            super::generators::check_users_without_role(&conn, &conn_map, &pool);
            super::generators::check_database_size(&conn, &conn_map, &pool, &data_dir);
            // Run cleanup
            if let Err(e) = super::generators::cleanup_old_warnings(&conn) {
                log::error!("Warning cleanup failed: {}", e);
            }
        }
    });
}
