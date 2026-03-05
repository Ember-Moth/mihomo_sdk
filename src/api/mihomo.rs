use crate::api::models::*;
use crate::{MihomoManager, ProcessError};

/// Helper: parse JSON body from an HTTP response, returning a typed result.
fn parse_body<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, ProcessError> {
    serde_json::from_str(body).map_err(|e| {
        ProcessError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to parse response JSON: {e}"),
        ))
    })
}

/// Minimal percent-encoding for path segments and query values.
/// Encodes spaces, `#`, `%`, `?`, `&`, `=`, `/`, `+` and non-ASCII bytes.
fn urlencoded(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b' ' => out.push_str("%20"),
            b'#' => out.push_str("%23"),
            b'%' => out.push_str("%25"),
            b'&' => out.push_str("%26"),
            b'+' => out.push_str("%2B"),
            b'/' => out.push_str("%2F"),
            b'=' => out.push_str("%3D"),
            b'?' => out.push_str("%3F"),
            // printable ASCII that doesn't need encoding
            0x21..=0x7E => out.push(b as char),
            // everything else (control chars, non-ASCII)
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════════════
//  Hello (health check)
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET / — 健康检查。
    ///
    /// 返回 `{"hello": "mihomo"}`，可用于判断 mihomo API 是否就绪。
    ///
    /// Source: `hub/route/server.go` — `hello` handler.
    pub async fn hello(&self) -> Result<HelloResponse, ProcessError> {
        let resp = self.api().get("/").await?;
        parse_body(&resp.body)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Version
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /version — 获取 mihomo 版本信息。
    ///
    /// Source: `hub/route/server.go` — `version` handler.
    /// Returns `{"meta": true/false, "version": "..."}`.
    pub async fn get_version(&self) -> Result<VersionResponse, ProcessError> {
        let resp = self.api().get("/version").await?;
        parse_body(&resp.body)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Configs
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /configs — 获取当前运行配置。
    ///
    /// Source: `hub/route/configs.go` — `getConfigs` calls `executor.GetGeneral()`.
    pub async fn get_configs(&self) -> Result<ConfigResponse, ProcessError> {
        let resp = self.api().get("/configs").await?;
        parse_body(&resp.body)
    }

    /// PUT /configs?force=true — 重新加载配置。
    ///
    /// - `path`: 配置文件的绝对路径（空字符串使用当前配置路径）
    /// - `payload`: 配置内容字符串（空字符串表示从 path 读取）
    ///
    /// Source: `hub/route/configs.go` — `updateConfigs`.
    /// 注意：`path` 如果不为空必须是绝对路径，且必须在 `SAFE_PATHS` 中。
    pub async fn reload_configs(&self, path: &str, payload: &str) -> Result<(), ProcessError> {
        let body = serde_json::to_string(&ConfigReloadRequest {
            path: path.to_string(),
            payload: payload.to_string(),
        })
        .unwrap();
        self.api().put("/configs?force=true", &body).await?;
        Ok(())
    }

    /// PUT /configs — 重新加载配置（不强制）。
    ///
    /// 与 `reload_configs` 相同但不带 `?force=true`。
    pub async fn reload_configs_no_force(
        &self,
        path: &str,
        payload: &str,
    ) -> Result<(), ProcessError> {
        let body = serde_json::to_string(&ConfigReloadRequest {
            path: path.to_string(),
            payload: payload.to_string(),
        })
        .unwrap();
        self.api().put("/configs", &body).await?;
        Ok(())
    }

    /// PATCH /configs — 更新部分配置字段。
    ///
    /// 传入任意 JSON value，例如：
    /// ```ignore
    /// mgr.patch_configs(serde_json::json!({"mixed-port": 7890})).await?;
    /// ```
    ///
    /// Source: `hub/route/configs.go` — `patchConfigs`.
    /// 支持的字段见 `configSchema` 结构体定义。
    pub async fn patch_configs(&self, patch: serde_json::Value) -> Result<(), ProcessError> {
        let body = serde_json::to_string(&ConfigPatchRequest(patch)).unwrap();
        self.api().patch("/configs", &body).await?;
        Ok(())
    }

    /// POST /configs/geo — 更新 GEO 数据库。
    ///
    /// Source: `hub/route/configs.go` — `updateGeoDatabases`.
    /// 注意：请求体会被忽略，实际调用的是 `updater.UpdateGeoDatabases()`。
    pub async fn update_geo_database(&self) -> Result<(), ProcessError> {
        self.api().post("/configs/geo", None).await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Restart / Upgrade
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// POST /restart — 重启 mihomo 内核进程。
    ///
    /// Source: `hub/route/restart.go` — `restart`.
    /// **注意**：此操作会导致 mihomo 进程 exec 重启自身。
    /// 在 Windows 上会启动新进程然后 `os.Exit(0)`。
    /// 调用后当前 pipe 连接会断开，需要重新连接。
    pub async fn restart_core(&self) -> Result<StatusResponse, ProcessError> {
        let resp = self.api().post("/restart", None).await?;
        parse_body(&resp.body)
    }

    /// POST /upgrade — 更新内核二进制。
    ///
    /// Source: `hub/route/upgrade.go` — `upgradeCore`.
    /// 支持查询参数：
    /// - `channel`: 更新通道（可选）
    /// - `force`: 是否强制更新（可选）
    ///
    /// 成功后会自动调用 `restartExecutable` 重启。
    pub async fn upgrade_core(
        &self,
        channel: Option<&str>,
        force: bool,
    ) -> Result<StatusResponse, ProcessError> {
        let mut path = "/upgrade".to_string();
        let mut params = Vec::new();
        if let Some(ch) = channel {
            params.push(format!("channel={}", urlencoded(ch)));
        }
        if force {
            params.push("force=true".to_string());
        }
        if !params.is_empty() {
            path.push('?');
            path.push_str(&params.join("&"));
        }
        let resp = self.api().post(&path, None).await?;
        parse_body(&resp.body)
    }

    /// POST /upgrade/ui — 更新外部 UI 面板。
    ///
    /// Source: `hub/route/upgrade.go` — `updateUI`.
    /// 需要配置文件中设置了 `external-ui`。
    pub async fn upgrade_ui(&self) -> Result<StatusResponse, ProcessError> {
        let resp = self.api().post("/upgrade/ui", None).await?;
        parse_body(&resp.body)
    }

    /// POST /upgrade/geo — 更新 GEO 数据库（upgrade 路径）。
    ///
    /// Source: `hub/route/upgrade.go` — 复用 `updateGeoDatabases` handler。
    pub async fn upgrade_geo(&self) -> Result<(), ProcessError> {
        self.api().post("/upgrade/geo", None).await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Cache
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// POST /cache/fakeip/flush — 清除 FakeIP 缓存。
    ///
    /// Source: `hub/route/cache.go` — `flushFakeIPPool`.
    pub async fn flush_fakeip_cache(&self) -> Result<(), ProcessError> {
        self.api().post("/cache/fakeip/flush", None).await?;
        Ok(())
    }

    /// POST /cache/dns/flush — 清除 DNS 缓存。
    ///
    /// Source: `hub/route/cache.go` — `flushDnsCache`.
    pub async fn flush_dns_cache(&self) -> Result<(), ProcessError> {
        self.api().post("/cache/dns/flush", None).await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Proxies
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /proxies — 获取所有代理信息（含 provider 内的代理）。
    ///
    /// Source: `hub/route/proxies.go` — `getProxies`.
    /// 返回 `{"proxies": {name: ProxyInfo, ...}}`。
    pub async fn get_proxies(&self) -> Result<ProxiesResponse, ProcessError> {
        let resp = self.api().get("/proxies").await?;
        parse_body(&resp.body)
    }

    /// GET /proxies/:name — 获取指定代理信息。
    ///
    /// Source: `hub/route/proxies.go` — `getProxy`.
    pub async fn get_proxy(&self, name: &str) -> Result<ProxyInfo, ProcessError> {
        let path = format!("/proxies/{}", urlencoded(name));
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// PUT /proxies/:name — 在 Selector 类型策略组中选择指定代理。
    ///
    /// Source: `hub/route/proxies.go` — `updateProxy`.
    /// 请求体：`{"name": "proxy_name"}`。
    /// 目标代理必须是 `SelectAble` 类型，否则返回 400。
    pub async fn select_proxy(&self, group: &str, proxy: &str) -> Result<(), ProcessError> {
        let path = format!("/proxies/{}", urlencoded(group));
        let body = serde_json::to_string(&SelectProxyRequest {
            name: proxy.to_string(),
        })
        .unwrap();
        self.api().put(&path, &body).await?;
        Ok(())
    }

    /// GET /proxies/:name/delay?url=xxx&timeout=5000 — 测试指定代理延迟。
    ///
    /// Source: `hub/route/proxies.go` — `getProxyDelay`.
    /// 返回 `{"delay": N}`。
    /// 可选参数 `expected`: 期望的 HTTP 状态码范围（如 "200" 或 "200-299"）。
    pub async fn test_proxy_delay(
        &self,
        name: &str,
        url: &str,
        timeout_ms: u64,
    ) -> Result<DelayResponse, ProcessError> {
        let path = format!(
            "/proxies/{}/delay?url={}&timeout={}",
            urlencoded(name),
            urlencoded(url),
            timeout_ms,
        );
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// GET /proxies/:name/delay — 测试延迟（附带 expected 参数）。
    ///
    /// `expected`: 期望的 HTTP 状态码范围，如 `"200"` 或 `"200-299,304"`。
    pub async fn test_proxy_delay_with_expected(
        &self,
        name: &str,
        url: &str,
        timeout_ms: u64,
        expected: &str,
    ) -> Result<DelayResponse, ProcessError> {
        let path = format!(
            "/proxies/{}/delay?url={}&timeout={}&expected={}",
            urlencoded(name),
            urlencoded(url),
            timeout_ms,
            urlencoded(expected),
        );
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// DELETE /proxies/:name — 清除非 Selector 类型策略组的 fixed 选择。
    ///
    /// Source: `hub/route/proxies.go` — `unfixedProxy`.
    /// 仅对 `SelectAble` 且非 `Selector` 类型（如 URLTest/Fallback）生效。
    pub async fn unfixed_proxy(&self, name: &str) -> Result<(), ProcessError> {
        let path = format!("/proxies/{}", urlencoded(name));
        self.api().delete(&path).await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Groups
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /group — 获取所有策略组信息。
    ///
    /// Source: `hub/route/groups.go` — `getGroups`.
    /// 返回 `{"proxies": [GroupInfo, ...]}`（注意是数组，不是 map）。
    pub async fn get_groups(&self) -> Result<GroupsResponse, ProcessError> {
        let resp = self.api().get("/group").await?;
        parse_body(&resp.body)
    }

    /// GET /group/:name — 获取指定策略组信息。
    ///
    /// Source: `hub/route/groups.go` — `getGroup`.
    /// 如果指定的代理不是 ProxyGroup 类型，返回 404。
    pub async fn get_group(&self, name: &str) -> Result<GroupInfo, ProcessError> {
        let path = format!("/group/{}", urlencoded(name));
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// GET /group/:name/delay?url=xxx&timeout=5000 — 测试策略组内所有节点延迟。
    ///
    /// Source: `hub/route/groups.go` — `getGroupDelay`.
    /// 会同时清除自动策略组的 fixed 选择。
    /// 返回 `{"proxy_name": delay_ms, ...}`。
    pub async fn test_group_delay(
        &self,
        name: &str,
        url: &str,
        timeout_ms: u64,
    ) -> Result<GroupDelayResponse, ProcessError> {
        let path = format!(
            "/group/{}/delay?url={}&timeout={}",
            urlencoded(name),
            urlencoded(url),
            timeout_ms,
        );
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// GET /group/:name/delay — 测试策略组延迟（附带 expected 参数）。
    pub async fn test_group_delay_with_expected(
        &self,
        name: &str,
        url: &str,
        timeout_ms: u64,
        expected: &str,
    ) -> Result<GroupDelayResponse, ProcessError> {
        let path = format!(
            "/group/{}/delay?url={}&timeout={}&expected={}",
            urlencoded(name),
            urlencoded(url),
            timeout_ms,
            urlencoded(expected),
        );
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Proxy Providers
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /providers/proxies — 获取所有代理集合。
    ///
    /// Source: `hub/route/provider.go` — `getProviders`.
    /// 返回 `{"providers": {name: ProxyProviderInfo, ...}}`。
    pub async fn get_proxy_providers(&self) -> Result<ProxyProvidersResponse, ProcessError> {
        let resp = self.api().get("/providers/proxies").await?;
        parse_body(&resp.body)
    }

    /// GET /providers/proxies/:name — 获取指定代理集合信息。
    ///
    /// Source: `hub/route/provider.go` — `getProvider`.
    pub async fn get_proxy_provider(&self, name: &str) -> Result<ProxyProviderInfo, ProcessError> {
        let path = format!("/providers/proxies/{}", urlencoded(name));
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// PUT /providers/proxies/:name — 更新指定代理集合。
    ///
    /// Source: `hub/route/provider.go` — `updateProvider`.
    /// 触发 provider 的 `Update()` 方法拉取最新数据。
    pub async fn update_proxy_provider(&self, name: &str) -> Result<(), ProcessError> {
        let path = format!("/providers/proxies/{}", urlencoded(name));
        self.api().put(&path, "").await?;
        Ok(())
    }

    /// GET /providers/proxies/:name/healthcheck — 触发代理集合健康检查。
    ///
    /// Source: `hub/route/provider.go` — `healthCheckProvider`.
    pub async fn healthcheck_proxy_provider(&self, name: &str) -> Result<(), ProcessError> {
        let path = format!("/providers/proxies/{}/healthcheck", urlencoded(name));
        self.api().get(&path).await?;
        Ok(())
    }

    /// GET /providers/proxies/:provider/:proxy — 获取代理集合内指定代理的信息。
    ///
    /// Source: `hub/route/provider.go` — `proxyProviderProxyRouter` mounts `getProxy`.
    pub async fn get_proxy_in_provider(
        &self,
        provider: &str,
        proxy: &str,
    ) -> Result<ProxyInfo, ProcessError> {
        let path = format!(
            "/providers/proxies/{}/{}",
            urlencoded(provider),
            urlencoded(proxy),
        );
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }

    /// GET /providers/proxies/:provider/:proxy/healthcheck?url=xxx&timeout=5000
    /// — 测试代理集合内指定代理延迟。
    ///
    /// Source: `hub/route/provider.go` — `proxyProviderProxyRouter` mounts `getProxyDelay`.
    /// 复用 `proxies.go` 的 `getProxyDelay` handler。
    pub async fn healthcheck_proxy_in_provider(
        &self,
        provider: &str,
        proxy: &str,
        url: &str,
        timeout_ms: u64,
    ) -> Result<DelayResponse, ProcessError> {
        let path = format!(
            "/providers/proxies/{}/{}/healthcheck?url={}&timeout={}",
            urlencoded(provider),
            urlencoded(proxy),
            urlencoded(url),
            timeout_ms,
        );
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Rules
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /rules — 获取所有规则。
    ///
    /// Source: `hub/route/rules.go` — `getRules`.
    /// 每条规则包含 `index`, `type`, `payload`, `proxy`, `size`，
    /// 以及可选的 `extra`（disabled, hitCount 等）。
    pub async fn get_rules(&self) -> Result<RulesResponse, ProcessError> {
        let resp = self.api().get("/rules").await?;
        parse_body(&resp.body)
    }

    /// PATCH /rules/disable — 禁用/启用指定规则。
    ///
    /// Source: `hub/route/rules.go` — `disableRules`.
    /// 请求体格式：`{rule_index: disabled, ...}`，例如 `{"0": true, "1": false}`。
    /// 此操作是临时的，重启后失效。
    pub async fn disable_rules(&self, rules: &DisableRulesRequest) -> Result<(), ProcessError> {
        let body = serde_json::to_string(rules).unwrap();
        self.api().patch("/rules/disable", &body).await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Rule Providers
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /providers/rules — 获取所有规则集合。
    ///
    /// Source: `hub/route/provider.go` — `getRuleProviders`.
    /// 返回 `{"providers": {name: RuleProviderInfo, ...}}`。
    pub async fn get_rule_providers(&self) -> Result<RuleProvidersResponse, ProcessError> {
        let resp = self.api().get("/providers/rules").await?;
        parse_body(&resp.body)
    }

    /// PUT /providers/rules/:name — 更新指定规则集合。
    ///
    /// Source: `hub/route/provider.go` — `updateRuleProvider`.
    pub async fn update_rule_provider(&self, name: &str) -> Result<(), ProcessError> {
        let path = format!("/providers/rules/{}", urlencoded(name));
        self.api().put(&path, "").await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Connections
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /connections — 获取当前所有连接信息的快照。
    ///
    /// Source: `hub/route/connections.go` — `getConnections` (non-WebSocket path).
    /// 返回 `Snapshot()` 结果。
    pub async fn get_connections(&self) -> Result<ConnectionsResponse, ProcessError> {
        let resp = self.api().get("/connections").await?;
        parse_body(&resp.body)
    }

    /// DELETE /connections — 关闭所有连接。
    ///
    /// Source: `hub/route/connections.go` — `closeAllConnections`.
    pub async fn close_all_connections(&self) -> Result<(), ProcessError> {
        self.api().delete("/connections").await?;
        Ok(())
    }

    /// DELETE /connections/:id — 关闭指定连接。
    ///
    /// Source: `hub/route/connections.go` — `closeConnection`.
    pub async fn close_connection(&self, id: &str) -> Result<(), ProcessError> {
        let path = format!("/connections/{}", urlencoded(id));
        self.api().delete(&path).await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  DNS
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// GET /dns/query?name=xxx&type=A — DNS 查询。
    ///
    /// Source: `hub/route/dns.go` — `queryDNS`.
    /// - `name`: 域名，例如 `example.com`
    /// - `query_type`: DNS 记录类型字符串，例如 `"A"`, `"AAAA"`, `"MX"`, `"CNAME"`。
    ///   如果为空字符串，mihomo 默认使用 `"A"`。
    pub async fn dns_query(
        &self,
        name: &str,
        query_type: &str,
    ) -> Result<DnsQueryResponse, ProcessError> {
        let mut path = format!("/dns/query?name={}", urlencoded(name));
        if !query_type.is_empty() {
            path.push_str(&format!("&type={}", urlencoded(query_type)));
        }
        let resp = self.api().get(&path).await?;
        parse_body(&resp.body)
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Debug
// ═══════════════════════════════════════════════════════════════════════

impl MihomoManager {
    /// PUT /debug/gc — 手动触发 Go runtime GC（`debug.FreeOSMemory`）。
    ///
    /// Source: `hub/route/server.go` — debug router, `/gc` handler.
    /// 需要 mihomo 以 `log-level: debug` 启动才能访问 `/debug` 路径。
    pub async fn debug_gc(&self) -> Result<(), ProcessError> {
        self.api().put("/debug/gc", "").await?;
        Ok(())
    }
}
