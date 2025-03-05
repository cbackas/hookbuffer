pub fn get_server_port() -> u16 {
    match std::env::var("HOOKBUFFER_PORT") {
        Ok(port) => {
            tracing::debug!("Found HOOKBUFFER_PORT: {}", port);
            match port.parse::<u16>() {
                Ok(port) => port,
                Err(_) => {
                    tracing::warn!("Custom HOOKBUFFER_PORT is not a valid port number, using default port 8000");
                    8000
                }
            }
        }
        Err(_) => {
            tracing::debug!("No HOOKBUFFER_PORT found, using default port 8000");
            8000
        }
    }
}

pub fn get_destination_url() -> String {
    match std::env::var("HOOKBUFFER_DESTINATION_URL") {
        Ok(mut url) => {
            if !url.ends_with('/') {
                url.push('/');
            }
            tracing::info!("Found custom HOOKBUFFER_DESTINATION_URL: {}", url);
            url
        }
        Err(_) => "https://discord.com/".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    mod get_server_port {
        use super::*;

        #[test]
        #[serial]
        fn default() {
            std::env::remove_var("HOOKBUFFER_PORT");
            assert_eq!(get_server_port(), 8000);
        }

        #[test]
        #[serial]
        fn custom() {
            std::env::set_var("HOOKBUFFER_PORT", "1234");
            assert_eq!(get_server_port(), 1234);
        }

        #[test]
        #[serial]
        fn custom_invalid() {
            std::env::set_var("HOOKBUFFER_PORT", "abc");
            assert_eq!(get_server_port(), 8000);
        }
    }

    mod get_destination_url {
        use super::*;

        #[test]
        #[serial]
        fn default() {
            std::env::remove_var("HOOKBUFFER_DESTINATION_URL");
            assert_eq!(get_destination_url(), "https://discord.com/");
        }

        #[test]
        #[serial]
        fn custom() {
            std::env::set_var("HOOKBUFFER_DESTINATION_URL", "https://example.com");
            assert_eq!(get_destination_url(), "https://example.com/");
        }

        #[test]
        #[serial]
        fn custom_no_trailing_slash() {
            std::env::set_var("HOOKBUFFER_DESTINATION_URL", "https://example.com");
            assert_eq!(get_destination_url(), "https://example.com/");
        }
    }
}
