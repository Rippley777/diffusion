use std::time::Duration;

/// A small, predictable connection pool.
pub struct PoolConfig {
    pub max_connections: usize,
    pub connect_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 8,
            connect_timeout: Duration::from_secs(30),
        }
    }
}

pub fn connect(config: &PoolConfig) -> Result<Pool, Error> {
    let pool = Pool::new(config.max_connections);
    pool.connect(config.connect_timeout)?;
    Ok(pool)
}

pub fn health_check(pool: &Pool) -> bool {
    pool.is_connected()
}
