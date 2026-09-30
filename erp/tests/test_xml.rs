//! Markup that plugins extend: parsed, located into, changed, and written back.

use erp::xml::{apply_extension, parse_fragment, to_markup};

fn extend(base: &str, spec: &str) -> Result<String, erp::xml::XmlError> {
    let mut nodes = parse_fragment(base)?;
    apply_extension(&mut nodes, &parse_fragment(spec)?)?;
    Ok(to_markup(&nodes))
}

#[test]
fn test_markup_is_written_back_as_it_was_read() {
    let markup = r#"<div class="a &quot;b&quot;"><span t-esc="x &amp;&amp; y"/>text &lt; 3<!-- note --></div><p/>"#;
    assert_eq!(to_markup(&parse_fragment(markup).unwrap()), markup);
}

#[test]
fn test_each_position() {
    let base = r#"<div><h1>Title</h1><p>Body</p></div>"#;
    let cases = [
        ("inside", r#"<div><h1>Title<i/></h1><p>Body</p></div>"#),
        ("after", r#"<div><h1>Title</h1><i/><p>Body</p></div>"#),
        ("before", r#"<div><i/><h1>Title</h1><p>Body</p></div>"#),
        ("replace", r#"<div><i/><p>Body</p></div>"#),
    ];
    for (position, expected) in cases {
        let spec = format!(r#"<xpath expr="//h1" position="{position}"><i/></xpath>"#);
        assert_eq!(extend(base, &spec).unwrap(), expected, "{position}");
    }
}

#[test]
fn test_attributes_are_set_and_removed() {
    let extended = extend(
        r#"<div class="a" id="x"/>"#,
        r#"<xpath expr="//div" position="attributes">
               <attribute name="class">b</attribute><attribute name="id"/><attribute name="title">t</attribute>
           </xpath>"#,
    )
    .unwrap();
    assert_eq!(extended, r#"<div class="b" title="t"/>"#);
}

#[test]
fn test_paths_select_by_name_attribute_and_position() {
    let base = r#"<form><group name="a"><field name="x"/></group><group name="b"><field name="y"/><field name="z"/></group></form>"#;
    for (expr, expected) in [
        ("//group[@name=\"b\"]/field[2]", r#"<field name="z"/><hr/>"#),
        ("//field[@name=\"x\"]", r#"<field name="x"/><hr/>"#),
        ("/form/group[2]", r#"<field name="z"/></group><hr/>"#),
        ("//*[@name=\"y\"]", r#"<field name="y"/><hr/>"#),
    ] {
        let extended = extend(
            base,
            &format!(r#"<xpath expr='{expr}' position="after"><hr/></xpath>"#),
        )
        .unwrap_or_else(|error| panic!("{expr}: {error}"));
        assert!(extended.contains(expected), "{expr}: {extended}");
    }
}

#[test]
fn test_a_path_may_quote_with_apostrophes() {
    let extended = extend(
        r#"<div><field name="x"/></div>"#,
        r#"<xpath expr="//field[@name='x']" position="replace"><field name="y"/></xpath>"#,
    )
    .unwrap();
    assert_eq!(extended, r#"<div><field name="y"/></div>"#);
}

/// Several changes at once, the second seeing what the first did.
#[test]
fn test_changes_apply_in_order() {
    let extended = extend(
        "<div><a/></div>",
        r#"<data><xpath expr="//a" position="after"><b/></xpath><xpath expr="//b" position="inside"><c/></xpath></data>"#,
    )
    .unwrap();
    assert_eq!(extended, "<div><a/><b><c/></b></div>");
}

/// An extension whose target moved says so, rather than doing nothing.
#[test]
fn test_a_path_matching_nothing_is_an_error() {
    let error = extend(
        "<div/>",
        r#"<xpath expr="//span" position="inside"><i/></xpath>"#,
    )
    .unwrap_err();
    assert!(error.to_string().contains("//span"), "got {error}");

    assert!(
        extend(
            "<div/>",
            r#"<xpath expr="//div" position="sideways"><i/></xpath>"#
        )
        .is_err()
    );
    assert!(extend("<div/>", r#"<span position="inside"/>"#).is_err());
    assert!(extend("<div/>", r#"<xpath expr="//div[@=1]"/>"#).is_err());
    assert!(parse_fragment("<div>").is_err());
}
