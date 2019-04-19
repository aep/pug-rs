#[macro_use]
extern crate pest_derive;

use std::fmt::Debug;
pub use pest::RuleType;
use pest::Parser;

#[derive(Debug)]
pub enum Error<E> {
    Parser(pest::error::Error<Rule>),
    Include(E)
}

impl<E: Debug> From<pest::error::Error<Rule>> for Error<E> {
    fn from(e : pest::error::Error<Rule>) -> Self {
        Error::Parser(e)
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Parser)]
#[grammar = "pug.pest"]
pub struct PugParser;

fn generate<E: Debug, F: FnMut(&str) -> Result<String, E>>  (file: &str, mut inc: F) -> Result<String, Error<E>> {
    let mut file = PugParser::parse(Rule::file, file)?;
    let mut html = String::new();

    let mut previous_was_text = false;
    let mut comment = None;
    let mut indent = 0;
    let mut tagstack: Vec<(usize, String)> = Vec::new();

    for decl in file.next().unwrap().into_inner() {
        match decl.as_rule() {
            Rule::indent => {
                indent = decl.as_str().len();

                if let Some(ind) = comment {
                    if indent > ind {
                        continue;
                    } else {
                        comment = None;
                    }
                }

                while let Some((ind, element)) = tagstack.last().cloned() {
                    if ind >= indent {
                        html.push_str("</");
                        html.push_str(&element);
                        html.push_str(">");
                        tagstack.pop();
                    } else {
                        break;
                    }
                }
            }
            Rule::include => {
                match inc(decl.into_inner().as_str()) {
                    Err(e) => return Err(Error::Include(e)),
                    Ok(v) => html.push_str(&v),
                };
            }
            Rule::doctype => {
                html.push_str("<!DOCTYPE ");
                html.push_str(decl.into_inner().as_str());
                html.push('>');
            }
            Rule::tag => {
                if comment.is_some() {
                    continue;
                }
                previous_was_text = false;

                let mut element = "div".to_string();
                let mut id = None;
                let mut class = Vec::new();
                let mut attrs = Vec::new();
                for e in decl.into_inner() {
                    match e.as_rule() {
                        Rule::element => {
                            element = e.as_str().to_string();
                        }
                        Rule::class => {
                            class.push(e.into_inner().next().unwrap().as_str().to_string());
                        }
                        Rule::id => {
                            id = Some(e.into_inner().next().unwrap().as_str().to_string());
                        }
                        Rule::attrs => {
                            for e in e.into_inner() {
                                let mut e = e.into_inner();
                                let key = e.next().unwrap().as_str();
                                let value = e.next().unwrap();
                                if key == "id" {
                                    id = Some(
                                        value.into_inner().next().unwrap().as_str().to_string(),
                                    );
                                } else if key == "class" {
                                    class.push(
                                        value.into_inner().next().unwrap().as_str().to_string(),
                                    );
                                } else {
                                    attrs.push(format!("{}={}", key, value.as_str()));
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }


                html.push('<');
                html.push_str(&element);
                if !class.is_empty() {
                    html.push_str(" class=\"");
                    html.push_str(&class.join(" "));
                    html.push('"');
                }
                if let Some(id) = id {
                    html.push_str(" id=\"");
                    html.push_str(&id);
                    html.push('"');
                }
                for attr in attrs {
                    html.push(' ');
                    html.push_str(&attr);
                }
                html.push('>');

                if !is_void_element(&element) {
                    tagstack.push((indent, element));
                }
            }
            Rule::comment => {
                if comment.is_some() {
                    continue;
                }
                comment = Some(indent);
            }
            Rule::text => {
                if comment.is_some() {
                    continue;
                }
                if previous_was_text {
                    html.push('\n')
                }
                html.push_str(decl.as_str());
                previous_was_text = true;
            }
            Rule::EOI => {
                for (_, element) in tagstack.drain(..).rev() {
                    html.push_str("</");
                    html.push_str(&element);
                    html.push_str(">");
                }
            }
            any => panic!(println!("parser bug. did not expect: {:?}", any)),
        }
    }

    Ok(html)
}

fn is_void_element(e: &str) -> bool {
    match e {
        "area"|"base"|"br"|"col"|"command"|"embed"|"hr"|"img"|"input"|"keygen"|"link"|"meta"|"param"|"source"|"track"|"wbr" => true,
        _ => false
    }
}

/// Render a Pug template into html.
#[cfg(not(target_arch = "wasm32"))]
pub fn parse<E : Debug, F: FnMut(&str) -> Result<String, E>>  (mut file: String, inc: F) -> Result<String, Error<E>> {
    file.push('\n');
    generate(&file, inc)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn parse(mut file: String) -> Option<String> {
    file.push('\n');

    generate(&file).ok()
}

#[test]
pub fn valid_identitifer_characters() {
    let html = parse(
        r#"a(a="b",a-:.b.="c"
x="y")"#
            .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(html, r#"<a a="b" a-:.b.="c" x="y"></a>"#);
}

#[test]
pub fn emptyline() {
    let html = parse(
        r#"
a
  b

  c

"#
        .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(html, r#"<a><b></b><c></c></a>"#);
}

#[test]
pub fn dupclass() {
    let html = parse(r#"a#x.b(id="v" class="c")"#.to_string(), |_|Err(0)).unwrap();
    assert_eq!(html, r#"<a class="b c" id="v"></a>"#);
}

#[test]
pub fn preserve_newline_in_multiline_text() {
    let html = parse(
        r#"pre
  | The pipe always goes at the beginning of its own line,
  | not counting indentation.
  |   lol look at me
  |   getting all getho indent
  |     watt"#
            .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(
        html,
        r#"<pre>The pipe always goes at the beginning of its own line,
not counting indentation.
  lol look at me
  getting all getho indent
    watt</pre>"#
    );
}

#[test]
pub fn eoi() {
    let html = parse(
        r#"body#blorp.herp.derp
  a(href="google.de")
derp
  yorlo jaja"#
            .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(html,
    r#"<body class="herp derp" id="blorp"><a href="google.de"></a></body><derp><yorlo>jaja</yorlo></derp>"#
    );

    let html = parse(
        r#"body#blorp.herp.derp
  a(href="google.de")
derp
  yorlo jaja
  "#
        .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(html,
    r#"<body class="herp derp" id="blorp"><a href="google.de"></a></body><derp><yorlo>jaja</yorlo></derp>"#
    );

    let html = parse(
        r#"body#blorp.herp.derp
  a(href="google.de")
derp
  yorlo jaja



"#
        .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(html,
    r#"<body class="herp derp" id="blorp"><a href="google.de"></a></body><derp><yorlo>jaja</yorlo></derp>"#
    );
}

#[test]
pub fn doctype() {
    let html = parse(
        r#"doctype html
html
  body
"#
            .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(
        html,
        r#"<!DOCTYPE html><html><body></body></html>"#
    );
}

#[test]
pub fn voidelements() {
    let html = parse(
        r#"
doctype html
html
    head(lang="en")
        meta(charset="utf-8")
        title n1's personal site
        link(rel="stylesheet", href="normalize.css")
        link(rel="stylesheet", href="style.css")

    body
        .container
"#
            .to_string(),
    |_|Err(0))
    .unwrap();
    assert_eq!(
        html,
        r#"<!DOCTYPE html><html><head lang="en"><meta charset="utf-8"><title>n1's personal site</title><link rel="stylesheet" href="normalize.css"><link rel="stylesheet" href="style.css"></head><body><div class="container"></div></body></html>"#
    );
}


#[test]
pub fn include () {
    let html = parse::<String, _>(
        r#"
doctype html
kebab
    include salad
"#
            .to_string(),
    |_|Ok("tomato".to_string()))
    .unwrap();
    assert_eq!(
        html,
        r#"<!DOCTYPE html><kebab>tomato</kebab>"#
    );
}


