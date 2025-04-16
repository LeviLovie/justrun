pub const PID: &str = "/tmp/justrund/pid";
pub const SOCKET: &str = "/tmp/justrund/socket";
pub const TMP_PATHS: [&str; 2] = [PID, SOCKET];
pub const CONFIG: &str = "/etc/justrund/config.yaml";
pub const RESULT_BASE: &str = "/tmp/justrund/results/";
pub const DEFAULT_CONFIG: &[u8] = include_bytes!("../default_config.yaml");
