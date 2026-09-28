use std::time::Duration;

/// A small, predictable connection pool.
pub struct PoolConfig {
    pub max_connections: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

pub fn connect(config: &PoolConfig) -> Result<Pool, Error> {
    let pool = Pool::new(config.max_connections);
    pool.set_idle_timeout(config.idle_timeout);
    pool.connect(config.connect_timeout)?;
    Ok(pool)
}

pub fn health_check(pool: &Pool) -> bool {
    pool.is_connected() && pool.ping().is_ok()
}
