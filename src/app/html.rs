use scraper::{Html, Selector};

pub fn get_html_content(el: &str, html: &str) -> String {
    let html = Html::parse_document(html);
    let selector = Selector::parse(el).unwrap();
    html.select(&selector)
        .next()
        .unwrap()
        .text()
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
}

pub fn get_nested_html_content(parent_el: &str, child_el: &str, html: &str) -> String {
    let html = Html::parse_document(html);
    let parent_selector = Selector::parse(parent_el).unwrap();
    let child_selector = Selector::parse(child_el).unwrap();

    let parent = html
        .select(&parent_selector)
        .next()
        .unwrap();

    let mut elements = parent
        .select(&child_selector)
        .into_iter();
    let el = elements.nth(0);
    if let Some(el) = el {
        el.text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string()
    } else {
        panic!("could not find element")
    }
}
