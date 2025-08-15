// This module sets up a global logger for the entire application, which
// is essential for observing behavior, debugging issues, and monitoring
// performance. It uses the `tracing` ecosystem, which provides structured,
// level-based logging.

use crate::config::Config;

pub fn init() {
    // Initialize with basic configuration
    let config = Config {
        key: None,
        account_token: None,
        max_threads: None,
        server_address: "quiver.nockpool.com:27016".to_string(),
        client_address: "0.0.0.0:27017".to_string(),
        network_only: false,
        insecure: false,
        benchmark: false,
        clear_key: false,
        api_url: "https://nockpool.com".to_string(),
        nockapp_cli: nockapp::kernel::boot::default_boot_cli(false),
    };
    init_with_config(&config);
}

pub fn init_with_config(config: &Config) {
    // Use nockapp's tracing initialization which handles Tracy and hoon-level tracing
    nockapp::kernel::boot::init_default_tracing(&config.nockapp_cli);
}