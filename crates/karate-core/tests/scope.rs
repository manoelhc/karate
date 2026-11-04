use karate_core::{Error, scope::*};

#[test]
fn origin_and_segment_boundaries_are_exact() {
    let scope = Scope::new("https://APP.example:443/api/", vec!["/api/admin/".into()]).unwrap();
    assert_eq!(scope.origin(), "https://app.example");
    for url in [
        "https://app.example/api",
        "https://app.example/api/",
        "https://app.example/api/items?q=%20",
        "https://app.example/api/administer",
    ] {
        assert!(scope.permits(url).is_ok(), "{url}");
    }
    for url in [
        "https://other.example/api",
        "http://app.example/api",
        "https://app.example:444/api",
        "https://app.example/apiculture",
        "https://app.example/api/admin",
        "https://app.example/api/admin/people",
        "https://app.example./api",
    ] {
        assert_eq!(scope.permits(url), Err(Error::OutOfScope), "{url}");
    }
    let all_blocked = Scope::new("https://app.example", vec!["/".into()]).unwrap();
    assert_eq!(
        all_blocked.permits("https://app.example/"),
        Err(Error::OutOfScope)
    );
    assert_eq!(
        all_blocked.permits("https://app.example/any"),
        Err(Error::OutOfScope)
    );
    assert!(
        Scope::new("https://app.example", vec![])
            .unwrap()
            .permits("https://app.example")
            .is_ok()
    );
}

#[test]
fn ambiguous_urls_are_rejected_before_normalization() {
    let scope = Scope::new("https://app.example", vec![]).unwrap();
    for url in [
        "invalid",
        "https:///app.example",
        "https://[:::1]/",
        "https://",
        "https://app.example:bad/",
        "file:///tmp/x",
        "https://@app.example/",
        "https://u:p@app.example/",
        "https://%61pp.example/",
        "https://app.example/#x",
        " https://app.example/",
        "https://app.example/\n",
        "https://app.example/\ta",
        "https://app.example/a\\b",
        "https://app.example/../admin",
        "https://app.example/a/./b",
        "https://app.example/a/../b",
        "https://app.example/%2e/admin",
        "https://app.example/a%2fb",
        "https://app.example/a%252fb",
        "https://app.example//admin",
        "https://app.example/ação",
        "https://app.example/a;b",
        "https://app.example/a b",
    ] {
        assert_eq!(scope.permits(url), Err(Error::OutOfScope), "{url}");
        assert_eq!(Scope::new(url, vec![]), Err(Error::Validation), "{url}");
    }
    assert_eq!(
        Scope::new("https://app.example/?q=1", vec![]),
        Err(Error::Validation)
    );
    assert_eq!(
        scope.permits(&format!("https://app.example/?q={}", "x".repeat(8192))),
        Err(Error::OutOfScope)
    );
    assert_eq!(
        Scope::new(&format!("https://app.example/{}", "x".repeat(2048)), vec![]),
        Err(Error::Validation)
    );
}

#[test]
fn exclusions_are_validated_and_bounded() {
    for path in [
        "",
        "admin",
        "/../",
        "/a/./b",
        "/%2F",
        "/a\\b",
        "/a\nb",
        "/a?b",
        "/a#b",
        "/a;b",
        "/ação",
        "/a//b",
        "/a b",
        &format!("/{}", "x".repeat(2048)),
    ] {
        assert_eq!(
            Scope::new("https://app.example", vec![path.into()]),
            Err(Error::Validation),
            "{path}"
        );
    }
    assert!(Scope::new("https://app.example", vec!["/ok".into(); 100]).is_ok());
    assert_eq!(
        Scope::new("https://app.example", vec!["/ok".into(); 101]),
        Err(Error::Validation)
    );
}

#[test]
fn special_addresses_mixed_dns_and_unbounded_results_fail_closed() {
    for value in [
        "0.0.0.0",
        "10.0.0.1",
        "127.0.0.1",
        "100.64.0.1",
        "100.127.255.255",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.0.1",
        "192.0.2.1",
        "192.88.99.1",
        "198.18.0.1",
        "198.19.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "::1",
        "::",
        "::ffff:127.0.0.1",
        "::ffff:8.8.8.8",
        "fe80::1",
        "fc00::1",
        "2001:db8::1",
        "2001::1",
        "2002::1",
        "3fff::1",
    ] {
        assert!(!public_ip(value.parse().unwrap()), "{value}");
        assert_eq!(
            public_addresses(&["8.8.8.8".parse().unwrap(), value.parse().unwrap()]),
            Err(Error::OutOfScope)
        );
    }
    for value in [
        "8.8.8.8",
        "1.1.1.1",
        "100.128.1.1",
        "172.15.0.1",
        "172.32.0.1",
        "192.88.98.1",
        "198.51.99.1",
        "203.0.112.1",
        "2606:4700:4700::1111",
        "2001:4860:4860::8888",
    ] {
        assert!(public_ip(value.parse().unwrap()), "{value}");
    }
    assert_eq!(public_addresses(&[]), Err(Error::OutOfScope));
    assert_eq!(
        public_addresses(&["8.8.8.8".parse().unwrap(); 65]),
        Err(Error::OutOfScope)
    );
    assert_eq!(public_addresses(&["8.8.8.8".parse().unwrap(); 64]), Ok(()));
}
