//! Outbound request guard for user-supplied URLs (recipe pages, images they
//! link to, Mealie servers).
//!
//! Loopback, link-local (including cloud metadata endpoints such as
//! 169.254.169.254) and unspecified addresses are refused, both when a host
//! name is resolved and when a redirect points at a literal IP. Private LAN
//! ranges stay allowed on purpose: self-hosters import from other services on
//! their own network.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// True for addresses user-supplied URLs must never reach.
pub fn is_blocked(ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
        v4 => v4,
    };
    ip.is_loopback()
        || ip.is_unspecified()
        || match ip {
            IpAddr::V4(v4) => v4.is_link_local() || v4.octets()[0] == 0,
            IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
        }
}

/// True if the URL's host is a literal IP address that is blocked.
pub fn url_has_blocked_ip(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(v4)) => is_blocked(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => is_blocked(IpAddr::V6(v6)),
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost") || d.ends_with(".localhost"),
        None => true,
    }
}

/// Resolver that drops blocked addresses and fails if none remain.
struct GuardedResolver;

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.filter(|a| !is_blocked(a.ip())).collect();
            if addrs.is_empty() {
                return Err(format!("refusing to connect to '{host}': it resolves to a local or link-local address").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// HTTP client for fetching user-supplied URLs.
pub fn guarded_client(user_agent: &str) -> reqwest::Result<reqwest::Client> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 8 {
            attempt.error("too many redirects")
        } else if url_has_blocked_ip(attempt.url()) {
            attempt.error("redirect to a local or link-local address refused")
        } else {
            attempt.follow()
        }
    });
    reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(10))
        .redirect(policy)
        .dns_resolver(Arc::new(GuardedResolver))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_local_addresses() {
        for ip in ["127.0.0.1", "0.0.0.0", "169.254.169.254", "::1", "fe80::1", "::ffff:127.0.0.1"] {
            assert!(is_blocked(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["192.168.1.10", "10.0.0.5", "8.8.8.8", "2001:db8::1"] {
            assert!(!is_blocked(ip.parse().unwrap()), "{ip}");
        }
        assert!(url_has_blocked_ip(&"http://localhost:8080/".parse().unwrap()));
        assert!(url_has_blocked_ip(&"http://[::1]/".parse().unwrap()));
        assert!(!url_has_blocked_ip(&"http://mealie.lan/".parse().unwrap()));
    }

    #[tokio::test]
    async fn client_refuses_loopback_hosts() {
        let client = guarded_client("test").unwrap();
        // "localhost" resolves only to loopback addresses, so the resolver refuses it.
        let err = client.get("http://localhost:9/").send().await.unwrap_err();
        assert!(format!("{err:?}").contains("refusing to connect"), "{err:?}");
    }
}
