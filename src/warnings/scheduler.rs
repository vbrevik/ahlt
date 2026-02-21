use std::time::Duration;
use sqlx::PgPool;
use crate::handlers::warning_handlers::ws::ConnectionMap;

pub fn spawn_scheduler(pool: PgPool, conn_map: ConnectionMap) {
    actix_web::rt::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            log::info!("Running warning scheduler");
            // Run generators
            super::generators::check_users_without_role(&pool, &conn_map).await;
            super::generators::check_database_size(&pool, &conn_map).await;
            super::generators::check_tor_vacancies(&pool, &conn_map).await;
            // Run cleanup
            if let Err(e) = super::generators::cleanup_old_warnings(&pool).await {
                log::error!("Warning cleanup failed: {}", e);
            }
        }
    });
}
