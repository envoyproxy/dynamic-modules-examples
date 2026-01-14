//! A Redis RESP protocol parser and command filter.
//!
//! This filter demonstrates:
//! 1. Parsing the Redis RESP (REdis Serialization Protocol).
//! 2. Command filtering and blocking.
//! 3. Protocol-aware connection handling.
//!
//! Configuration format (JSON):
//! ```json
//! {
//!   "blocked_commands": ["FLUSHALL", "FLUSHDB", "DEBUG"],
//!   "log_commands": true,
//!   "max_command_length": 1024
//! }
//! ```
//!
//! To use this filter as a standalone module, create a separate crate with:
//! ```ignore
//! use envoy_proxy_dynamic_modules_rust_sdk::*;
//! declare_network_filter_init_functions!(init, network_redis::new_filter_config);
//! ```

use envoy_proxy_dynamic_modules_rust_sdk::*;
use serde::Deserialize;
use std::collections::HashSet;

/// Configuration data parsed from JSON.
#[derive(Deserialize)]
struct RedisFilterConfigData {
    #[serde(default)]
    blocked_commands: Vec<String>,
    #[serde(default = "default_log_commands")]
    log_commands: bool,
    #[serde(default = "default_max_command_length")]
    max_command_length: usize,
}

fn default_log_commands() -> bool {
    true
}

fn default_max_command_length() -> usize {
    1024
}

impl Default for RedisFilterConfigData {
    fn default() -> Self {
        RedisFilterConfigData {
            blocked_commands: Vec::new(),
            log_commands: default_log_commands(),
            max_command_length: default_max_command_length(),
        }
    }
}

/// Factory function for creating Redis filter configurations.
pub fn new_filter_config<EC: EnvoyNetworkFilterConfig, ENF: EnvoyNetworkFilter>(
    envoy_filter_config: &mut EC,
    _name: &str,
    config: &[u8],
) -> Option<Box<dyn NetworkFilterConfig<ENF>>> {
    let config_data: RedisFilterConfigData = if config.is_empty() {
        RedisFilterConfigData::default()
    } else {
        match serde_json::from_slice(config) {
            Ok(c) => c,
            Err(e) => {
                envoy_log_info!(
                    "Failed to parse Redis filter config: {}. Using defaults.",
                    e
                );
                RedisFilterConfigData::default()
            }
        }
    };

    // Define metrics.
    let commands_total = envoy_filter_config
        .define_counter("redis_commands_total")
        .ok()?;
    let commands_blocked = envoy_filter_config
        .define_counter("redis_commands_blocked")
        .ok()?;
    let bytes_received = envoy_filter_config
        .define_counter("redis_bytes_received")
        .ok()?;
    let bytes_sent = envoy_filter_config
        .define_counter("redis_bytes_sent")
        .ok()?;
    let active_connections = envoy_filter_config
        .define_gauge("redis_active_connections")
        .ok()?;
    let parse_errors = envoy_filter_config
        .define_counter("redis_parse_errors")
        .ok()?;

    Some(Box::new(RedisFilterConfig {
        blocked_commands: config_data.blocked_commands.into_iter().collect(),
        log_commands: config_data.log_commands,
        max_command_length: config_data.max_command_length,
        commands_total,
        commands_blocked,
        bytes_received,
        bytes_sent,
        active_connections,
        parse_errors,
    }))
}

/// Redis filter configuration.
struct RedisFilterConfig {
    blocked_commands: HashSet<String>,
    log_commands: bool,
    max_command_length: usize,
    commands_total: EnvoyCounterId,
    commands_blocked: EnvoyCounterId,
    bytes_received: EnvoyCounterId,
    bytes_sent: EnvoyCounterId,
    active_connections: EnvoyGaugeId,
    parse_errors: EnvoyCounterId,
}

impl<ENF: EnvoyNetworkFilter> NetworkFilterConfig<ENF> for RedisFilterConfig {
    fn new_network_filter(&self, envoy_filter: &mut ENF) -> Box<dyn NetworkFilter<ENF>> {
        let _ = envoy_filter.increase_gauge(self.active_connections, 1);

        Box::new(RedisFilter {
            blocked_commands: self.blocked_commands.clone(),
            log_commands: self.log_commands,
            max_command_length: self.max_command_length,
            commands_total: self.commands_total,
            commands_blocked: self.commands_blocked,
            bytes_received: self.bytes_received,
            bytes_sent: self.bytes_sent,
            active_connections: self.active_connections,
            parse_errors: self.parse_errors,
        })
    }
}

/// Redis filter instance for a single connection.
struct RedisFilter {
    blocked_commands: HashSet<String>,
    log_commands: bool,
    max_command_length: usize,
    commands_total: EnvoyCounterId,
    commands_blocked: EnvoyCounterId,
    bytes_received: EnvoyCounterId,
    bytes_sent: EnvoyCounterId,
    active_connections: EnvoyGaugeId,
    parse_errors: EnvoyCounterId,
}

/// Represents a parsed Redis command.
#[derive(Debug, Clone, PartialEq)]
pub struct RedisCommand {
    pub name: String,
    pub args: Vec<Vec<u8>>,
}

/// RESP parsing result.
#[derive(Debug, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Vec<u8>),
    Array(Vec<RespValue>),
    Null,
}

/// Standalone RESP parser - can be used and tested without SDK dependencies.
pub struct RespParser;

impl RespParser {
    /// Parses RESP data and extracts commands.
    pub fn parse_commands(data: &[u8]) -> Result<Vec<RedisCommand>, &'static str> {
        let mut commands = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            match Self::parse_resp_value(data, pos) {
                Ok((value, new_pos)) => {
                    if let Some(cmd) = Self::resp_to_command(value) {
                        commands.push(cmd);
                    }
                    pos = new_pos;
                }
                Err(_) => {
                    break;
                }
            }
        }

        Ok(commands)
    }

    /// Parses a single RESP value starting at the given position.
    pub fn parse_resp_value(data: &[u8], pos: usize) -> Result<(RespValue, usize), &'static str> {
        if pos >= data.len() {
            return Err("Incomplete data");
        }

        let type_byte = data[pos];
        let start = pos + 1;

        match type_byte {
            b'+' => {
                let (line, end) = Self::read_line(data, start)?;
                Ok((
                    RespValue::SimpleString(String::from_utf8_lossy(line).to_string()),
                    end,
                ))
            }
            b'-' => {
                let (line, end) = Self::read_line(data, start)?;
                Ok((
                    RespValue::Error(String::from_utf8_lossy(line).to_string()),
                    end,
                ))
            }
            b':' => {
                let (line, end) = Self::read_line(data, start)?;
                let num: i64 = String::from_utf8_lossy(line)
                    .parse()
                    .map_err(|_| "Invalid integer")?;
                Ok((RespValue::Integer(num), end))
            }
            b'$' => {
                let (line, end) = Self::read_line(data, start)?;
                let len: i64 = String::from_utf8_lossy(line)
                    .parse()
                    .map_err(|_| "Invalid bulk string length")?;

                if len < 0 {
                    return Ok((RespValue::Null, end));
                }

                let len = len as usize;
                if end + len + 2 > data.len() {
                    return Err("Incomplete bulk string");
                }

                let content = data[end..end + len].to_vec();
                Ok((RespValue::BulkString(content), end + len + 2))
            }
            b'*' => {
                let (line, mut end) = Self::read_line(data, start)?;
                let count: i64 = String::from_utf8_lossy(line)
                    .parse()
                    .map_err(|_| "Invalid array count")?;

                if count < 0 {
                    return Ok((RespValue::Null, end));
                }

                let mut elements = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let (value, new_end) = Self::parse_resp_value(data, end)?;
                    elements.push(value);
                    end = new_end;
                }

                Ok((RespValue::Array(elements), end))
            }
            _ => Err("Unknown RESP type"),
        }
    }

    /// Reads a line (until \r\n) from the data.
    pub fn read_line(data: &[u8], start: usize) -> Result<(&[u8], usize), &'static str> {
        if start >= data.len() {
            return Err("Start position out of bounds");
        }
        for i in start..data.len().saturating_sub(1) {
            if data[i] == b'\r' && data[i + 1] == b'\n' {
                return Ok((&data[start..i], i + 2));
            }
        }
        Err("Incomplete line")
    }

    /// Converts a RESP value to a Redis command.
    pub fn resp_to_command(value: RespValue) -> Option<RedisCommand> {
        match value {
            RespValue::Array(elements) if !elements.is_empty() => {
                let mut args: Vec<Vec<u8>> = Vec::new();

                for elem in elements {
                    match elem {
                        RespValue::BulkString(s) => args.push(s),
                        RespValue::SimpleString(s) => args.push(s.into_bytes()),
                        _ => {}
                    }
                }

                if args.is_empty() {
                    return None;
                }

                let name = String::from_utf8_lossy(&args[0]).to_uppercase();
                Some(RedisCommand {
                    name,
                    args: args.into_iter().skip(1).collect(),
                })
            }
            _ => None,
        }
    }

    /// Creates a Redis error response.
    pub fn create_error_response(message: &str) -> Vec<u8> {
        format!("-ERR {}\r\n", message).into_bytes()
    }
}

impl RedisFilter {
    fn parse_commands(&self, data: &[u8]) -> Result<Vec<RedisCommand>, &'static str> {
        RespParser::parse_commands(data)
    }

    fn create_error_response(&self, message: &str) -> Vec<u8> {
        RespParser::create_error_response(message)
    }
}

impl<ENF: EnvoyNetworkFilter> NetworkFilter<ENF> for RedisFilter {
    fn on_new_connection(
        &mut self,
        _envoy_filter: &mut ENF,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        envoy_log_debug!("New Redis connection established.");
        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_read(
        &mut self,
        envoy_filter: &mut ENF,
        read_buffer_length: usize,
        _end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        let _ = envoy_filter.increment_counter(self.bytes_received, read_buffer_length as u64);

        // Get data from read buffer.
        let (chunks, _) = envoy_filter.get_read_buffer_chunks();

        // Collect all bytes.
        let mut data = Vec::new();
        for chunk in chunks {
            data.extend_from_slice(chunk.as_slice());
        }

        // Check for command length limit.
        if data.len() > self.max_command_length {
            envoy_log_info!(
                "Redis command exceeds max length: {} > {}",
                data.len(),
                self.max_command_length
            );
            let _ = envoy_filter.increment_counter(self.parse_errors, 1);
            envoy_filter.drain_read_buffer(read_buffer_length);
            envoy_filter.write(&self.create_error_response("command too long"), false);
            return abi::envoy_dynamic_module_type_on_network_filter_data_status::StopIteration;
        }

        // Parse Redis commands.
        match self.parse_commands(&data) {
            Ok(commands) => {
                for cmd in &commands {
                    let _ = envoy_filter.increment_counter(self.commands_total, 1);

                    if self.log_commands {
                        let args_str: Vec<String> = cmd
                            .args
                            .iter()
                            .take(3)
                            .map(|a| String::from_utf8_lossy(a).to_string())
                            .collect();
                        envoy_log_debug!("Redis command: {} {:?}", cmd.name, args_str);
                    }

                    // Check if command is blocked.
                    if self.blocked_commands.contains(&cmd.name) {
                        envoy_log_info!("Blocked Redis command: {}", cmd.name);
                        let _ = envoy_filter.increment_counter(self.commands_blocked, 1);

                        // Drain the read buffer and send error response.
                        envoy_filter.drain_read_buffer(read_buffer_length);
                        let error_msg = format!("command '{}' is not allowed", cmd.name);
                        envoy_filter.write(&self.create_error_response(&error_msg), false);
                        return abi::envoy_dynamic_module_type_on_network_filter_data_status::StopIteration;
                    }
                }
            }
            Err(e) => {
                envoy_log_debug!("Redis parse error: {}", e);
                let _ = envoy_filter.increment_counter(self.parse_errors, 1);
            }
        }

        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_write(
        &mut self,
        envoy_filter: &mut ENF,
        write_buffer_length: usize,
        _end_stream: bool,
    ) -> abi::envoy_dynamic_module_type_on_network_filter_data_status {
        let _ = envoy_filter.increment_counter(self.bytes_sent, write_buffer_length as u64);
        abi::envoy_dynamic_module_type_on_network_filter_data_status::Continue
    }

    fn on_event(
        &mut self,
        envoy_filter: &mut ENF,
        event: abi::envoy_dynamic_module_type_network_connection_event,
    ) {
        match event {
            abi::envoy_dynamic_module_type_network_connection_event::RemoteClose
            | abi::envoy_dynamic_module_type_network_connection_event::LocalClose => {
                let _ = envoy_filter.decrease_gauge(self.active_connections, 1);
                envoy_log_debug!("Redis connection closed.");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config_default() {
        let config = RedisFilterConfigData::default();
        assert!(config.blocked_commands.is_empty());
        assert!(config.log_commands);
        assert_eq!(config.max_command_length, 1024);
    }

    #[test]
    fn test_redis_config_parsing() {
        let json = r#"{"blocked_commands": ["FLUSHALL"], "log_commands": false}"#;
        let config: RedisFilterConfigData = serde_json::from_str(json).unwrap();
        assert_eq!(config.blocked_commands, vec!["FLUSHALL"]);
        assert!(!config.log_commands);
    }

    #[test]
    fn test_create_error_response() {
        let response = RespParser::create_error_response("test error");
        assert_eq!(response, b"-ERR test error\r\n");
    }

    #[test]
    fn test_resp_simple_string_parsing() {
        let data = b"+OK\r\n";
        let result = RespParser::parse_resp_value(data, 0);
        assert!(result.is_ok());
        let (value, consumed) = result.unwrap();
        assert_eq!(value, RespValue::SimpleString("OK".to_string()));
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_resp_bulk_string_parsing() {
        let data = b"$5\r\nhello\r\n";
        let result = RespParser::parse_resp_value(data, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(value, RespValue::BulkString(b"hello".to_vec()));
    }

    #[test]
    fn test_resp_integer_parsing() {
        let data = b":1000\r\n";
        let result = RespParser::parse_resp_value(data, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(value, RespValue::Integer(1000));
    }

    #[test]
    fn test_command_extraction() {
        // *2\r\n$3\r\nGET\r\n$3\r\nkey\r\n
        let data = b"*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n";
        let commands = RespParser::parse_commands(data).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "GET");
        assert_eq!(commands[0].args.len(), 1);
        assert_eq!(commands[0].args[0], b"key");
    }

    #[test]
    fn test_resp_error_parsing() {
        let data = b"-ERR something went wrong\r\n";
        let result = RespParser::parse_resp_value(data, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(
            value,
            RespValue::Error("ERR something went wrong".to_string())
        );
    }

    #[test]
    fn test_resp_null_bulk_string() {
        let data = b"$-1\r\n";
        let result = RespParser::parse_resp_value(data, 0);
        assert!(result.is_ok());
        let (value, _) = result.unwrap();
        assert_eq!(value, RespValue::Null);
    }
}
