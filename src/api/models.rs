use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Hello (health check) ─────────────────────────────────────────────

/// GET /
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloResponse {
    pub hello: String,
}

// ── Version ──────────────────────────────────────────────────────────

/// GET /version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
    #[serde(default)]
    pub meta: bool,
}

// ── Traffic ──────────────────────────────────────────────────────────

/// GET /traffic (streaming, each JSON line is one of these).
///
/// Source: `hub/route/server.go` — struct `Traffic`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficEntry {
    pub up: i64,
    pub down: i64,
    #[serde(default, rename = "upTotal")]
    pub up_total: i64,
    #[serde(default, rename = "downTotal")]
    pub down_total: i64,
}

// ── Memory ───────────────────────────────────────────────────────────

/// GET /memory (streaming, each JSON line is one of these).
///
/// Source: `hub/route/server.go` — struct `Memory`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub inuse: u64,
    #[serde(default)]
    pub oslimit: u64,
}

// ── Log ──────────────────────────────────────────────────────────────

/// GET /logs (streaming, each JSON line is one of these — default format).
///
/// Source: `hub/route/server.go` — struct `Log`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    #[serde(rename = "type")]
    pub level: String,
    pub payload: String,
}

/// GET /logs?format=structured (streaming, each JSON line is one of these).
///
/// Source: `hub/route/server.go` — struct `LogStructured`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStructured {
    pub time: String,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub fields: Vec<LogStructuredField>,
}

/// A single key-value field in a structured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStructuredField {
    pub key: String,
    pub value: String,
}

// ── Configs ──────────────────────────────────────────────────────────

/// GET /configs
///
/// The response is the `General` struct returned by `executor.GetGeneral()`.
/// We capture the most commonly used fields; unknown fields land in `extra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    #[serde(default)]
    pub port: i32,
    #[serde(default, rename = "socks-port")]
    pub socks_port: i32,
    #[serde(default, rename = "redir-port")]
    pub redir_port: i32,
    #[serde(default, rename = "tproxy-port")]
    pub tproxy_port: i32,
    #[serde(default, rename = "mixed-port")]
    pub mixed_port: i32,
    #[serde(default)]
    pub authentication: Option<Vec<String>>,
    #[serde(default, rename = "allow-lan")]
    pub allow_lan: bool,
    #[serde(default, rename = "bind-address")]
    pub bind_address: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default, rename = "log-level")]
    pub log_level: String,
    #[serde(default)]
    pub ipv6: bool,
    #[serde(default)]
    pub sniffing: bool,
    #[serde(default, rename = "tcp-concurrent")]
    pub tcp_concurrent: bool,
    #[serde(default, rename = "interface-name")]
    pub interface_name: String,
    #[serde(default)]
    pub tun: Option<TunConfig>,

    /// Catch any extra / unknown fields from GetGeneral().
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// TUN configuration embedded in ConfigResponse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub stack: String,
    #[serde(default, rename = "dns-hijack")]
    pub dns_hijack: Option<Vec<String>>,
    #[serde(default, rename = "auto-route")]
    pub auto_route: bool,
    #[serde(default, rename = "auto-detect-interface")]
    pub auto_detect_interface: bool,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// PATCH /configs — request body.
/// Uses `serde_json::Value` so callers can patch arbitrary fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPatchRequest(pub serde_json::Value);

/// PUT /configs?force=true  and  POST /restart  request body.
///
/// Source: `hub/route/configs.go` — `updateConfigs` handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReloadRequest {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub payload: String,
}

// ── Proxies ──────────────────────────────────────────────────────────

/// GET /proxies
///
/// Source: `hub/route/proxies.go` — `getProxies` returns `{"proxies": map}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxiesResponse {
    pub proxies: HashMap<String, ProxyInfo>,
}

/// A single proxy entry.
///
/// The actual JSON shape depends on the adapter type; we capture common
/// fields and let `extra` absorb the rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub proxy_type: String,
    #[serde(default)]
    pub udp: bool,
    #[serde(default)]
    pub xudp: bool,
    #[serde(default)]
    pub history: Vec<ProxyDelayEntry>,
    #[serde(default)]
    pub all: Option<Vec<String>>,
    #[serde(default)]
    pub now: Option<String>,

    /// Catch any extra / unknown fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// PUT /proxies/:name  request body.
///
/// Source: `hub/route/proxies.go` — `updateProxy`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectProxyRequest {
    pub name: String,
}

/// A single delay-history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyDelayEntry {
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub delay: u64,
}

/// GET /proxies/:name/delay  response.
///
/// Source: `hub/route/proxies.go` — `getProxyDelay` returns `{"delay": N}`.
/// There is no `meanDelay` field in mihomo source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayResponse {
    #[serde(default)]
    pub delay: u64,
}

// ── Groups ───────────────────────────────────────────────────────────

/// GET /group
///
/// Source: `hub/route/groups.go` — `getGroups` returns `{"proxies": [array]}`.
/// Note: this is an *array*, not a map (unlike `/proxies`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupsResponse {
    pub proxies: Vec<GroupInfo>,
}

/// A single policy-group entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub group_type: String,
    #[serde(default)]
    pub now: String,
    #[serde(default)]
    pub all: Vec<String>,
    #[serde(default)]
    pub history: Vec<ProxyDelayEntry>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// GET /group/:name/delay  response.
///
/// Source: `hub/route/groups.go` — `getGroupDelay` returns a map of
/// proxy-name → delay (uint16).
pub type GroupDelayResponse = HashMap<String, u64>;

// ── Providers ────────────────────────────────────────────────────────

/// GET /providers/proxies
///
/// Source: `hub/route/provider.go` — `getProviders` returns `{"providers": map}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProvidersResponse {
    pub providers: HashMap<String, ProxyProviderInfo>,
}

/// A single proxy-provider entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyProviderInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub provider_type: String,
    #[serde(default, rename = "vehicleType")]
    pub vehicle_type: String,
    #[serde(default)]
    pub proxies: Vec<ProxyInfo>,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: String,
    #[serde(default, rename = "subscriptionInfo")]
    pub subscription_info: Option<SubscriptionInfo>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Subscription metadata inside a proxy provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    #[serde(default, rename = "Upload")]
    pub upload: u64,
    #[serde(default, rename = "Download")]
    pub download: u64,
    #[serde(default, rename = "Total")]
    pub total: u64,
    #[serde(default, rename = "Expire")]
    pub expire: u64,
}

/// GET /providers/rules
///
/// Source: `hub/route/provider.go` — `getRuleProviders` returns `{"providers": map}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProvidersResponse {
    pub providers: HashMap<String, RuleProviderInfo>,
}

/// A single rule-provider entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProviderInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "type")]
    pub provider_type: String,
    #[serde(default)]
    pub behavior: String,
    #[serde(default, rename = "ruleCount")]
    pub rule_count: u64,
    #[serde(default, rename = "vehicleType")]
    pub vehicle_type: String,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: String,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Rules ────────────────────────────────────────────────────────────

/// GET /rules
///
/// Source: `hub/route/rules.go` — `getRules` returns `{"rules": [array]}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesResponse {
    pub rules: Vec<RuleInfo>,
}

/// A single rule entry.
///
/// Source: `hub/route/rules.go` — struct `Rule`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleInfo {
    #[serde(default)]
    pub index: i64,
    #[serde(default, rename = "type")]
    pub rule_type: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub proxy: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub extra: Option<RuleExtra>,
}

/// Extra metadata on a rule (from `RuleWrapper`).
///
/// Source: `hub/route/rules.go` — struct `RuleExtra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExtra {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, rename = "hitCount")]
    pub hit_count: u64,
    #[serde(default, rename = "hitAt")]
    pub hit_at: String,
    #[serde(default, rename = "missCount")]
    pub miss_count: u64,
    #[serde(default, rename = "missAt")]
    pub miss_at: String,
}

/// PATCH /rules/disable — request body.
///
/// Key = rule index, value = disabled or not.
/// Source: `hub/route/rules.go` — `disableRules`.
pub type DisableRulesRequest = HashMap<i64, bool>;

// ── Connections ──────────────────────────────────────────────────────

/// GET /connections
///
/// Source: `tunnel/statistic/manager.go` — `Snapshot()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionsResponse {
    #[serde(default, rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(default, rename = "uploadTotal")]
    pub upload_total: u64,
    #[serde(default)]
    pub connections: Option<Vec<ConnectionInfo>>,
    #[serde(default)]
    pub memory: u64,
}

/// A single active connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub metadata: ConnectionMetadata,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub chains: Vec<String>,
    #[serde(default)]
    pub rule: String,
    #[serde(default, rename = "rulePayload")]
    pub rule_payload: String,
}

/// Metadata of a connection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionMetadata {
    #[serde(default)]
    pub network: String,
    #[serde(default, rename = "type")]
    pub conn_type: String,
    #[serde(default, rename = "sourceIP")]
    pub source_ip: String,
    #[serde(default, rename = "destinationIP")]
    pub destination_ip: String,
    #[serde(default, rename = "sourcePort")]
    pub source_port: String,
    #[serde(default, rename = "destinationPort")]
    pub destination_port: String,
    #[serde(default)]
    pub host: String,
    #[serde(default, rename = "dnsMode")]
    pub dns_mode: String,
    #[serde(default, rename = "processPath")]
    pub process_path: String,
    #[serde(default, rename = "specialProxy")]
    pub special_proxy: String,
}

// ── DNS ──────────────────────────────────────────────────────────────

/// GET /dns/query?name=xxx&type=A
///
/// Source: `hub/route/dns.go` — `queryDNS`.
/// Response fields use Go-style capitalisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsQueryResponse {
    #[serde(default, rename = "Status")]
    pub status: i32,
    #[serde(default, rename = "Question")]
    pub question: Option<Vec<DnsQuestion>>,
    #[serde(default, rename = "Answer")]
    pub answer: Option<Vec<DnsAnswer>>,
    #[serde(default, rename = "Authority")]
    pub authority: Option<Vec<DnsAnswer>>,
    #[serde(default, rename = "Additional")]
    pub additional: Option<Vec<DnsAnswer>>,
    #[serde(default, rename = "TC")]
    pub tc: bool,
    #[serde(default, rename = "RD")]
    pub rd: bool,
    #[serde(default, rename = "RA")]
    pub ra: bool,
    #[serde(default, rename = "AD")]
    pub ad: bool,
    #[serde(default, rename = "CD")]
    pub cd: bool,
}

/// A question section entry in a DNS response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsQuestion {
    #[serde(default, rename = "Name")]
    pub name: String,
    #[serde(default, rename = "Qtype")]
    pub qtype: u16,
    #[serde(default, rename = "Qclass")]
    pub qclass: u16,
}

/// An answer / authority / additional section entry in a DNS response.
///
/// Source: `hub/route/dns.go` — `rr2Json` mapper.
/// Fields: `name`, `type`, `TTL`, `data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsAnswer {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "TTL")]
    pub ttl: u64,
    #[serde(default, rename = "type")]
    pub rr_type: u16,
    #[serde(default)]
    pub data: String,
}

// ── Upgrade / Restart ────────────────────────────────────────────────

/// POST /upgrade, POST /upgrade/geo, POST /configs/geo  request body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeRequest {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub payload: String,
}

/// Response from POST /upgrade and POST /restart on success.
///
/// Source: `hub/route/upgrade.go` — `upgradeCore` returns `{"status":"ok"}`.
/// Source: `hub/route/restart.go` — `restart` returns `{"status":"ok"}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: String,
}

// ── Error ────────────────────────────────────────────────────────────

/// Generic error response from mihomo API.
///
/// Source: `hub/route/errors.go` — struct `HTTPError`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Hello ────────────────────────────────────────────────────────

    #[test]
    fn deserialize_hello() {
        let json = r#"{"hello":"mihomo"}"#;
        let h: HelloResponse = serde_json::from_str(json).unwrap();
        assert_eq!(h.hello, "mihomo");
    }

    // ── Version ──────────────────────────────────────────────────────

    #[test]
    fn deserialize_version() {
        let json = r#"{"version":"v1.18.1","meta":true}"#;
        let v: VersionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(v.version, "v1.18.1");
        assert!(v.meta);
    }

    #[test]
    fn deserialize_version_minimal() {
        let json = r#"{"version":"1.0"}"#;
        let v: VersionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(v.version, "1.0");
        assert!(!v.meta);
    }

    // ── Traffic ──────────────────────────────────────────────────────

    #[test]
    fn deserialize_traffic_entry() {
        let json = r#"{"up":1024,"down":2048,"upTotal":10240,"downTotal":20480}"#;
        let t: TrafficEntry = serde_json::from_str(json).unwrap();
        assert_eq!(t.up, 1024);
        assert_eq!(t.down, 2048);
        assert_eq!(t.up_total, 10240);
        assert_eq!(t.down_total, 20480);
    }

    #[test]
    fn deserialize_traffic_entry_missing_totals() {
        let json = r#"{"up":10,"down":20}"#;
        let t: TrafficEntry = serde_json::from_str(json).unwrap();
        assert_eq!(t.up, 10);
        assert_eq!(t.down, 20);
        assert_eq!(t.up_total, 0);
        assert_eq!(t.down_total, 0);
    }

    // ── Memory ───────────────────────────────────────────────────────

    #[test]
    fn deserialize_memory_entry() {
        let json = r#"{"inuse":65536,"oslimit":0}"#;
        let m: MemoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(m.inuse, 65536);
        assert_eq!(m.oslimit, 0);
    }

    // ── Log ──────────────────────────────────────────────────────────

    #[test]
    fn deserialize_log_entry() {
        let json = r#"{"type":"info","payload":"Start initial configuration"}"#;
        let l: LogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(l.level, "info");
        assert!(!l.payload.is_empty());
    }

    #[test]
    fn deserialize_log_structured() {
        let json = r#"{"time":"15:04:05","level":"info","message":"hello","fields":[]}"#;
        let l: LogStructured = serde_json::from_str(json).unwrap();
        assert_eq!(l.level, "info");
        assert_eq!(l.message, "hello");
        assert!(l.fields.is_empty());
    }

    // ── Configs ──────────────────────────────────────────────────────

    #[test]
    fn deserialize_config_response() {
        let json = r#"{
            "port": 7890,
            "socks-port": 7891,
            "redir-port": 0,
            "tproxy-port": 0,
            "mixed-port": 0,
            "allow-lan": false,
            "bind-address": "*",
            "mode": "rule",
            "log-level": "info",
            "ipv6": false
        }"#;
        let c: ConfigResponse = serde_json::from_str(json).unwrap();
        assert_eq!(c.port, 7890);
        assert_eq!(c.socks_port, 7891);
        assert_eq!(c.mode, "rule");
        assert!(!c.allow_lan);
    }

    #[test]
    fn deserialize_config_with_extra_fields() {
        let json = r#"{
            "port": 7890,
            "socks-port": 0,
            "redir-port": 0,
            "tproxy-port": 0,
            "mixed-port": 0,
            "allow-lan": false,
            "bind-address": "*",
            "mode": "rule",
            "log-level": "info",
            "ipv6": false,
            "geodata-mode": true,
            "unified-delay": false
        }"#;
        let c: ConfigResponse = serde_json::from_str(json).unwrap();
        assert_eq!(c.port, 7890);
        assert!(c.extra.contains_key("geodata-mode"));
        assert!(c.extra.contains_key("unified-delay"));
    }

    // ── Proxies ──────────────────────────────────────────────────────

    #[test]
    fn deserialize_proxy_info() {
        let json = r#"{
            "name": "DIRECT",
            "type": "Direct",
            "udp": true,
            "xudp": false,
            "history": [{"time":"2024-01-01T00:00:00Z","delay":10}]
        }"#;
        let p: ProxyInfo = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "DIRECT");
        assert_eq!(p.proxy_type, "Direct");
        assert!(p.udp);
        assert_eq!(p.history.len(), 1);
        assert_eq!(p.history[0].delay, 10);
    }

    #[test]
    fn deserialize_proxies_response() {
        let json = r#"{"proxies":{"DIRECT":{"name":"DIRECT","type":"Direct","udp":true,"xudp":false,"history":[]}}}"#;
        let r: ProxiesResponse = serde_json::from_str(json).unwrap();
        assert!(r.proxies.contains_key("DIRECT"));
    }

    #[test]
    fn serialize_select_proxy_request() {
        let req = SelectProxyRequest {
            name: "Japan".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""name":"Japan""#));
    }

    #[test]
    fn deserialize_delay_response() {
        // Source returns only {"delay": N}
        let json = r#"{"delay":120}"#;
        let d: DelayResponse = serde_json::from_str(json).unwrap();
        assert_eq!(d.delay, 120);
    }

    #[test]
    fn proxy_info_extra_fields() {
        let json = r#"{"name":"test","type":"Vmess","udp":false,"xudp":false,"history":[],"server":"1.2.3.4","port":443}"#;
        let p: ProxyInfo = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "test");
        assert!(p.extra.contains_key("server"));
        assert!(p.extra.contains_key("port"));
    }

    // ── Groups ───────────────────────────────────────────────────────

    #[test]
    fn deserialize_groups_response_as_array() {
        // Source: getGroups returns {"proxies": [...]}, an array, not a map
        let json = r#"{"proxies":[
            {"name":"Proxy","type":"Selector","now":"Japan","all":["Japan","USA"],"history":[]},
            {"name":"Auto","type":"URLTest","now":"USA","all":["Japan","USA"],"history":[]}
        ]}"#;
        let g: GroupsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(g.proxies.len(), 2);
        assert_eq!(g.proxies[0].name, "Proxy");
        assert_eq!(g.proxies[1].group_type, "URLTest");
    }

    #[test]
    fn deserialize_group_info() {
        let json = r#"{
            "name": "Proxy",
            "type": "Selector",
            "now": "Japan",
            "all": ["Japan", "USA"],
            "history": []
        }"#;
        let g: GroupInfo = serde_json::from_str(json).unwrap();
        assert_eq!(g.name, "Proxy");
        assert_eq!(g.now, "Japan");
        assert_eq!(g.all.len(), 2);
    }

    #[test]
    fn deserialize_group_delay_response() {
        let json = r#"{"Japan":120,"USA":250}"#;
        let d: GroupDelayResponse = serde_json::from_str(json).unwrap();
        assert_eq!(*d.get("Japan").unwrap(), 120);
        assert_eq!(*d.get("USA").unwrap(), 250);
    }

    // ── Rules ────────────────────────────────────────────────────────

    #[test]
    fn deserialize_rules_response() {
        let json = r#"{"rules":[{"index":0,"type":"DOMAIN","payload":"example.com","proxy":"DIRECT","size":-1}]}"#;
        let r: RulesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.rules.len(), 1);
        assert_eq!(r.rules[0].index, 0);
        assert_eq!(r.rules[0].rule_type, "DOMAIN");
        assert_eq!(r.rules[0].payload, "example.com");
        assert_eq!(r.rules[0].size, -1);
        assert!(r.rules[0].extra.is_none());
    }

    #[test]
    fn deserialize_rules_with_extra() {
        let json = r#"{"rules":[{
            "index":0,
            "type":"DOMAIN",
            "payload":"example.com",
            "proxy":"DIRECT",
            "size":-1,
            "extra":{"disabled":false,"hitCount":5,"hitAt":"2024-01-01T00:00:00Z","missCount":0,"missAt":"0001-01-01T00:00:00Z"}
        }]}"#;
        let r: RulesResponse = serde_json::from_str(json).unwrap();
        let extra = r.rules[0].extra.as_ref().unwrap();
        assert!(!extra.disabled);
        assert_eq!(extra.hit_count, 5);
    }

    #[test]
    fn serialize_disable_rules_request() {
        let mut req: DisableRulesRequest = HashMap::new();
        req.insert(0, false);
        req.insert(1, true);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("0"));
        assert!(json.contains("1"));
    }

    // ── Connections ──────────────────────────────────────────────────

    #[test]
    fn deserialize_connections_response() {
        let json = r#"{
            "downloadTotal": 100,
            "uploadTotal": 50,
            "connections": [{
                "id": "abc-123",
                "metadata": {
                    "network": "tcp",
                    "type": "HTTP",
                    "sourceIP": "127.0.0.1",
                    "destinationIP": "1.1.1.1",
                    "sourcePort": "12345",
                    "destinationPort": "443",
                    "host": "example.com",
                    "dnsMode": "",
                    "processPath": "",
                    "specialProxy": ""
                },
                "upload": 10,
                "download": 20,
                "start": "2024-01-01T00:00:00Z",
                "chains": ["DIRECT"],
                "rule": "MATCH",
                "rulePayload": ""
            }],
            "memory": 0
        }"#;
        let c: ConnectionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(c.download_total, 100);
        assert_eq!(c.upload_total, 50);
        let conns = c.connections.as_ref().unwrap();
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].id, "abc-123");
        assert_eq!(conns[0].metadata.host, "example.com");
    }

    #[test]
    fn deserialize_connections_null_list() {
        // When no connections exist, mihomo may return null
        let json = r#"{"downloadTotal":0,"uploadTotal":0,"connections":null,"memory":0}"#;
        let c: ConnectionsResponse = serde_json::from_str(json).unwrap();
        assert!(c.connections.is_none());
    }

    // ── DNS ──────────────────────────────────────────────────────────

    #[test]
    fn deserialize_dns_query_response() {
        // Source uses "type" and "TTL" (not Qtype/Qclass) in answer records
        let json = r#"{
            "Status": 0,
            "Question": [{"Name":"example.com.","Qtype":1,"Qclass":1}],
            "Answer": [{"name":"example.com.","TTL":300,"type":1,"data":"93.184.216.34"}],
            "TC": false,
            "RD": true,
            "RA": true,
            "AD": false,
            "CD": false
        }"#;
        let d: DnsQueryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(d.status, 0);
        let questions = d.question.as_ref().unwrap();
        assert_eq!(questions.len(), 1);
        let answers = d.answer.as_ref().unwrap();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].data, "93.184.216.34");
        assert_eq!(answers[0].ttl, 300);
        assert_eq!(answers[0].rr_type, 1);
        assert!(d.rd);
        assert!(d.ra);
    }

    #[test]
    fn deserialize_dns_response_no_answer() {
        let json = r#"{"Status":3,"Question":[{"Name":"nx.example.com.","Qtype":1,"Qclass":1}],"TC":false,"RD":true,"RA":true,"AD":false,"CD":false}"#;
        let d: DnsQueryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(d.status, 3);
        assert!(d.answer.is_none());
    }

    // ── Providers ────────────────────────────────────────────────────

    #[test]
    fn deserialize_proxy_provider() {
        let json = r#"{
            "name": "my-provider",
            "type": "Proxy",
            "vehicleType": "HTTP",
            "proxies": [],
            "updatedAt": "2024-01-01T00:00:00Z"
        }"#;
        let p: ProxyProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "my-provider");
        assert_eq!(p.vehicle_type, "HTTP");
    }

    #[test]
    fn deserialize_rule_provider() {
        let json = r#"{
            "name": "my-rules",
            "type": "Rule",
            "behavior": "domain",
            "ruleCount": 42,
            "vehicleType": "HTTP",
            "updatedAt": "2024-01-01T00:00:00Z"
        }"#;
        let r: RuleProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(r.name, "my-rules");
        assert_eq!(r.rule_count, 42);
        assert_eq!(r.behavior, "domain");
    }

    #[test]
    fn deserialize_subscription_info() {
        let json = r#"{"Upload":100,"Download":200,"Total":1000,"Expire":1700000000}"#;
        let s: SubscriptionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(s.upload, 100);
        assert_eq!(s.download, 200);
        assert_eq!(s.total, 1000);
        assert_eq!(s.expire, 1700000000);
    }

    // ── Config reload / upgrade requests ─────────────────────────────

    #[test]
    fn serialize_config_reload_request() {
        let req = ConfigReloadRequest {
            path: "/path/to/config.yaml".to_string(),
            payload: String::new(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("/path/to/config.yaml"));
    }

    #[test]
    fn serialize_upgrade_request() {
        let req = UpgradeRequest {
            path: String::new(),
            payload: String::new(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""path":"""#));
    }

    #[test]
    fn config_patch_request_arbitrary_json() {
        let val = serde_json::json!({"mixed-port": 7890, "allow-lan": true});
        let req = ConfigPatchRequest(val);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("mixed-port"));
        assert!(json.contains("allow-lan"));
    }

    // ── Status / Error responses ─────────────────────────────────────

    #[test]
    fn deserialize_status_response() {
        let json = r#"{"status":"ok"}"#;
        let s: StatusResponse = serde_json::from_str(json).unwrap();
        assert_eq!(s.status, "ok");
    }

    #[test]
    fn deserialize_api_error() {
        let json = r#"{"message":"Body invalid"}"#;
        let e: ApiError = serde_json::from_str(json).unwrap();
        assert_eq!(e.message, "Body invalid");
    }
}
