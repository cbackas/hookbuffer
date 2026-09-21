use shared_lib::send::Target;
use shared_lib::structs::pushover::PushoverConfig;

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

/// Which forwarding target this instance uses, from `HOOKBUFFER_TARGET`
/// (`discord` by default). Falls back to Discord if `pushover` is requested but
/// its credentials are missing.
pub fn get_target() -> Target {
    match std::env::var("HOOKBUFFER_TARGET") {
        Ok(target) if target.eq_ignore_ascii_case("pushover") => match pushover_config() {
            Some(config) => Target::Pushover(config),
            None => {
                tracing::error!(
                    "HOOKBUFFER_TARGET=pushover but PUSHOVER_TOKEN and PUSHOVER_USER are not both set; falling back to discord"
                );
                Target::Discord
            }
        },
        Ok(target) if !target.is_empty() && !target.eq_ignore_ascii_case("discord") => {
            tracing::warn!(
                "Unknown HOOKBUFFER_TARGET '{}', defaulting to discord",
                target
            );
            Target::Discord
        }
        _ => Target::Discord,
    }
}

fn pushover_config() -> Option<PushoverConfig> {
    let token = non_empty_var("PUSHOVER_TOKEN")?;
    let user = non_empty_var("PUSHOVER_USER")?;
    Some(PushoverConfig {
        token,
        user,
        priority: get_pushover_priority(),
        sound: get_pushover_sound(),
    })
}

fn get_pushover_priority() -> Option<i8> {
    let raw = non_empty_var("PUSHOVER_PRIORITY")?;
    match raw.parse::<i8>() {
        Ok(priority) if (-2..=2).contains(&priority) => Some(priority),
        Ok(priority) => {
            tracing::warn!(
                "PUSHOVER_PRIORITY {} out of range (-2..=2), ignoring",
                priority
            );
            None
        }
        Err(_) => {
            tracing::warn!(
                "PUSHOVER_PRIORITY '{}' is not a valid integer, ignoring",
                raw
            );
            None
        }
    }
}

fn get_pushover_sound() -> Option<String> {
    non_empty_var("PUSHOVER_SOUND")
}

fn non_empty_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
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

    mod get_target {
        use super::*;

        fn clear() {
            std::env::remove_var("HOOKBUFFER_TARGET");
            std::env::remove_var("PUSHOVER_TOKEN");
            std::env::remove_var("PUSHOVER_USER");
            std::env::remove_var("PUSHOVER_PRIORITY");
            std::env::remove_var("PUSHOVER_SOUND");
        }

        #[test]
        #[serial]
        fn defaults_to_discord() {
            clear();
            assert!(matches!(get_target(), Target::Discord));
        }

        #[test]
        #[serial]
        fn unknown_target_falls_back_to_discord() {
            clear();
            std::env::set_var("HOOKBUFFER_TARGET", "carrier-pigeon");
            assert!(matches!(get_target(), Target::Discord));
        }

        #[test]
        #[serial]
        fn pushover_without_credentials_falls_back_to_discord() {
            clear();
            std::env::set_var("HOOKBUFFER_TARGET", "pushover");
            assert!(matches!(get_target(), Target::Discord));
        }

        #[test]
        #[serial]
        fn pushover_with_credentials() {
            clear();
            std::env::set_var("HOOKBUFFER_TARGET", "PushOver");
            std::env::set_var("PUSHOVER_TOKEN", "app-token");
            std::env::set_var("PUSHOVER_USER", "user-key");
            std::env::set_var("PUSHOVER_PRIORITY", "1");
            std::env::set_var("PUSHOVER_SOUND", "magic");
            match get_target() {
                Target::Pushover(config) => {
                    assert_eq!(config.token, "app-token");
                    assert_eq!(config.user, "user-key");
                    assert_eq!(config.priority, Some(1));
                    assert_eq!(config.sound.as_deref(), Some("magic"));
                }
                Target::Discord => panic!("expected pushover target"),
            }
        }

        #[test]
        #[serial]
        fn invalid_priority_is_ignored() {
            clear();
            std::env::set_var("HOOKBUFFER_TARGET", "pushover");
            std::env::set_var("PUSHOVER_TOKEN", "app-token");
            std::env::set_var("PUSHOVER_USER", "user-key");
            std::env::set_var("PUSHOVER_PRIORITY", "99");
            match get_target() {
                Target::Pushover(config) => assert_eq!(config.priority, None),
                Target::Discord => panic!("expected pushover target"),
            }
        }
    }
}
