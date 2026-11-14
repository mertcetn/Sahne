use super::*;

const MANIFEST: &str = r#"{"formatVersion":1,"apiVersion":1,"id":"test.wallpaper","name":"Original fixture","entry":"index.html","permissions":[],"preferredFps":30}"#;
fn entries() -> Vec<(String, Vec<u8>)> {
    vec![
        ("manifest.json".into(), MANIFEST.as_bytes().to_vec()),
        ("index.html".into(), b"<div></div>".to_vec()),
    ]
}
fn parse(value: &str) -> Result<Manifest> {
    Manifest::parse(value.as_bytes())
}

#[test]
fn immutable_package_retains_original_bytes_and_exact_case_lookup() {
    let package = Package::from_entries(entries()).unwrap();
    assert_eq!(package.manifest().id(), "test.wallpaper");
    assert_eq!(package.manifest().preferred_fps(), 30);
    assert_eq!(
        package
            .require_text(package.manifest().entry(), ".html", "entry")
            .unwrap(),
        "<div></div>"
    );
    assert!(
        package
            .file(&RelativePath::parse("INDEX.html").unwrap())
            .is_none()
    );
    assert_eq!(package.files().len(), 2);
}

#[test]
fn unknown_duplicate_and_escaped_duplicate_fields_are_schema_errors() {
    for field in [
        r#""unknown":true,"#,
        r#""id":"second","#,
        r#""\u0069d":"second","#,
    ] {
        let value = format!("{{{field}{}", &MANIFEST[1..]);
        let error = parse(&value).unwrap_err();
        assert_eq!(error.code, ErrorCode::ManifestSchema);
        assert!(error.line_column.is_some());
    }
}

#[test]
fn missing_required_fields_never_default() {
    for field in [
        r#""formatVersion":1,"#,
        r#""apiVersion":1,"#,
        r#""id":"test.wallpaper","#,
        r#""name":"Original fixture","#,
        r#""entry":"index.html","#,
        r#""permissions":[],"#,
        r#", "preferredFps":30"#,
    ] {
        let value = if field.starts_with(", ") {
            MANIFEST.replace(",\"preferredFps\":30", "")
        } else {
            MANIFEST.replace(field, "")
        };
        assert_eq!(
            parse(&value).unwrap_err().code,
            ErrorCode::ManifestSchema,
            "field={field}"
        );
    }
}

#[test]
fn permissions_cannot_request_any_capability_or_wrong_type() {
    for permission in [r#"["network"]"#, r#"[""]"#, "null", "{}", "false"] {
        assert_eq!(
            parse(&MANIFEST.replace("[]", permission)).unwrap_err().code,
            ErrorCode::ManifestSchema
        );
    }
}

#[test]
fn versions_fps_and_primitive_types_are_strict() {
    for (from, to, code) in [
        (
            "\"formatVersion\":1",
            "\"formatVersion\":2",
            ErrorCode::UnsupportedVersion,
        ),
        (
            "\"apiVersion\":1",
            "\"apiVersion\":0",
            ErrorCode::UnsupportedVersion,
        ),
        (
            "\"preferredFps\":30",
            "\"preferredFps\":0",
            ErrorCode::InvalidFps,
        ),
        (
            "\"preferredFps\":30",
            "\"preferredFps\":61",
            ErrorCode::InvalidFps,
        ),
        (
            "\"preferredFps\":30",
            "\"preferredFps\":1.0",
            ErrorCode::ManifestSchema,
        ),
        (
            "\"preferredFps\":30",
            "\"preferredFps\":65536",
            ErrorCode::ManifestSchema,
        ),
    ] {
        assert_eq!(parse(&MANIFEST.replace(from, to)).unwrap_err().code, code);
    }
    parse(&MANIFEST.replace("\"preferredFps\":30", "\"preferredFps\":60")).unwrap();
}

#[test]
fn optional_script_is_absent_or_a_local_js_string() {
    for value in [
        "null",
        "false",
        "[]",
        "\"code.bin\"",
        "\"https://example.test/a.js\"",
    ] {
        let input = format!("{{\"script\":{value},{}", &MANIFEST[1..]);
        assert!(parse(&input).is_err());
    }
    let mut input = entries();
    input[0].1 = format!("{{\"script\":\"script.js\",{}", &MANIFEST[1..]).into_bytes();
    assert_eq!(
        Package::from_entries(input.clone()).unwrap_err().code,
        ErrorCode::MissingSource
    );
    input.push((
        "script.js".into(),
        b"throw new Error('fixture must never execute');".to_vec(),
    ));
    let package = Package::from_entries(input).unwrap();
    assert_eq!(package.manifest().script().unwrap().as_str(), "script.js");
}

#[test]
fn parser_reports_locations_without_echoing_untrusted_text() {
    let error = parse("{\n\"secret\\u001bpayload\":true\n}").unwrap_err();
    assert!(error.line_column.unwrap().0 >= 2);
    assert!(!error.to_string().contains("secret"));
    assert_eq!(parse("{").unwrap_err().code, ErrorCode::ManifestSyntax);
    assert_eq!(
        parse(&(MANIFEST.to_owned() + " false")).unwrap_err().code,
        ErrorCode::ManifestSyntax
    );
    assert_eq!(
        Manifest::parse(&[0xff]).unwrap_err().code,
        ErrorCode::InvalidUtf8
    );
    assert_eq!(
        Manifest::parse(&vec![b' '; MAX_MANIFEST_BYTES + 1])
            .unwrap_err()
            .code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn portable_paths_reject_windows_and_url_escape_forms() {
    for path in [
        "",
        "/a",
        "a/",
        "a//b",
        ".",
        "..",
        "a/../b",
        "./a",
        "C:a",
        "C:/a",
        "\\\\host\\share",
        "a\\b",
        "a:stream",
        "http://host/a",
        "a%2fb",
        "a?x",
        "a#x",
        "a ",
        "a.",
        "a/CON.txt",
        "nul",
        "COM1",
        "lpt0.log",
        "é.html",
        "a\0b",
    ] {
        assert_eq!(
            RelativePath::parse(path).unwrap_err().code,
            ErrorCode::InvalidPath,
            "path={path:?}"
        );
    }
    for path in ["index.html", "Assets/image-1.png", "_data/a.b_c"] {
        RelativePath::parse(path).unwrap();
    }
    assert!(RelativePath::parse(&"a".repeat(65)).is_err());
    assert!(RelativePath::parse(&vec!["a"; 33].join("/")).is_err());
}

#[test]
fn duplicate_case_colliding_and_file_parent_paths_are_rejected() {
    for duplicate in ["index.html", "INDEX.html"] {
        let mut input = entries();
        input.push((duplicate.into(), vec![]));
        assert_eq!(
            Package::from_entries(input).unwrap_err().code,
            ErrorCode::CaseCollision
        );
    }
    for paths in [["assets", "assets/a.png"], ["assets/a.png", "ASSETS"]] {
        let mut input = entries();
        input.extend(paths.map(|path| (path.into(), vec![])));
        assert_eq!(
            Package::from_entries(input).unwrap_err().code,
            ErrorCode::PathConflict
        );
    }
}

#[test]
fn missing_manifest_entry_wrong_extension_and_utf8_fail_closed() {
    assert_eq!(
        Package::from_entries(vec![]).unwrap_err().code,
        ErrorCode::MissingManifest
    );
    let mut input = entries();
    input.pop();
    assert_eq!(
        Package::from_entries(input).unwrap_err().code,
        ErrorCode::MissingSource
    );
    assert_eq!(
        parse(&MANIFEST.replace("index.html", "index.js"))
            .unwrap_err()
            .code,
        ErrorCode::SourceType
    );
    let mut input = entries();
    input[1].1 = vec![0xff];
    assert_eq!(
        Package::from_entries(input).unwrap_err().code,
        ErrorCode::InvalidUtf8
    );
}

#[test]
fn package_size_accounting_checks_all_entries_and_boundaries() {
    check_sizes([MAX_FILE_BYTES; 4].into_iter()).unwrap();
    assert_eq!(
        check_sizes([MAX_FILE_BYTES; 5].into_iter())
            .unwrap_err()
            .code,
        ErrorCode::BudgetExceeded
    );
    assert_eq!(
        check_sizes([usize::MAX].into_iter()).unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
    check_sizes(std::iter::repeat_n(0, MAX_ENTRIES)).unwrap();
    assert_eq!(
        check_sizes(std::iter::repeat_n(0, MAX_ENTRIES + 1))
            .unwrap_err()
            .code,
        ErrorCode::BudgetExceeded
    );
    let mut input = entries();
    input.extend((0..MAX_ENTRIES).map(|index| (format!("unused-{index}"), vec![])));
    assert_eq!(
        Package::from_entries(input).unwrap_err().code,
        ErrorCode::BudgetExceeded
    );
}

#[test]
fn id_and_display_name_have_independent_limits() {
    assert_eq!(
        parse(&MANIFEST.replace("test.wallpaper", "../bad"))
            .unwrap_err()
            .code,
        ErrorCode::InvalidId
    );
    assert_eq!(
        parse(&MANIFEST.replace("Original fixture", " "))
            .unwrap_err()
            .code,
        ErrorCode::InvalidName
    );
    assert_eq!(
        parse(&MANIFEST.replace("Original fixture", "bad\\nname"))
            .unwrap_err()
            .code,
        ErrorCode::InvalidName
    );
    let manifest = parse(&MANIFEST.replace("Original fixture", "Özgün örnek")).unwrap();
    assert_eq!(manifest.name(), "Özgün örnek");
}
