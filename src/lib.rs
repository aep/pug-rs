#[macro_use]
extern crate pest_derive;

use std::fmt::Debug;
pub use pest::RuleType;
use pest::Parser;
use std::io::Write;

#[derive(Debug)]
pub enum Error<E> {
    Parser(pest::error::Error<Rule>),
    Include(E),
    Io(std::io::Error),
}

impl<E: Debug> From<pest::error::Error<Rule>> for Error<E> {
    fn from(e : pest::error::Error<Rule>) -> Self {
        Error::Parser(e)
    }
}

impl<E: Debug> From<std::io::Error> for Error<E> {
    fn from(e : std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Parser)]
#[grammar = "pug.pest"]
pub struct PugParser;


#[derive(Default, Debug)]
pub struct Ast {
    pub children:   Vec<Ast>,
    pub id:         Option<String>,
    pub element:    String,
    pub class:      Vec<String>,
    pub attrs:      Vec<String>,
}

impl Ast {
    pub fn special<A: Into<String>, B: Into<String>>(element: A, id: B) -> Self {
        Self {
            element: element.into(),
            id: Some(id.into()),
            .. Default::default()
        }
    }

    pub fn expand<E, F>(mut self, mut inc: F) -> Result<Self, Error<E>>
        where E: Debug,
              F: Clone + FnMut(String) -> Result<Ast, E>,
    {
        match self.element.as_ref() {
            ":include" => {
                return inc(self.id.as_ref().unwrap().to_string()).map_err(Error::Include)?.expand(inc.clone());
            }
            _ => {
                for child in std::mem::replace(&mut self.children, Vec::new()) {
                    eprintln!("{}", child.element);
                    self.children.push(child.expand(inc.clone())?);
                };
                Ok(self)
            }
        }
    }

    pub fn to_html<W>(&self, w: &mut W) -> Result<(), std::io::Error>
        where W: Write,

    {
        self.to_html_i(w, &mut false)
    }

    fn to_html_i<W>(&self, w: &mut W, previous_was_text: &mut bool) -> Result<(), std::io::Error>
        where W: Write,

    {
        match self.element.as_ref() {
            ":include" => {
                panic!("include cannot be written to html. forgot to call expand?");
            }
            ":document" => {
                *previous_was_text = false;
                for child in &self.children {
                    child.to_html_i(w, previous_was_text)?;
                }
                return Ok(());
            }
            ":text" => {
                if *previous_was_text {
                    w.write_all(b"\n")?;
                }
                *previous_was_text = true;
                w.write_all(self.id.as_ref().unwrap().as_bytes())?;
                return Ok(());
            }
            ":doctype" => {
                *previous_was_text = false;
                w.write_all(b"<!DOCTYPE ")?;
                w.write_all(self.id.as_ref().unwrap().as_bytes())?;
                w.write_all(b">")?;
                return Ok(());
            }
            _ => {
                *previous_was_text = false;
                w.write_all(b"<")?;
                w.write_all(self.element.as_bytes())?;
                if !self.class.is_empty() {
                    w.write_all(b" class=\"")?;
                    w.write_all(self.class.join(" ").as_bytes())?;
                    w.write_all(b"\"")?;
                }
                if let Some(ref id) = self.id {
                    w.write_all(b" id=\"")?;
                    w.write_all(id.as_bytes())?;
                    w.write_all(b"\"")?;
                }
                for attr in &self.attrs {
                    w.write_all(b" ")?;
                    w.write_all(attr.as_bytes())?;
                }
                match self.element.as_ref() {
                    "area"|"base"|"br"|"col"|"command"|"embed"|"hr"|"img"|"input"|"keygen"|"link"|"meta"|"param"|"source"|"track"|"wbr" => {
                        w.write_all(b">")?;
                        return Ok(());
                    },
                    _ => (),
                };
                w.write_all(b">")?;
            }
        }

        for child in &self.children {
            child.to_html_i(w, previous_was_text)?;
        }

        w.write_all(b"</")?;
        w.write_all(self.element.as_bytes())?;
        w.write_all(b">")?;

        Ok(())
    }
}

fn parse_impl(file: &str) -> Result<Ast, pest::error::Error<Rule>> {
    let mut file = PugParser::parse(Rule::file, file)?;

    let mut comment = None;
    let mut indent = 0;

    let mut cur     = Ast::default();
    cur.element     = ":document".into();
    let mut stack : Vec<(usize, Ast)> = Vec::new();

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

                while let Some((ind, mut ast)) = stack.pop() {
                    if ind >= indent {
                        ast.children.push(std::mem::replace(&mut cur, Ast::default()));
                        cur = ast;
                    } else {
                        stack.push((ind,ast));
                        break;
                    }
                }
            }
            Rule::include => {
                cur.children.push(Ast::special(":include", decl.into_inner().as_str()));
            }
            Rule::doctype => {
                cur.children.push(Ast::special(":doctype", decl.into_inner().as_str()));
            }
            Rule::tag => {
                eprintln!("tag: {}", decl.as_str());
                if comment.is_some() {
                    continue;
                }

                let parent = std::mem::replace(&mut cur, Ast::default());
                stack.push((indent, parent));

                cur.element = "div".into();
                for e in decl.into_inner() {
                    match e.as_rule() {
                        Rule::element => {
                            cur.element = e.as_str().to_string();
                        }
                        Rule::class => {
                            cur.class.push(e.into_inner().next().unwrap().as_str().to_string());
                        }
                        Rule::id => {
                            cur.id = Some(e.into_inner().next().unwrap().as_str().to_string());
                        }
                        Rule::attrs => {
                            for e in e.into_inner() {
                                let mut e = e.into_inner();
                                let key = e.next().unwrap().as_str();
                                let value = e.next().unwrap();
                                if key == "id" {
                                    cur.id = Some(
                                        value.into_inner().next().unwrap().as_str().to_string(),
                                    );
                                } else if key == "class" {
                                    cur.class.push(
                                        value.into_inner().next().unwrap().as_str().to_string(),
                                    );
                                } else {
                                    cur.attrs.push(format!("{}={}", key, value.as_str()));
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
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
                let text = decl.as_str().to_string();
                cur.children.push(Ast::special(":text", text));
            }
            Rule::EOI => {
                for (_, mut ast) in stack.drain(..).rev() {
                    ast.children.push(std::mem::replace(&mut cur, Ast::default()));
                    cur = ast;
                }
            }
            any => panic!(println!("parser bug. did not expect: {:?}", any)),
        }
    }

    Ok(cur)
}

/// parse a Pug template into an abstract syntax tree
pub fn parse<S: Into<String>>(file: S) -> Result<Ast, pest::error::Error<Rule>> {
    let mut file = file.into();
    file.push('\n');
    parse_impl(&file)
}

#[test]
pub fn valid_identitifer_characters() {
    let mut html = Vec::new();
    parse(
        r#"a(a="b",a-:.b.="c"
x="y")"#
    ).unwrap().to_html(&mut html).unwrap();
    assert_eq!(html, br#"<a a="b" a-:.b.="c" x="y"></a>"#);
}

#[test]
pub fn emptyline() {
    let mut html = Vec::new();
    parse(
        r#"
a
  b

  c

"#
    ).unwrap().to_html(&mut html).unwrap();
    assert_eq!(html, br#"<a><b></b><c></c></a>"#);
}

#[test]
pub fn dupclass() {
    let mut html = Vec::new();
    parse(r#"a#x.b(id="v" class="c")"#).unwrap().to_html(&mut html).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<a class="b c" id="v"></a>"#
    );
}

#[test]
pub fn preserve_newline_in_multiline_text() {
    let mut html = Vec::new();
    parse(
        r#"pre
  | The pipe always goes at the beginning of its own line,
  | not counting indentation.
  |   lol look at me
  |   getting all getho indent
  |     watt"#
    ).unwrap().to_html(&mut html).unwrap();


    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<pre>The pipe always goes at the beginning of its own line,
not counting indentation.
  lol look at me
  getting all getho indent
    watt</pre>"#
    );
}

#[test]
pub fn eoi() {
    let mut html = Vec::new();
    parse(
        r#"body#blorp.herp.derp
  a(href="google.de")
derp
  yorlo jaja"#
    ).unwrap().to_html(&mut html).unwrap();

    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<body class="herp derp" id="blorp"><a href="google.de"></a></body><derp><yorlo>jaja</yorlo></derp>"#
    );

    let mut html = Vec::new();
    parse(
        r#"body#blorp.herp.derp
  a(href="google.de")
derp
  yorlo jaja
  "#
    ).unwrap().to_html(&mut html).unwrap();

    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<body class="herp derp" id="blorp"><a href="google.de"></a></body><derp><yorlo>jaja</yorlo></derp>"#
    );

    let mut html = Vec::new();
    parse(
        r#"body#blorp.herp.derp
  a(href="google.de")
derp
  yorlo jaja



"#
    ).unwrap().to_html(&mut html).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<body class="herp derp" id="blorp"><a href="google.de"></a></body><derp><yorlo>jaja</yorlo></derp>"#
    );
}

#[test]
pub fn doctype() {
    let mut html = Vec::new();
    parse(
        r#"doctype html
html
  body
"#
    ).unwrap().to_html(&mut html).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<!DOCTYPE html><html><body></body></html>"#
    );
}

#[test]
pub fn voidelements() {
    let mut html = Vec::new();
    parse(
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
    ).unwrap().to_html(&mut html).unwrap();

    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<!DOCTYPE html><html><head lang="en"><meta charset="utf-8"><title>n1's personal site</title><link rel="stylesheet" href="normalize.css"><link rel="stylesheet" href="style.css"></head><body><div class="container"></div></body></html>"#
    );
}


#[test]
pub fn include_p() {
    let ast = parse("include ./a").unwrap();
    assert_eq!(
        ast.children.len(),
        1
    );
    assert_eq!(
        ast.children[0].element,
        ":include"
    );
}


#[test]
pub fn include () {
    let f = |i:String| match i.as_ref() {
        "/a/1" => parse("include a"),
        _      => parse("| tomato"),
    };
    let mut html = Vec::new();
    parse(
        r#"
doctype html
kebab
    include /a/1
"#
    ).unwrap().expand(f).unwrap().to_html(&mut html).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&html),
        r#"<!DOCTYPE html><kebab>tomato</kebab>"#
    );
}


