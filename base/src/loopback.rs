use std::net::SocketAddr;

pub fn binds_beyond_loopback(addr: SocketAddr) -> bool {
    !addr.ip().is_loopback()
}

pub fn warn_if_binds_beyond_loopback(element: &str, addr: SocketAddr, suppressed: bool) {
    if suppressed || !binds_beyond_loopback(addr) {
        return;
    }
    log::warn!(
        "SECURITY: `{element}` bound to non-loopback `{addr}`: unauthenticated, reachable hosts may inject or read data. use a loopback address (127.0.0.1), or set `suppress_non_loopback_bind_warning: true` to silence this warning."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beyond_loopback_ipv4_unspecified() {
        assert!(binds_beyond_loopback("0.0.0.0:8080".parse().unwrap()));
    }

    #[test]
    fn beyond_loopback_ipv6_unspecified() {
        assert!(binds_beyond_loopback("[::]:8080".parse().unwrap()));
    }

    #[test]
    fn beyond_loopback_ipv4_lan() {
        assert!(binds_beyond_loopback("192.168.1.5:8080".parse().unwrap()));
    }

    #[test]
    fn loopback_ipv4() {
        assert!(!binds_beyond_loopback("127.0.0.1:8080".parse().unwrap()));
    }

    #[test]
    fn loopback_ipv4_other_in_range() {
        assert!(!binds_beyond_loopback("127.0.0.2:8080".parse().unwrap()));
    }

    #[test]
    fn loopback_ipv6() {
        assert!(!binds_beyond_loopback("[::1]:8080".parse().unwrap()));
    }

    /// `Ipv6Addr::is_loopback` only treats `::1` as loopback, not IPv4-mapped
    /// addresses. This means `::ffff:127.0.0.1` is intentionally reported as
    /// beyond loopback (i.e. we warn) even though it maps to 127.0.0.1.
    #[test]
    fn ipv4_mapped_loopback_is_not_recognized_as_loopback() {
        assert!(binds_beyond_loopback(
            "[::ffff:127.0.0.1]:8080".parse().unwrap()
        ));
    }

    #[test]
    fn suppressed_warning_does_not_check_address() {
        // Should not panic or otherwise misbehave when suppressed, even for
        // a non-loopback address.
        warn_if_binds_beyond_loopback("test-element", "0.0.0.0:8080".parse().unwrap(), true);
    }
}
